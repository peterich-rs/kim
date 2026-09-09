//! Local Goose-backed agent host.
//!
//! KIM owns identity, @mention routing, and IM send/receive. This crate
//! assembles Goose's unrolled loop (`goose-agent`) with `goose-providers`
//! (OpenAI Completions/Responses and Anthropic Messages).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::inference::{InferenceEffect, InferenceRunner};
use goose_agent::machine::{EffectHandler, MachineSession, SessionLoader, StateMachine, Step};
use goose_agent::operation::{Emitter, MachineEffect, Operation};
use goose_provider_types::base::Provider;
use goose_provider_types::conversation::message::{Message, MessageContent};
use goose_provider_types::conversation::token_usage::ProviderUsage;
use goose_provider_types::conversation::Conversation;
use goose_provider_types::model::ModelConfig;
use goose_providers::api_client::{ApiClient, AuthMethod};
use goose_providers::openai::{
    ensure_url_scheme, OpenAiProviderBuilder, OPEN_AI_DEFAULT_BASE_PATH,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

/// Built-in persona: mention `@助手` or `@goose` in an IM thread.
pub const DEFAULT_AGENT_ID: &str = "goose";
pub const DEFAULT_AGENT_NAME: &str = "助手";
pub const DEFAULT_SYSTEM_PROMPT: &str =
    "You are 助手, a local desktop agent inside the KIM messenger. \
You run on the user's machine (not a cloud bot). Reply in the user's language. \
Be concise. You can see the current conversation because the host pasted it into this session. \
Do not claim you have tools you were not given.";

const DEFAULT_OPENAI_HOST: &str = "https://api.openai.com";
const DEFAULT_ANTHROPIC_HOST: &str = "https://api.anthropic.com";

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("api key missing")]
    MissingApiKey,
    #[error("unknown provider {0}")]
    UnknownProvider(String),
    #[error("invalid provider url: {0}")]
    InvalidUrl(String),
    #[error("session {0} is busy")]
    Busy(String),
    #[error("{0}")]
    Failed(String),
}

impl From<anyhow::Error> for HostError {
    fn from(err: anyhow::Error) -> Self {
        HostError::Failed(err.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
}

impl ProviderKind {
    pub fn parse(raw: &str) -> Result<Self, HostError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "openai" | "responses_http" | "live" | "responses" | "scripted" | "" => {
                Ok(Self::OpenAi)
            }
            "anthropic" | "messages" => Ok(Self::Anthropic),
            other => Err(HostError::UnknownProvider(other.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl ProviderConfig {
    pub fn openai(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            kind: ProviderKind::OpenAi,
            base_url: format!("{DEFAULT_OPENAI_HOST}/v1"),
            api_key: api_key.into(),
            model: model.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum HostEvent {
    TextDelta { delta: String },
    Finished { text: String },
    Failed { message: String },
}

#[derive(Clone)]
struct HostSession {
    id: String,
    conversation: Conversation,
}

impl MachineSession for HostSession {
    fn id(&self) -> &str {
        &self.id
    }

    fn conversation(&self) -> Option<&Conversation> {
        Some(&self.conversation)
    }
}

#[derive(Clone)]
enum HostEffect {
    Append(Message),
    Usage(#[allow(dead_code)] ProviderUsage),
}

impl From<Message> for HostEffect {
    fn from(message: Message) -> Self {
        Self::Append(message)
    }
}

impl InferenceEffect for HostEffect {
    fn record_usage(usage: ProviderUsage) -> Self {
        Self::Usage(usage)
    }
}

impl MachineEffect for HostEffect {
    fn ensure_message_ids(&mut self) {
        if let Self::Append(message) = self {
            if message.id.is_none() {
                message.id = Some(format!("msg_{}", uuid::Uuid::new_v4()));
            }
        }
    }
}

struct SystemPromptOp {
    prompt: String,
}

#[async_trait]
impl Operation<HostSession, HostEffect> for SystemPromptOp {
    fn name(&self) -> &'static str {
        "system_prompt"
    }

    async fn prompt_parts(
        &self,
        _session: &HostSession,
        _conversation: &Conversation,
    ) -> Result<Vec<(String, String)>> {
        Ok(vec![("system".into(), self.prompt.clone())])
    }
}

struct Store {
    conversations: HashMap<String, Conversation>,
    busy: HashMap<String, bool>,
}

struct Inner {
    provider: Arc<dyn Provider>,
    model: ModelConfig,
    system_prompt: String,
    store: Mutex<Store>,
}

#[derive(Clone)]
pub struct AgentHost {
    inner: Arc<Inner>,
}

impl AgentHost {
    pub fn new(config: ProviderConfig) -> Result<Self, HostError> {
        let provider = build_provider(&config)?;
        let model = model_config(&config.model);
        Ok(Self {
            inner: Arc::new(Inner {
                provider,
                model,
                system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
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

        let inference = InferenceRunner::<HostSession, HostEffect>::new(
            Arc::clone(&self.inner.provider),
            self.inner.model.clone(),
        );
        let system = SystemPromptOp {
            prompt: self.inner.system_prompt.clone(),
        };
        let machine = StateMachine::new(
            vec![
                Step::Operation(Arc::new(system)),
                Step::Inference(Arc::new(inference)),
            ],
            cancel,
        );

        let outcome = machine.run(self, session_id, &emit).await;
        drop(emit);
        let _ = pump.await;

        match outcome {
            Ok(session) => {
                let text = last_assistant_text(&session.conversation);
                Ok(text)
            }
            Err(err) => Err(HostError::Failed(err.to_string())),
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
                HostEffect::Append(message) => conversation.push(message.clone()),
                HostEffect::Usage(_) => {}
            }
        }
        Ok(())
    }
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

fn model_config(model: &str) -> ModelConfig {
    let name = if model.is_empty() || model == "scripted" {
        "gpt-4o"
    } else {
        model
    };
    ModelConfig::new(name)
}

fn build_provider(config: &ProviderConfig) -> Result<Arc<dyn Provider>, HostError> {
    if config.api_key.trim().is_empty() {
        return Err(HostError::MissingApiKey);
    }
    match config.kind {
        ProviderKind::OpenAi => Ok(Arc::new(build_openai(config)?)),
        ProviderKind::Anthropic => Ok(Arc::new(build_anthropic(config)?)),
    }
}

fn build_openai(
    config: &ProviderConfig,
) -> Result<goose_providers::openai::OpenAiProvider, HostError> {
    let (host, base_path) = split_openai_url(&config.base_url)?;
    let client = ApiClient::with_timeout_and_tls(
        host,
        AuthMethod::BearerToken(config.api_key.clone()),
        Duration::from_secs(120),
        None,
    )
    .map_err(|e| HostError::Failed(e.to_string()))?;
    Ok(OpenAiProviderBuilder::new(client)
        .base_path(base_path)
        .supports_streaming(true)
        .build())
}

fn build_anthropic(
    config: &ProviderConfig,
) -> Result<goose_providers::anthropic::AnthropicProvider, HostError> {
    let host = anthropic_host(&config.base_url)?;
    let client = ApiClient::with_timeout_and_tls(
        host,
        AuthMethod::ApiKey {
            header_name: "x-api-key".into(),
            key: config.api_key.clone(),
        },
        Duration::from_secs(120),
        None,
    )
    .map_err(|e| HostError::Failed(e.to_string()))?;
    Ok(goose_providers::anthropic::AnthropicProviderBuilder::new(client).build())
}

fn split_openai_url(raw: &str) -> Result<(String, String), HostError> {
    let fallback = (
        DEFAULT_OPENAI_HOST.to_string(),
        OPEN_AI_DEFAULT_BASE_PATH.to_string(),
    );
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(fallback);
    }
    let url = url::Url::parse(&ensure_url_scheme(trimmed))
        .map_err(|e| HostError::InvalidUrl(e.to_string()))?;
    let host = url[..url::Position::BeforePath].to_string();
    if host.is_empty() {
        return Ok(fallback);
    }
    Ok((host, OPEN_AI_DEFAULT_BASE_PATH.to_string()))
}

fn anthropic_host(raw: &str) -> Result<String, HostError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(DEFAULT_ANTHROPIC_HOST.to_string());
    }
    let url = url::Url::parse(&ensure_url_scheme(trimmed))
        .map_err(|e| HostError::InvalidUrl(e.to_string()))?;
    let host = url[..url::Position::BeforePath].to_string();
    if host.is_empty() {
        Ok(DEFAULT_ANTHROPIC_HOST.to_string())
    } else {
        Ok(host)
    }
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
    fn provider_kind_aliases() {
        assert_eq!(ProviderKind::parse("openai").unwrap(), ProviderKind::OpenAi);
        assert_eq!(
            ProviderKind::parse("anthropic").unwrap(),
            ProviderKind::Anthropic
        );
        assert_eq!(
            ProviderKind::parse("responses_http").unwrap(),
            ProviderKind::OpenAi
        );
        assert!(ProviderKind::parse("unknown").is_err());
    }

    #[test]
    fn missing_key_is_explicit() {
        let result = AgentHost::new(ProviderConfig::openai("", "gpt-4o"));
        assert!(matches!(result, Err(HostError::MissingApiKey)));
    }
}
