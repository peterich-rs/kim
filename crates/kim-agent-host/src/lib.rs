//! Local Goose-backed agent host.
//!
//! KIM owns identity, @mention routing, and IM send/receive. This crate
//! assembles Goose's unrolled loop (`goose-agent`) with `goose-providers`
//! (OpenAI Completions/Responses and Anthropic Messages).

mod catalog;
mod events;
mod machine;
mod ops;
mod profile;
mod provider;
mod scripted;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

pub use catalog::{
    catalog_surface_json, catalog_validate, catalog_vendors_json, normalize_vendor_id,
    ReasoningChoice, ReasoningSurface, VendorSummary,
};
pub use events::{HostError, HostEvent, PendingYield, TurnOutcome, YieldKind};
pub use machine::MachineFactory;
pub use ops::permission::parse_permission;
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
You have search_contacts, search_messages, get_conversation_context, list_profiles, \
send_message, and read_clipboard. send_message and clipboard require user confirmation. \
You do not have filesystem or shell access. Do not claim you have tools you were not given.";

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
    session_path: Option<PathBuf>,
    resume_on_open: bool,
}

struct Inner {
    profile: AgentProfile,
    provider: Arc<dyn Provider>,
    model: ModelConfig,
    project_root: PathBuf,
    store: Mutex<Store>,
    mcp: Arc<ops::mcp::McpHub>,
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
        mut profile: AgentProfile,
        provider: Arc<dyn Provider>,
        project_root: PathBuf,
    ) -> Result<Self, HostError> {
        profile.apply_reasoning()?;
        let model = machine::model_config(&profile.model)?;
        Ok(Self {
            inner: Arc::new(Inner {
                profile,
                provider,
                model,
                project_root,
                store: Mutex::new(Store {
                    conversations: HashMap::new(),
                    session_path: None,
                    resume_on_open: true,
                }),
                mcp: Arc::new(ops::mcp::McpHub::new()),
            }),
        })
    }

    pub fn from_resolved(resolved: ResolvedProfile) -> Result<Self, HostError> {
        if resolved.api_key.trim().is_empty() {
            return Err(HostError::MissingApiKey);
        }
        let mut resolved = resolved;
        resolved.profile.normalize_mode();
        resolved.profile.apply_reasoning()?;
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
                    session_path: None,
                    resume_on_open: true,
                }),
                mcp: Arc::new(ops::mcp::McpHub::new()),
            }),
        })
    }

    pub async fn configure_persist(&self, path: Option<PathBuf>, resume_on_open: bool) {
        let mut store = self.inner.store.lock().await;
        store.session_path = path;
        store.resume_on_open = resume_on_open;
    }

    pub async fn connect_extensions(&self) -> Result<(), HostError> {
        self.inner
            .mcp
            .connect(&self.inner.profile.extensions, &self.inner.project_root)
            .await
    }

    pub async fn disconnect_extensions(&self) {
        self.inner.mcp.disconnect().await;
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
            let conversation = ensure_conversation(&mut store, session_id);
            if let Some(ctx) = context_json.map(str::trim).filter(|s| !s.is_empty()) {
                let preview = truncate_chars(ctx, 8 * 1024);
                conversation.push(
                    Message::user()
                        .with_text(preview)
                        .with_visibility(false, true),
                );
            }
            conversation.push(Message::user().with_text(text));
            persist_session(&store, session_id)?;
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
            if !ops::permission::unanswered_confirmations(conversation).is_empty() {
                return Err(HostError::UnknownToolCall(call_id.to_string()));
            }
            let pending = kim_pending(conversation, &self.inner.profile.tools);
            if !pending.iter().any(|p| p.call_id == call_id) {
                return Err(HostError::UnknownToolCall(call_id.to_string()));
            }
            let result = parse_tool_output(output_json);
            let message = Message::user().with_tool_response(call_id.to_string(), Ok(result));
            conversation.push(message);
            persist_session(&store, session_id)?;
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

    pub async fn respond_permission(
        &self,
        session_id: &str,
        call_id: &str,
        permission: goose_provider_types::permission::Permission,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError> {
        {
            let mut store = self.inner.store.lock().await;
            let conversation = store
                .conversations
                .get_mut(session_id)
                .ok_or_else(|| HostError::UnknownSession(session_id.to_string()))?;
            let pending = ops::permission::unanswered_confirmations(conversation);
            if !pending.iter().any(|p| p.call_id == call_id) {
                return Err(HostError::UnknownToolCall(call_id.to_string()));
            }
            conversation.push(ops::permission::confirmation_response_message(
                call_id, permission,
            ));
            persist_session(&store, session_id)?;
        }
        let remaining = self.pending_yields(session_id).await;
        if let Some(first) = remaining
            .iter()
            .find(|p| p.kind == YieldKind::ActionRequired)
        {
            return Ok(TurnOutcome::Yielded {
                kind: YieldKind::ActionRequired,
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
            .map(|c| session_pending(c, &self.inner.profile.tools))
            .unwrap_or_default()
    }

    pub async fn cancel_pending_tools(&self, session_id: &str) {
        let mut store = self.inner.store.lock().await;
        let Some(conversation) = store.conversations.get_mut(session_id) else {
            return;
        };
        let confirmations = ops::permission::unanswered_confirmations(conversation);
        let pending = kim_pending(conversation, &self.inner.profile.tools);
        if confirmations.is_empty() && pending.is_empty() {
            return;
        }
        let mut message = Message::user();
        for p in &confirmations {
            message =
                message.with_content(MessageContent::action_required_tool_confirmation_response(
                    p.call_id.clone(),
                    goose_provider_types::permission::Permission::Cancel,
                ));
        }
        let mut seen = std::collections::HashSet::new();
        for p in confirmations.into_iter().chain(pending) {
            if !seen.insert(p.call_id.clone()) {
                continue;
            }
            message.add_tool_response_with_metadata(
                p.call_id,
                Ok(rmcp::model::CallToolResult::error(vec![
                    rmcp::model::ContentBlock::text("cancelled"),
                ])),
                None,
            );
        }
        conversation.push(message);
        let _ = persist_session(&store, session_id);
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
            Arc::clone(&self.inner.mcp),
        );
        let machine = StateMachine::new(steps, cancel);

        let outcome = machine.run(self, session_id, &emit).await;
        drop(emit);
        let _ = pump.await;

        match outcome {
            Ok(session) => {
                let pending = session_pending(&session.conversation, &self.inner.profile.tools);
                if let Some(first) = pending.first() {
                    tracing::info!(
                        session_id,
                        outcome = "yielded",
                        tool_name = %first.name,
                        "agent turn end"
                    );
                    Ok(TurnOutcome::Yielded {
                        kind: first.kind,
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
        let mut store = self.inner.store.lock().await;
        let conversation = ensure_conversation(&mut store, session_id).clone();
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
        persist_session(&store, &session.id)?;
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

#[derive(serde::Serialize, serde::Deserialize)]
struct SessionDisk {
    v: u32,
    messages: Vec<Message>,
}

fn ensure_conversation<'a>(store: &'a mut Store, session_id: &str) -> &'a mut Conversation {
    let path = store.session_path.clone();
    let resume = store.resume_on_open;
    store
        .conversations
        .entry(session_id.to_string())
        .or_insert_with(|| match (path.as_ref(), resume) {
            (Some(p), true) if p.exists() => read_session(p),
            _ => Conversation::empty(),
        })
}

fn persist_session(store: &Store, session_id: &str) -> Result<(), HostError> {
    let Some(path) = store.session_path.as_ref() else {
        return Ok(());
    };
    let Some(conversation) = store.conversations.get(session_id) else {
        return Ok(());
    };
    write_session(path, conversation)
}

fn write_session(path: &Path, conversation: &Conversation) -> Result<(), HostError> {
    let disk = SessionDisk {
        v: 1,
        messages: conversation.messages().clone(),
    };
    let bytes = serde_json::to_vec(&disk).map_err(|e| HostError::Failed(e.to_string()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| HostError::Failed(e.to_string()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| HostError::Failed(e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| HostError::Failed(e.to_string()))?;
    Ok(())
}

fn read_session(path: &Path) -> Conversation {
    let Ok(bytes) = std::fs::read(path) else {
        return Conversation::empty();
    };
    let Ok(disk) = serde_json::from_slice::<SessionDisk>(&bytes) else {
        return Conversation::empty();
    };
    Conversation::new_unvalidated(disk.messages)
}

fn session_pending(conversation: &Conversation, tools: &ToolSet) -> Vec<PendingYield> {
    let confirmations = ops::permission::unanswered_confirmations(conversation);
    if !confirmations.is_empty() {
        return confirmations;
    }
    kim_pending(conversation, tools)
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
                kind: YieldKind::ToolRequest,
                prompt: String::new(),
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

pub fn session_file_from_sqlite_path(sqlite_path: &str) -> Option<PathBuf> {
    let trimmed = sqlite_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = Path::new(trimmed);
    if path.is_absolute() || trimmed.contains('/') || trimmed.contains('\\') {
        Some(path.to_path_buf())
    } else {
        None
    }
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
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_fs_tools: true,
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
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

    fn send_host(messages: Vec<Message>) -> AgentHost {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            enable_approvals: true,
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

    fn send_call(id: &str, dest: &str, text: &str) -> Message {
        let mut args = rmcp::model::JsonObject::new();
        args.insert("dest".into(), serde_json::json!(dest));
        args.insert("text".into(), serde_json::json!(text));
        let mut call = rmcp::model::CallToolRequestParams::new("send_message");
        call.arguments = Some(args);
        Message::assistant().with_tool_request(id, Ok(call))
    }

    #[tokio::test]
    async fn send_message_smart_approve_yields_action_required() {
        let host = send_host(vec![send_call("c1", "bob", "hi")]);
        let (tx, mut rx) = mpsc::channel(32);
        let outcome = host
            .prompt("s", "ping bob", tx, CancellationToken::new())
            .await
            .unwrap();
        match outcome {
            TurnOutcome::Yielded {
                kind: YieldKind::ActionRequired,
                call_id,
                name,
            } => {
                assert_eq!(call_id, "c1");
                assert_eq!(name, "send_message");
            }
            other => panic!("expected action required, got {other:?}"),
        }
        while let Ok(ev) = rx.try_recv() {
            assert!(!matches!(ev, HostEvent::ActionRequired { .. }));
            assert!(!matches!(ev, HostEvent::ToolRequest { .. }));
        }
        let pending = host.pending_yields("s").await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].kind, YieldKind::ActionRequired);
    }

    #[tokio::test]
    async fn allow_once_then_deferred_kim_yields() {
        let host = send_host(vec![send_call("c1", "bob", "hi")]);
        let (tx, _rx) = mpsc::channel(32);
        host.prompt("s", "ping bob", tx, CancellationToken::new())
            .await
            .unwrap();
        let (tx2, _rx2) = mpsc::channel(32);
        let mid = host
            .respond_permission(
                "s",
                "c1",
                goose_provider_types::permission::Permission::AllowOnce,
                tx2,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        match mid {
            TurnOutcome::Yielded {
                kind: YieldKind::ToolRequest,
                call_id,
                name,
            } => {
                assert_eq!(call_id, "c1");
                assert_eq!(name, "send_message");
            }
            other => panic!("expected tool yield, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn deny_finishes_with_user_rejected() {
        let host = send_host(vec![send_call("c1", "bob", "hi")]);
        let (tx, _rx) = mpsc::channel(32);
        host.prompt("s", "ping bob", tx, CancellationToken::new())
            .await
            .unwrap();
        let (tx2, _rx2) = mpsc::channel(32);
        let done = host
            .respond_permission(
                "s",
                "c1",
                goose_provider_types::permission::Permission::DenyOnce,
                tx2,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(matches!(done, TurnOutcome::Finished { .. }));
        let conv = host.conversation_for_test("s").await;
        let dump = format!("{conv:?}");
        assert!(
            dump.contains("user-rejected"),
            "conversation should record the rejection: {dump}"
        );
    }

    #[tokio::test]
    async fn session_json_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        let host = kim_host(vec![Message::assistant().with_text("hello")]);
        host.configure_persist(Some(path.clone()), true).await;
        let (tx, _rx) = mpsc::channel(32);
        host.prompt("s", "hi", tx, CancellationToken::new())
            .await
            .unwrap();
        assert!(path.exists());
        let host2 = kim_host(vec![Message::assistant().with_text("again")]);
        host2.configure_persist(Some(path.clone()), true).await;
        let (tx2, _rx2) = mpsc::channel(32);
        host2
            .prompt("s", "next", tx2, CancellationToken::new())
            .await
            .unwrap();
        let conv = host2.conversation_for_test("s").await;
        let texts: Vec<_> = conv.messages().iter().map(|m| m.as_concat_text()).collect();
        assert!(texts.iter().any(|t| t == "hi"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "next"), "{texts:?}");
    }

    #[tokio::test]
    async fn path_without_slash_does_not_write() {
        let host = kim_host(vec![Message::assistant().with_text("ok")]);
        host.configure_persist(None, true).await;
        let (tx, _rx) = mpsc::channel(32);
        host.prompt("goose", "hi", tx, CancellationToken::new())
            .await
            .unwrap();
        assert!(!Path::new("goose").exists());
        assert!(!Path::new("goose.json").exists());
    }

    #[tokio::test]
    async fn compaction_replaces_oversized_conversation() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        let host = AgentHost::from_provider_for_test(
            profile,
            Arc::new(
                ScriptedProvider::new(vec![Message::assistant().with_text("ok")])
                    .with_context_limit(8),
            ),
            PathBuf::from("/tmp"),
        )
        .unwrap();
        let (tx, _rx) = mpsc::channel(32);
        host.prompt_with_context(
            "s",
            "q",
            Some(&"x".repeat(400)),
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let conv = host.conversation_for_test("s").await;
        let blob: String = conv.messages().iter().map(|m| m.as_concat_text()).collect();
        assert!(blob.contains("conversation summary"), "{blob}");
        assert!(blob.contains("q"), "{blob}");
        assert!(
            blob.matches('x').count() < 400,
            "summary should truncate older context: {blob}"
        );
    }

    #[test]
    fn sqlite_path_without_slash_is_not_a_file() {
        assert!(session_file_from_sqlite_path("goose").is_none());
        assert!(session_file_from_sqlite_path("thread-1").is_none());
        assert!(session_file_from_sqlite_path("/tmp/s.json").is_some());
        assert!(session_file_from_sqlite_path("agent/sessions/s.json").is_some());
    }
}
