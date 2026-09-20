//! Local Goose-backed agent host.
//!
//! KIM owns identity, @mention routing, and IM send/receive. This crate
//! assembles Goose's unrolled loop (`goose-agent`) with `goose-providers`
//! (OpenAI Completions/Responses and Anthropic Messages).

pub mod capability;
mod catalog;
#[cfg(feature = "codex")]
mod codex_drive;
#[cfg(feature = "codex")]
mod codex_inject;
#[cfg(feature = "codex")]
mod codex_rt;
mod events;
mod harness;
mod machine;
mod ops;
mod profile;
mod provider;
mod scripted;
mod skills;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::machine::{EffectHandler, MachineSession, SessionLoader};
use goose_agent::operation::{ConversationEffect, Emitter};
use goose_provider_types::base::Provider;
use goose_provider_types::conversation::message::{Message, MessageContent};
use goose_provider_types::conversation::token_usage::ProviderUsage;
use goose_provider_types::conversation::Conversation;
use goose_provider_types::model::ModelConfig;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

pub use capability::{
    preview_assembled, AssembledPreview, CapabilityRef, PermissionMatch, PermissionRule,
    PreviewTool, RiskTier,
};
pub use catalog::{
    catalog_surface_json, catalog_validate, catalog_vendors_json, default_context_tokens,
    normalize_vendor_id, ReasoningChoice, ReasoningChoiceBody, ReasoningSurface, VendorSummary,
    DEFAULT_CONTEXT_TOKENS,
};
pub use events::{
    CancelReason, HostError, HostEvent, PendingYield, ProviderFail, TimeoutKind, TurnOutcome,
    YieldKind,
};
pub use harness::{resolve_limits, HarnessLimits};
pub use machine::MachineFactory;
pub use ops::permission::parse_permission;
pub use ops::skill::{activate_skill_tool, ACTIVATE_SKILL};
pub use profile::{
    builtin_templates, parse_goose_mode, parse_thinking_effort, AgentProfile, ExtensionSpec,
    HarnessSpec, LegacyOpenOpts, ModelSpec, PermissionConfig, PermissionDefault, ProviderSpec,
    ResolvedProfile, SandboxMode, SandboxPolicy, ToolSet, WorkspaceKind, WorkspaceSpec,
};
pub use provider::{
    bundled_declarative_json, bundled_provider_summaries, fetch_models, BundledProviderSummary,
    ProviderConfig, ProviderKind, SessionKeyResolver,
};
pub use scripted::ScriptedProvider;
pub use skills::{
    activate, build_registry, bundled_ids, catalog_prompt_block, parse_skill_md, read_agents_md,
    scan_portable, skill_app_catalog_json, skill_portable_list_json, Activation, PortableSkill,
    RegistryScan, SkillClass, SkillDoc, SkillEntry, SkillError, SkillMeta, SkillPackage, SkillRef,
    SkillRegistry, SkillResolver, SkillSource,
};

#[cfg(feature = "codex")]
pub use codex_drive::CodexLaunch;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentRuntime {
    Goose,
    Codex,
}

pub(crate) use events::HostEffect;

/// Built-in persona: mention `@助手` or `@goose` in an IM thread.
pub const DEFAULT_AGENT_ID: &str = "goose";
pub const DEFAULT_AGENT_NAME: &str = "助手";
/// Identity-only fallback. Tool lists belong in the generated capability digest.
/// `{display_name}` / `{model_name}` are interpolated at assemble and subagent spawn.
pub const DEFAULT_IDENTITY_PROMPT: &str = "\
You are {display_name}, a personal agent inside the KIM messenger. You run locally
on the user's desktop machine, not in a cloud service. You are powered by
{model_name} via the user's own provider account.

# How you work

## Identity and language
- Reply in the user's language (default to Chinese when mixed).
- You are a contact in the user's chat list. Conversations are casual and
  ongoing, like chatting with a colleague — not one-shot CLI commands.

## Task execution
- Persist until the task is done within the current turn: do not stop at
  analysis or partial results when tools could finish the job.
- Prefer answering with evidence from tools over guessing from memory.
- If a tool call fails, adjust the approach; do not retry the exact same call.
- If the user denies a confirmation request, never re-issue the same call.
  Change your approach or ask what they prefer.
- Fix root causes, not symptoms. Do not make unrelated changes along the way.

## Confirmations
- Some tools are gated: before they run, the user sees a confirmation card.
  This is normal — call the tool and let the gate do its job; never ask the
  user to \"disable approvals\".
- Actions that reach outside this machine or are hard to undo (sending
  messages to others, writing files outside the workspace, deleting things)
  always warrant extra care. State what you are about to do and why.

## Communication
- Be concise; match the user's tone. IM bubbles are read on phones too —
  prefer short paragraphs over walls of text.
- Use Markdown sparingly: bullets and bold for structure, code fences for
  code. No emojis unless the user uses them first.
- When a task spans multiple tool calls, narrate briefly between steps
  (one short sentence) so the user knows where things stand.
- When you finish, lead with the outcome, then at most a few lines of detail.
  Suggest a next step only when one is natural.

## Honesty and boundaries
- Only use tools that appear in your tool list for this session. Never claim
  a capability you were not granted — if the user asks for something you
  cannot do here, say so and suggest what they could enable.
- Never fabricate URLs, file paths, message contents, or tool results.
- If you are unsure whether something is true, check with a tool or say
  you are unsure.";
/// Deprecated alias: identity only (no tool laundry list). Prefer `DEFAULT_IDENTITY_PROMPT`.
pub const DEFAULT_SYSTEM_PROMPT: &str = DEFAULT_IDENTITY_PROMPT;

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

pub struct HostRuntimeState {
    pub conversations: HashMap<String, Conversation>,
    pub session_path: Option<PathBuf>,
    pub resume_on_open: bool,
}

struct Inner {
    profile: RwLock<AgentProfile>,
    provider: Arc<dyn Provider>,
    model: ModelConfig,
    project_root: PathBuf,
    store: Mutex<Store>,
    mcp: Arc<ops::mcp::McpHub>,
    provider_generation: AtomicU64,
    limits: RwLock<HarnessLimits>,
    busy: std::sync::Mutex<HashSet<String>>,
    hard_remaining: std::sync::Mutex<HashMap<String, Duration>>,
    last_usage: std::sync::Mutex<HashMap<String, ProviderUsage>>,
    steer: std::sync::Mutex<harness::SteerInbox>,
    turn_tx: std::sync::Mutex<Option<mpsc::Sender<HostEvent>>>,
    idle: std::sync::Mutex<Option<Arc<harness::IdleClock>>>,
    active_cancel: std::sync::Mutex<Option<CancellationToken>>,
    input_tokens: Arc<AtomicU64>,
    codex_runtime: AtomicU8,
    #[cfg(feature = "codex")]
    codex: Mutex<codex_drive::CodexSlot>,
}

fn blank_inner(
    profile: AgentProfile,
    provider: Arc<dyn Provider>,
    model: ModelConfig,
    project_root: PathBuf,
) -> Inner {
    harness::install_panic_hook();
    Inner {
        profile: RwLock::new(profile),
        provider,
        model,
        project_root,
        store: Mutex::new(Store {
            conversations: HashMap::new(),
            session_path: None,
            resume_on_open: true,
        }),
        mcp: Arc::new(ops::mcp::McpHub::new()),
        provider_generation: AtomicU64::new(1),
        limits: RwLock::new(HarnessLimits::default()),
        busy: std::sync::Mutex::new(HashSet::new()),
        hard_remaining: std::sync::Mutex::new(HashMap::new()),
        last_usage: std::sync::Mutex::new(HashMap::new()),
        steer: std::sync::Mutex::new(harness::SteerInbox::default()),
        turn_tx: std::sync::Mutex::new(None),
        idle: std::sync::Mutex::new(None),
        active_cancel: std::sync::Mutex::new(None),
        input_tokens: Arc::new(AtomicU64::new(0)),
        codex_runtime: AtomicU8::new(0),
        #[cfg(feature = "codex")]
        codex: Mutex::new(codex_drive::CodexSlot::new()),
    }
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
            inner: Arc::new(blank_inner(profile, provider, model, project_root)),
        })
    }

    pub fn from_spec(
        profile: &AgentProfile,
        api_key: &str,
        project_root: PathBuf,
    ) -> Result<Self, HostError> {
        Self::from_resolved(ResolvedProfile {
            profile: profile.clone(),
            api_key: api_key.to_string(),
            project_root,
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
            inner: Arc::new(blank_inner(
                resolved.profile,
                provider,
                model,
                resolved.project_root,
            )),
        })
    }

    pub fn profile_snapshot(&self) -> AgentProfile {
        self.inner
            .profile
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn provider_generation(&self) -> u64 {
        self.inner.provider_generation.load(Ordering::Relaxed)
    }

    pub fn replace_prompt_steer(&self, system_prompt: String, steer: String) {
        let mut profile = self
            .inner
            .profile
            .write()
            .unwrap_or_else(|e| e.into_inner());
        profile.system_prompt = system_prompt;
        profile.steer = steer;
    }

    pub async fn runtime_state(&self) -> HostRuntimeState {
        let store = self.inner.store.lock().await;
        HostRuntimeState {
            conversations: store.conversations.clone(),
            session_path: store.session_path.clone(),
            resume_on_open: store.resume_on_open,
        }
    }

    pub async fn restore_runtime_state(&self, state: HostRuntimeState) {
        let mut store = self.inner.store.lock().await;
        store.conversations = state.conversations;
        store.session_path = state.session_path;
        store.resume_on_open = state.resume_on_open;
    }

    pub async fn configure_persist(&self, path: Option<PathBuf>, resume_on_open: bool) {
        let mut store = self.inner.store.lock().await;
        store.session_path = path;
        store.resume_on_open = resume_on_open;
    }

    pub async fn connect_extensions(&self) -> Result<(), HostError> {
        let extensions = self.profile_snapshot().project_extensions();
        self.inner
            .mcp
            .connect(&extensions, &self.inner.project_root)
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
        if text.len() > self.limits().prompt_bytes {
            return Err(HostError::Failed("prompt exceeds byte cap".into()));
        }
        #[cfg(feature = "codex")]
        if self.is_codex() {
            let _busy = self.acquire_session(session_id)?;
            self.reset_hard_budget(session_id);
            return self.codex_prompt(text, context_json, events, cancel).await;
        }
        let _busy = self.acquire_session(session_id)?;
        self.reset_hard_budget(session_id);

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
        #[cfg(feature = "codex")]
        if self.is_codex() {
            let _busy = self.acquire_session(session_id)?;
            return self
                .codex_complete_tool(call_id, output_json, events, cancel)
                .await;
        }
        let _busy = self.acquire_session(session_id)?;
        {
            let mut store = self.inner.store.lock().await;
            let conversation = store
                .conversations
                .get_mut(session_id)
                .ok_or_else(|| HostError::UnknownSession(session_id.to_string()))?;
            if !ops::permission::unanswered_confirmations(conversation).is_empty() {
                return Err(HostError::UnknownToolCall(call_id.to_string()));
            }
            let tools = self.profile_snapshot().project_toolset();
            let pending = kim_pending(conversation, &tools);
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
        #[cfg(feature = "codex")]
        if self.is_codex() {
            let _busy = self.acquire_session(session_id)?;
            return self
                .codex_respond_permission(call_id, permission, events, cancel)
                .await;
        }
        let _busy = self.acquire_session(session_id)?;
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
        #[cfg(feature = "codex")]
        if self.is_codex() {
            return self.codex_pending().await;
        }
        let store = self.inner.store.lock().await;
        store
            .conversations
            .get(session_id)
            .map(|c| session_pending(c, &self.profile_snapshot().project_toolset()))
            .unwrap_or_default()
    }

    pub async fn cancel_pending_tools(&self, session_id: &str) {
        #[cfg(feature = "codex")]
        if self.is_codex() {
            self.codex_cancel_pending().await;
            return;
        }
        self.repair_pairing(session_id).await;
    }

    pub fn is_codex(&self) -> bool {
        self.inner.codex_runtime.load(Ordering::Relaxed) == 1
    }

    pub fn kim_tool_names(&self) -> Vec<&'static str> {
        self.profile_snapshot().project_toolset().kim_world_names()
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
        let mut usage_event = None;
        for effect in effects.iter_mut() {
            match effect {
                HostEffect::Usage(usage) => {
                    let input = usage.usage.input_tokens.unwrap_or(0).max(0) as u64;
                    let output = usage.usage.output_tokens.unwrap_or(0).max(0) as u64;
                    self.inner.input_tokens.store(input, Ordering::Relaxed);
                    if let Ok(mut slot) = self.inner.last_usage.lock() {
                        slot.insert(session.id.clone(), usage.clone());
                    }
                    usage_event = Some(HostEvent::Usage {
                        input_tokens: input,
                        output_tokens: output,
                    });
                }
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
        drop(store);
        if let Some(event) = usage_event {
            if events::is_idle_activity(&event) {
                let idle = self
                    .inner
                    .idle
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone();
                if let Some(idle) = idle {
                    idle.reset();
                }
            }
            let tx = self
                .inner
                .turn_tx
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clone();
            if let Some(tx) = tx {
                let _ = tx.try_send(event);
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

pub(crate) async fn pump_message(
    events: &mpsc::Sender<HostEvent>,
    message: &Message,
    idle: Option<&harness::IdleClock>,
) {
    let delta = message_text(message);
    if !delta.is_empty() {
        let event = HostEvent::TextDelta { delta };
        note_activity(idle, &event);
        let _ = events.try_send(event);
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
                let event = HostEvent::ToolRequest {
                    call_id: req.id.clone(),
                    name,
                    arguments_json,
                };
                note_activity(idle, &event);
                let _ = events.try_send(event);
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
                let event = HostEvent::ToolResult {
                    call_id: res.id.clone(),
                    name: String::new(),
                    output_preview: preview,
                    ok,
                };
                note_activity(idle, &event);
                let _ = events.try_send(event);
            }
            _ => {}
        }
    }
}

fn note_activity(idle: Option<&harness::IdleClock>, event: &HostEvent) {
    if events::is_idle_activity(event) {
        if let Some(idle) = idle {
            idle.reset();
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

pub(crate) fn ensure_conversation<'a>(
    store: &'a mut Store,
    session_id: &str,
) -> &'a mut Conversation {
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

pub(crate) fn persist_session(store: &Store, session_id: &str) -> Result<(), HostError> {
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

pub(crate) fn session_pending(conversation: &Conversation, tools: &ToolSet) -> Vec<PendingYield> {
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

#[cfg(test)]
fn last_assistant_text(conversation: &Conversation) -> String {
    if !goose_agent::operation::ends_turn(conversation.messages()) {
        return String::new();
    }
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

pub fn runtime_from_harness_json(json: &str) -> AgentRuntime {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return AgentRuntime::Goose;
    };
    match value.get("runtime").and_then(|item| item.as_str()) {
        Some("codex") => AgentRuntime::Codex,
        _ => AgentRuntime::Goose,
    }
}

pub fn codex_home_from_session_file(session_file: &Path) -> PathBuf {
    session_file
        .parent()
        .and_then(|sessions| sessions.parent())
        .map(|agent| agent.join("codex"))
        .unwrap_or_else(|| session_file.join("codex"))
}

/// Locate `kim-codex-helper`.
///
/// Order: `KIM_CODEX_HELPER`, then the sibling of this process (the macOS
/// assemble script copies the binary into `Contents/MacOS`), then a walk up
/// from the cwd and executable for a repo `target/{debug,release}` build.
pub fn resolve_codex_helper() -> Result<PathBuf, HostError> {
    let name = if cfg!(windows) {
        "kim-codex-helper.exe"
    } else {
        "kim-codex-helper"
    };
    if let Some(raw) = std::env::var_os("KIM_CODEX_HELPER") {
        let path = PathBuf::from(raw);
        if path.is_file() {
            return Ok(absolute_file(&path));
        }
        return Err(HostError::Failed(format!(
            "codex helper executable is not configured ({})",
            path.display()
        )));
    }
    if let Some(path) = helper_beside_exe(name) {
        return Ok(path);
    }
    for root in helper_search_roots() {
        for profile in ["debug", "release"] {
            let candidate = root.join("target").join(profile).join(name);
            if candidate.is_file() {
                return Ok(absolute_file(&candidate));
            }
        }
        let sibling = root.join(name);
        if sibling.is_file() {
            return Ok(absolute_file(&sibling));
        }
    }
    Err(HostError::Failed(
        "codex helper executable is not configured".into(),
    ))
}

pub fn linux_sandbox_beside(helper: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let path = helper.parent()?.join("codex-linux-sandbox");
        path.is_file().then(|| absolute_file(&path))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = helper;
        None
    }
}

fn absolute_file(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn helper_beside_exe(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let sibling = exe.parent()?.join(name);
    sibling.is_file().then(|| absolute_file(&sibling))
}

fn helper_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        push_ancestors(&mut roots, cwd, 8);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            push_ancestors(&mut roots, dir.to_path_buf(), 8);
        }
    }
    roots
}

fn push_ancestors(out: &mut Vec<PathBuf>, mut dir: PathBuf, depth: usize) {
    for _ in 0..depth {
        if !out.iter().any(|have| have == &dir) {
            out.push(dir.clone());
        }
        if !dir.pop() {
            break;
        }
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
    fn harness_json_runtime_defaults_to_goose() {
        assert_eq!(runtime_from_harness_json(""), AgentRuntime::Goose);
        assert_eq!(
            runtime_from_harness_json("{\"enabled\":true}"),
            AgentRuntime::Goose
        );
        assert_eq!(
            runtime_from_harness_json("{\"enabled\":false,\"runtime\":\"codex\"}"),
            AgentRuntime::Codex
        );
    }

    #[test]
    fn last_assistant_text_ignores_earlier_progress() {
        let conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("移除登录页顶部按钮"),
            Message::assistant().with_text("接着查登录页历史。"),
            Message::assistant(),
        ]);
        assert_eq!(last_assistant_text(&conv), "");
    }

    #[test]
    fn last_assistant_text_is_ends_turn_message_only() {
        let conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("移除登录页顶部按钮"),
            Message::assistant().with_text("接着查登录页历史。"),
            Message::user().with_text("tool result"),
        ]);
        assert_eq!(last_assistant_text(&conv), "");
        let done = Conversation::new_unvalidated(vec![
            Message::user().with_text("移除登录页顶部按钮"),
            Message::assistant().with_text("接着查登录页历史。"),
            Message::user().with_text("tool result"),
            Message::assistant().with_text("按钮已经从登录页拿掉了。"),
        ]);
        assert_eq!(last_assistant_text(&done), "按钮已经从登录页拿掉了。");
    }

    #[test]
    fn missing_key_is_explicit() {
        let result = AgentHost::new(ProviderConfig::openai("", "gpt-4o"));
        assert!(matches!(result, Err(HostError::MissingApiKey)));
    }

    #[test]
    fn from_spec_delegates_to_from_resolved() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "scripted".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        let err = AgentHost::from_spec(&profile, "", PathBuf::from("/tmp"));
        assert!(matches!(err, Err(HostError::MissingApiKey)));
    }

    #[test]
    fn replace_prompt_steer_does_not_rebuild_provider() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        let host = AgentHost::from_provider_for_test(
            profile,
            Arc::new(ScriptedProvider::new(vec![])),
            PathBuf::from("/tmp"),
        )
        .unwrap();
        let gen = host.provider_generation();
        host.replace_prompt_steer("new identity".into(), "be brief".into());
        assert_eq!(host.provider_generation(), gen);
        let snap = host.profile_snapshot();
        assert_eq!(snap.system_prompt, "new identity");
        assert_eq!(snap.steer, "be brief");
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
        let mut profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            enable_approvals: true,
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        profile
            .permissions
            .tools
            .insert("send_message".into(), PermissionDefault::AskBefore);
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
        assert!(blob.contains("kim.compaction.v1"), "{blob}");
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
