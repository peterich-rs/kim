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

pub use events::{HostError, HostEvent};
pub use machine::MachineFactory;
pub use profile::{
    builtin_templates, AgentProfile, ExtensionSpec, LegacyOpenOpts, ModelSpec, PermissionConfig,
    PermissionDefault, ProviderSpec, ResolvedProfile, SandboxMode, SandboxPolicy, ToolSet,
};
pub use provider::{
    bundled_declarative_json, bundled_provider_summaries, fetch_models, BundledProviderSummary,
    ProviderConfig, ProviderKind, SessionKeyResolver,
};

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
    busy: HashMap<String, bool>,
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
                    busy: HashMap::new(),
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
    ) -> Result<String, HostError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(HostError::Failed("empty prompt".into()));
        }

        {
            let mut store = self.inner.store.lock().await;
            if store.busy.get(session_id).copied().unwrap_or(false) {
                return Err(HostError::Busy(session_id.to_string()));
            }
            store.busy.insert(session_id.to_string(), true);
            let conversation = store
                .conversations
                .entry(session_id.to_string())
                .or_insert_with(Conversation::empty);
            conversation.push(Message::user().with_text(text));
        }

        tracing::info!(session_id, "agent turn start");
        let result = self.run_loop(session_id, events, cancel).await;

        {
            let mut store = self.inner.store.lock().await;
            store.busy.insert(session_id.to_string(), false);
        }

        result
    }
}

impl AgentHost {
    async fn run_loop(
        &self,
        session_id: &str,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<String, HostError> {
        let (tx, mut rx) = mpsc::channel(64);
        let emit = Emitter::new(tx, cancel.clone());
        let pump = tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                if let goose_agent::events::AgentEvent::Message(message) = ev {
                    let delta = message_text(&message);
                    if !delta.is_empty() {
                        let _ = events.send(HostEvent::TextDelta { delta }).await;
                    }
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
                let text = last_assistant_text(&session.conversation);
                tracing::info!(session_id, outcome = "finished", "agent turn end");
                Ok(text)
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
}
