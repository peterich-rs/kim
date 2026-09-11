//! Local Goose-backed agent host.
//!
//! KIM owns identity, @mention routing, and IM send/receive. This crate
//! assembles Goose's unrolled loop (`goose-agent`) with `goose-providers`
//! (OpenAI Completions/Responses and Anthropic Messages).

mod events;
mod machine;
mod ops;
mod profile;
mod provider;
mod scripted;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::machine::{EffectHandler, MachineSession, SessionLoader, StateMachine};
use goose_agent::operation::{ConversationEffect, Emitter};
use goose_provider_types::base::Provider;
use goose_provider_types::conversation::message::{Message, MessageContent};
use goose_provider_types::conversation::Conversation;
use goose_provider_types::model::ModelConfig;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

pub use events::{HostError, HostEvent, PendingYield, TurnOutcome, YieldKind};
pub use machine::MachineFactory;
pub use profile::{
    builtin_templates, AgentProfile, ExtensionSpec, LegacyOpenOpts, ModelSpec, PermissionConfig,
    PermissionDefault, ProviderSpec, ResolvedProfile, SandboxMode, SandboxPolicy, ToolSet,
};
pub use provider::{
    bundled_declarative_json, bundled_provider_summaries, fetch_models, BundledProviderSummary,
    ProviderConfig, ProviderKind, SessionKeyResolver,
};
pub use scripted::ScriptedProvider;

pub(crate) use events::HostEffect;

/// Built-in persona: mention `@助手` or `@goose` in an IM thread.
pub const DEFAULT_AGENT_ID: &str = "goose";
pub const DEFAULT_AGENT_NAME: &str = "助手";
pub const DEFAULT_SYSTEM_PROMPT: &str =
    "You are 助手, a local desktop agent inside the KIM messenger. \
You run on the user's machine (not a cloud bot). Reply in the user's language. \
Be concise. You can see the current conversation because the host pasted it into this session. \
Do not claim you have tools you were not given.";

pub(crate) struct HostSession {
    pub id: String,
    pub conversation: Conversation,
}

impl MachineSession for HostSession {
    fn id(&self) -> &str {
        &self.id
    }

    fn conversation(&self) -> Option<&Conversation> {
        Some(&self.conversation)
    }
}

struct Store {
    conversations: HashMap<String, Conversation>,
}

struct Inner {
    profile: AgentProfile,
    provider: Arc<dyn Provider>,
    model: ModelConfig,
    project_root: PathBuf,
    store: Mutex<Store>,
}

#[derive(Clone)]
pub struct AgentHost {
    inner: Arc<Inner>,
}

impl AgentHost {
    pub fn new(config: ProviderConfig) -> Result<Self, HostError> {
        Self::from_resolved(ResolvedProfile::from_provider_config(config)?)
    }

    pub fn from_provider_for_test(
        profile: AgentProfile,
        provider: Arc<dyn Provider>,
        project_root: PathBuf,
    ) -> Result<Self, HostError> {
        let model = machine::model_config(&profile.model)?;
        Ok(Self {
            inner: Arc::new(Inner {
                profile,
                provider,
                model,
                project_root,
                store: Mutex::new(Store {
                    conversations: HashMap::new(),
                }),
            }),
        })
    }

    pub fn from_resolved(resolved: ResolvedProfile) -> Result<Self, HostError> {
        if resolved.api_key.trim().is_empty() {
            return Err(HostError::MissingApiKey);
        }
        let provider =
            provider::build_provider_from_spec(&resolved.profile.provider, &resolved.api_key)?;
        let model = machine::model_config(&resolved.profile.model)?;
        tracing::info!(
            profile_id = %resolved.profile.id,
            provider = %resolved.profile.provider.kind,
            model = %resolved.profile.model.name,
            "agent host assembled"
        );
        Ok(Self {
            inner: Arc::new(Inner {
                profile: resolved.profile,
                provider,
                model,
                project_root: resolved.project_root,
                store: Mutex::new(Store {
                    conversations: HashMap::new(),
                }),
            }),
        })
    }

    pub async fn prompt(
        &self,
        session_id: &str,
        text: &str,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError> {
        self.prompt_with_context(session_id, text, None, events, cancel)
            .await
    }

    pub async fn prompt_with_context(
        &self,
        session_id: &str,
        text: &str,
        context_json: Option<&str>,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(HostError::Failed("empty prompt".into()));
        }

        {
            let mut store = self.inner.store.lock().await;
            let conversation = store
                .conversations
                .entry(session_id.to_string())
                .or_insert_with(Conversation::empty);
            if let Some(ctx) = context_json.map(str::trim).filter(|s| !s.is_empty()) {
                let preview = truncate_chars(ctx, 8 * 1024);
                conversation.push(
                    Message::user()
                        .with_text(preview)
                        .with_visibility(false, true),
                );
            }
            conversation.push(Message::user().with_text(text));
        }

        tracing::info!(session_id, "agent turn start");
        self.run_loop(session_id, events, cancel).await
    }

    pub async fn complete_tool(
        &self,
        session_id: &str,
        call_id: &str,
        output_json: &str,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError> {
        {
            let mut store = self.inner.store.lock().await;
            let conversation = store
                .conversations
                .get_mut(session_id)
                .ok_or_else(|| HostError::UnknownSession(session_id.to_string()))?;
            let pending = kim_pending(conversation, &self.inner.profile.tools);
            if !pending.iter().any(|p| p.call_id == call_id) {
                return Err(HostError::UnknownToolCall(call_id.to_string()));
            }
            let result = parse_tool_output(output_json);
            let message = Message::user().with_tool_response(call_id.to_string(), Ok(result));
            conversation.push(message);
        }
        let remaining = self.pending_yields(session_id).await;
        if !remaining.is_empty() {
            let first = &remaining[0];
            return Ok(TurnOutcome::Yielded {
                kind: YieldKind::ToolRequest,
                call_id: first.call_id.clone(),
                name: first.name.clone(),
            });
        }
        self.run_loop(session_id, events, cancel).await
    }

    pub async fn pending_yields(&self, session_id: &str) -> Vec<PendingYield> {
        let store = self.inner.store.lock().await;
        store
            .conversations
            .get(session_id)
            .map(|c| kim_pending(c, &self.inner.profile.tools))
            .unwrap_or_default()
    }

    pub async fn cancel_pending_tools(&self, session_id: &str) {
        let mut store = self.inner.store.lock().await;
        let Some(conversation) = store.conversations.get_mut(session_id) else {
            return;
        };
        let pending = kim_pending(conversation, &self.inner.profile.tools);
        if pending.is_empty() {
            return;
        }
        let mut message = Message::user();
        for p in pending {
            message.add_tool_response_with_metadata(
                p.call_id,
                Ok(rmcp::model::CallToolResult::error(vec![
                    rmcp::model::ContentBlock::text("cancelled"),
                ])),
                None,
            );
        }
        conversation.push(message);
    }

    pub fn kim_tool_names(&self) -> Vec<&'static str> {
        self.inner.profile.tools.kim_world_names()
    }

    #[cfg(test)]
    async fn conversation_for_test(&self, session_id: &str) -> Conversation {
        let store = self.inner.store.lock().await;
        store
            .conversations
            .get(session_id)
            .cloned()
            .unwrap_or_else(Conversation::empty)
    }
}

impl AgentHost {
    async fn run_loop(
        &self,
        session_id: &str,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError> {
        let (tx, mut rx) = mpsc::channel(64);
        let emit = Emitter::new(tx, cancel.clone());
        let events_for_pump = events.clone();
        let pump = tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                if let goose_agent::events::AgentEvent::Message(message) = ev {
                    pump_message(&events_for_pump, &message).await;
                }
            }
        });

        let steps = MachineFactory::assemble(
            &self.inner.profile,
            Arc::clone(&self.inner.provider),
            self.inner.model.clone(),
            &self.inner.project_root,
        );
        let machine = StateMachine::new(steps, cancel);

        let outcome = machine.run(self, session_id, &emit).await;
        drop(emit);
        let _ = pump.await;

        match outcome {
            Ok(session) => {
                let pending = kim_pending(&session.conversation, &self.inner.profile.tools);
                if let Some(first) = pending.first() {
                    tracing::info!(session_id, outcome = "yielded", "agent turn end");
                    Ok(TurnOutcome::Yielded {
                        kind: YieldKind::ToolRequest,
                        call_id: first.call_id.clone(),
                        name: first.name.clone(),
                    })
                } else {
                    let text = last_assistant_text(&session.conversation);
                    let _ = events
                        .send(HostEvent::Finished { text: text.clone() })
                        .await;
                    tracing::info!(session_id, outcome = "finished", "agent turn end");
                    Ok(TurnOutcome::Finished { text })
                }
            }
            Err(err) => {
                tracing::info!(session_id, outcome = "failed", "agent turn end");
                Err(HostError::Failed(err.to_string()))
            }
        }
    }
}

#[async_trait]
impl SessionLoader<HostSession> for AgentHost {
    async fn load(&self, session_id: &str) -> Result<HostSession> {
        let store = self.inner.store.lock().await;
        let conversation = store
            .conversations
            .get(session_id)
            .cloned()
            .unwrap_or_else(Conversation::empty);
        Ok(HostSession {
            id: session_id.to_string(),
            conversation,
        })
    }
}

#[async_trait]
impl EffectHandler<HostSession, HostEffect> for AgentHost {
    async fn apply_effects(
        &self,
        session: &HostSession,
        effects: &mut [HostEffect],
        _emit: &Emitter,
    ) -> Result<()> {
        let mut store = self.inner.store.lock().await;
        let conversation = store
            .conversations
            .entry(session.id.clone())
            .or_insert_with(Conversation::empty);
        for effect in effects.iter_mut() {
            match effect {
                HostEffect::Usage(_) => {}
                HostEffect::Conversation(ConversationEffect::AppendMessage(m)) => {
                    conversation.push(m.clone());
                }
                HostEffect::Conversation(ConversationEffect::ReplaceConversation(c)) => {
                    *conversation = c.clone();
                }
                HostEffect::Conversation(ConversationEffect::PatchToolRequestMeta {
                    tool_call_id,
                    patch,
                }) => {
                    patch_tool_request_meta(conversation, tool_call_id, patch)?;
                }
                HostEffect::Conversation(ConversationEffect::SetMessageVisibility {
                    message_id,
                    user_visible,
                    agent_visible,
                }) => {
                    set_visibility(conversation, message_id, *user_visible, *agent_visible)?;
                }
            }
        }
        Ok(())
    }
}

fn patch_tool_request_meta(
    conversation: &mut Conversation,
    tool_call_id: &str,
    patch: &serde_json::Value,
) -> Result<()> {
    for message in conversation.messages_mut() {
        for block in &mut message.content {
            let MessageContent::ToolRequest(req) = block else {
                continue;
            };
            if req.id != tool_call_id {
                continue;
            }
            let mut meta = req
                .tool_meta
                .take()
                .unwrap_or_else(|| serde_json::json!({}));
            match (meta.as_object_mut(), patch.as_object()) {
                (Some(dst), Some(src)) => {
                    for (k, v) in src {
                        dst.insert(k.clone(), v.clone());
                    }
                }
                _ => meta = patch.clone(),
            }
            req.tool_meta = Some(meta);
            return Ok(());
        }
    }
    anyhow::bail!("tool request {tool_call_id} not found")
}

fn set_visibility(
    conversation: &mut Conversation,
    message_id: &str,
    user_visible: bool,
    agent_visible: bool,
) -> Result<()> {
    for message in conversation.messages_mut() {
        if message.id.as_deref() == Some(message_id) {
            message.metadata.user_visible = user_visible;
            message.metadata.agent_visible = agent_visible;
            return Ok(());
        }
    }
    anyhow::bail!("message {message_id} not found")
}

async fn pump_message(events: &mpsc::Sender<HostEvent>, message: &Message) {
    let delta = message_text(message);
    if !delta.is_empty() {
        let _ = events.send(HostEvent::TextDelta { delta }).await;
    }
    for block in &message.content {
        match block {
            MessageContent::ToolRequest(req) => {
                let name = req
                    .tool_call
                    .as_ref()
                    .ok()
                    .map(|c| c.name.to_string())
                    .unwrap_or_default();
                if is_kim_world_tool(&name) {
                    continue;
                }
                let (name, arguments_json) = match req.tool_call.as_ref() {
                    Ok(call) => (
                        call.name.to_string(),
                        call.arguments
                            .as_ref()
                            .map(|a| serde_json::to_string(a).unwrap_or_else(|_| "{}".into()))
                            .unwrap_or_else(|| "{}".into()),
                    ),
                    Err(_) => (String::new(), String::new()),
                };
                let _ = events
                    .send(HostEvent::ToolRequest {
                        call_id: req.id.clone(),
                        name,
                        arguments_json,
                    })
                    .await;
            }
            MessageContent::ToolResponse(res) => {
                let preview = block
                    .as_tool_response_text()
                    .unwrap_or_default()
                    .chars()
                    .take(80)
                    .collect::<String>();
                let ok = match &res.tool_result {
                    Ok(result) => result.is_error != Some(true),
                    Err(_) => false,
                };
                let _ = events
                    .send(HostEvent::ToolResult {
                        call_id: res.id.clone(),
                        name: String::new(),
                        output_preview: preview,
                        ok,
                    })
                    .await;
            }
            _ => {}
        }
    }
}

fn is_kim_world_tool(name: &str) -> bool {
    matches!(
        name,
        "search_contacts"
            | "search_messages"
            | "get_conversation_context"
            | "list_profiles"
            | "send_message"
            | "read_clipboard"
    )
}

fn kim_pending(conversation: &Conversation, tools: &ToolSet) -> Vec<PendingYield> {
    let names = tools.kim_world_names();
    ops::unanswered_tool_requests(conversation)
        .into_iter()
        .filter_map(|req| {
            let call = req.tool_call.as_ref().ok()?;
            let name = call.name.as_ref();
            if !names.contains(&name) {
                return None;
            }
            let arguments_json = call
                .arguments
                .as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_else(|_| "{}".into()))
                .unwrap_or_else(|| "{}".into());
            Some(PendingYield {
                call_id: req.id,
                name: name.to_string(),
                arguments_json,
            })
        })
        .collect()
}

fn parse_tool_output(raw: &str) -> rmcp::model::CallToolResult {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) if value.is_object() || value.is_array() || value.is_string() => {
            rmcp::model::CallToolResult::structured(value)
        }
        _ => rmcp::model::CallToolResult::error(vec![rmcp::model::ContentBlock::text(
            raw.to_string(),
        )]),
    }
}

fn truncate_chars(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn last_assistant_text(conversation: &Conversation) -> String {
    conversation
        .messages()
        .last()
        .map(message_text)
        .unwrap_or_default()
}

fn message_text(message: &Message) -> String {
    let mut out = String::new();
    for block in &message.content {
        if let MessageContent::Text(text) = block {
            out.push_str(&text.text);
        }
    }
    out
}

/// True when `text` @-mentions the built-in Goose persona.
pub fn mentions_default_agent(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let t = token.trim_matches(|c: char| matches!(c, ',' | '.' | '!' | '?' | '，' | '。'));
        t.eq_ignore_ascii_case("@goose") || t == "@助手"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mention_detects_handle() {
        assert!(mentions_default_agent("hey @助手 总结一下"));
        assert!(mentions_default_agent("@goose please"));
        assert!(!mentions_default_agent("hello goose"));
        assert!(!mentions_default_agent("email goose@x.com"));
    }

    #[test]
    fn missing_key_is_explicit() {
        let result = AgentHost::new(ProviderConfig::openai("", "gpt-4o"));
        assert!(matches!(result, Err(HostError::MissingApiKey)));
    }

    #[tokio::test]
    async fn scripted_fs_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("note.txt"), "hello workspace").unwrap();
        let mut args = rmcp::model::JsonObject::new();
        args.insert("path".into(), serde_json::json!("note.txt"));
        let mut call = rmcp::model::CallToolRequestParams::new("read_file");
        call.arguments = Some(args);
        let assistant = Message::assistant().with_tool_request("c1", Ok(call));
        let profile = AgentProfile::from_legacy(&crate::profile::LegacyOpenOpts {
            enable_fs_tools: true,
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..crate::profile::LegacyOpenOpts::default()
        });
        let host = AgentHost::from_provider_for_test(
            profile,
            Arc::new(ScriptedProvider::new(vec![assistant])),
            root,
        )
        .unwrap();
        let (tx, mut rx) = mpsc::channel(32);
        let outcome = host
            .prompt("s", "read the note", tx, CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(outcome, TurnOutcome::Finished { .. }));
        let mut saw_tool = false;
        while let Ok(ev) = rx.try_recv() {
            if let HostEvent::ToolResult { ok, .. } = ev {
                saw_tool = ok;
            }
        }
        assert!(saw_tool, "fs tool should succeed");
    }

    fn kim_search_call(id: &str, query: &str) -> Message {
        let mut args = rmcp::model::JsonObject::new();
        args.insert("query".into(), serde_json::json!(query));
        let mut call = rmcp::model::CallToolRequestParams::new("search_contacts");
        call.arguments = Some(args);
        Message::assistant().with_tool_request(id, Ok(call))
    }

    fn kim_host(messages: Vec<Message>) -> AgentHost {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        AgentHost::from_provider_for_test(
            profile,
            Arc::new(ScriptedProvider::new(messages)),
            PathBuf::from("/tmp"),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn search_contacts_yields_then_complete_finishes() {
        let host = kim_host(vec![kim_search_call("c1", "bob")]);
        let (tx, mut rx) = mpsc::channel(32);
        let outcome = host
            .prompt("s", "find bob", tx, CancellationToken::new())
            .await
            .unwrap();
        match outcome {
            TurnOutcome::Yielded { call_id, name, .. } => {
                assert_eq!(call_id, "c1");
                assert_eq!(name, "search_contacts");
            }
            other => panic!("expected yield, got {other:?}"),
        }
        let pending = host.pending_yields("s").await;
        assert_eq!(pending.len(), 1);
        while let Ok(ev) = rx.try_recv() {
            assert!(!matches!(ev, HostEvent::ToolRequest { .. }));
        }
        let (tx2, _rx2) = mpsc::channel(32);
        let done = host
            .complete_tool("s", "c1", r#"{"people":[]}"#, tx2, CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(done, TurnOutcome::Finished { .. }));
    }

    #[tokio::test]
    async fn two_tool_requests_first_complete_stays_yielded() {
        let mut args = rmcp::model::JsonObject::new();
        args.insert("query".into(), serde_json::json!("a"));
        let mut c1 = rmcp::model::CallToolRequestParams::new("search_contacts");
        c1.arguments = Some(args.clone());
        let mut c2 = rmcp::model::CallToolRequestParams::new("search_contacts");
        c2.arguments = Some(args);
        let assistant = Message::assistant()
            .with_tool_request("c1", Ok(c1))
            .with_tool_request("c2", Ok(c2));
        let host = kim_host(vec![assistant]);
        let (tx, _rx) = mpsc::channel(32);
        let outcome = host
            .prompt("s", "find", tx, CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(outcome, TurnOutcome::Yielded { .. }));
        assert_eq!(host.pending_yields("s").await.len(), 2);
        let (tx2, _rx2) = mpsc::channel(32);
        let mid = host
            .complete_tool("s", "c1", r#"{"people":[]}"#, tx2, CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(mid, TurnOutcome::Yielded { call_id, .. } if call_id == "c2"));
        let (tx3, _rx3) = mpsc::channel(32);
        let done = host
            .complete_tool("s", "c2", r#"{"people":[]}"#, tx3, CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(done, TurnOutcome::Finished { .. }));
    }

    #[tokio::test]
    async fn context_injection_does_not_replace_kickoff() {
        let host = kim_host(vec![Message::assistant().with_text("ok")]);
        let (tx, _rx) = mpsc::channel(32);
        host.prompt_with_context(
            "s",
            "real question",
            Some("old line 1\nold line 2"),
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let conv = host.conversation_for_test("s").await;
        let kickoff = goose_agent::operation::messages_since_kickoff(&conv).unwrap();
        assert_eq!(kickoff[0].as_concat_text(), "real question");
        assert!(kickoff[0].is_user_visible());
    }
}
