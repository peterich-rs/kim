use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_AGENT_ID: &str = "goose";
pub const DEFAULT_AGENT_NAME: &str = "助手";
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

pub const LEGACY_TOOL_LAUNDRY_IDENTITY: &str = concat!(
    "You are 助手, a local desktop agent inside the KIM messenger. ",
    "You run on the user's machine (not a cloud bot). Reply in the user's language. ",
    "Be concise. You can see the current conversation because the host pasted it into this session. ",
    "You have search_contacts, search_messages, get_conversation_context, list_profiles, ",
    "send_message, and read_clipboard. send_message and clipboard require user confirmation. ",
    "You do not have filesystem or shell access. Do not claim you have tools you were not given.",
);

pub fn is_legacy_tool_laundry_identity(prompt: &str) -> bool {
    let t = prompt.trim();
    t == LEGACY_TOOL_LAUNDRY_IDENTITY
        || (t.contains("You are 助手")
            && t.contains("search_contacts")
            && t.contains("search_messages")
            && t.contains("get_conversation_context")
            && t.contains("list_profiles")
            && t.contains("send_message")
            && t.contains("read_clipboard")
            && t.contains("You do not have filesystem or shell access"))
}

fn enabled_true() -> bool {
    true
}

fn default_smart_approve() -> ApprovalMode {
    ApprovalMode::SmartApprove
}

/// Disk and protobuf approval mode. JSON is `snake_case`, matching the historical Goose mode names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    Auto,
    Approve,
    #[default]
    SmartApprove,
    Chat,
}

impl ApprovalMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Approve => "approve",
            Self::SmartApprove => "smart_approve",
            Self::Chat => "chat",
        }
    }
}

impl FromStr for ApprovalMode {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "approve" => Ok(Self::Approve),
            "smart_approve" | "smartapprove" => Ok(Self::SmartApprove),
            "chat" => Ok(Self::Chat),
            _ => Err(()),
        }
    }
}

impl<'de> Deserialize<'de> for ApprovalMode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        ApprovalMode::from_str(&raw)
            .map_err(|()| de::Error::custom(format!("unknown approval mode {raw}")))
    }
}

pub fn parse_approval_mode(raw: &str) -> Option<ApprovalMode> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        ApprovalMode::from_str(trimmed).ok()
    }
}

/// Closed runtime selector. The persisted field stays a string so empty and `"goose"` keep their meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Goose,
    Codex,
}

/// Historical thinking-effort spellings. Serde accepts the Goose aliases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Off,
    Low,
    Medium,
    High,
    Max,
}

impl ReasoningEffort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }
}

impl FromStr for ReasoningEffort {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "disabled" | "none" => Ok(Self::Off),
            "low" => Ok(Self::Low),
            "medium" | "med" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "max" | "xhigh" => Ok(Self::Max),
            _ => Err(()),
        }
    }
}

impl<'de> Deserialize<'de> for ReasoningEffort {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        ReasoningEffort::from_str(&raw)
            .map_err(|()| de::Error::custom(format!("unknown reasoning effort {raw}")))
    }
}

pub fn parse_reasoning_effort(raw: &str) -> Option<ReasoningEffort> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        ReasoningEffort::from_str(trimmed).ok()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarnessSpec {
    pub idle_timeout_secs: Option<u64>,
    pub hard_timeout_secs: Option<u64>,
    pub bash_timeout_secs: Option<u64>,
    pub mcp_tool_timeout_secs: Option<u64>,
    pub yield_wait_secs: Option<u64>,
    pub require_reply: Option<bool>,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentProfile {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub provider: ProviderSpec,
    pub model: ModelSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningChoice>,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default = "default_smart_approve")]
    pub mode: ApprovalMode,
    #[serde(default)]
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub tools: ToolSet,
    #[serde(default)]
    pub capabilities: Vec<CapabilityRef>,
    #[serde(default)]
    pub permission_rules: Vec<PermissionRule>,
    #[serde(default)]
    pub permissions: PermissionConfig,
    #[serde(default)]
    pub sandbox: SandboxPolicy,
    #[serde(default)]
    pub extensions: Vec<ExtensionSpec>,
    #[serde(default = "enabled_true")]
    pub enabled: bool,
    #[serde(default)]
    pub steer: String,
    #[serde(default)]
    pub workspace: WorkspaceSpec,
    #[serde(default)]
    pub skills: Vec<SkillRef>,
    #[serde(default)]
    pub portable_denylist: Vec<String>,
    #[serde(default)]
    pub user_agents_skills: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub runtime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<HarnessSpec>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    #[default]
    Sandbox,
    Repo,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceSpec {
    #[serde(default)]
    pub kind: WorkspaceKind,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderSpec {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub key_ref: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModelSpec {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub thinking_effort: Option<ReasoningEffort>,
    #[serde(default)]
    pub temperature: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<i32>,
    #[serde(default)]
    pub context_tokens: Option<i32>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_params: HashMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolSet {
    #[serde(default)]
    pub send_message: bool,
    #[serde(default)]
    pub search_contacts: bool,
    #[serde(default)]
    pub search_messages: bool,
    #[serde(default)]
    pub get_conversation_context: bool,
    #[serde(default)]
    pub read_clipboard: bool,
    #[serde(default)]
    pub list_profiles: bool,
    #[serde(default)]
    pub fs: bool,
    #[serde(default)]
    pub fs_write: bool,
    #[serde(default)]
    pub bash: bool,
    #[serde(default)]
    pub subagent: bool,
}

impl ToolSet {
    pub fn kim_world_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.send_message {
            names.push("send_message");
        }
        if self.search_contacts {
            names.push("search_contacts");
        }
        if self.search_messages {
            names.push("search_messages");
        }
        if self.get_conversation_context {
            names.push("get_conversation_context");
        }
        if self.read_clipboard {
            names.push("read_clipboard");
        }
        if self.list_profiles {
            names.push("list_profiles");
        }
        names
    }

    pub fn has_in_process(&self) -> bool {
        self.fs || self.fs_write || self.bash || self.subagent
    }

    pub fn has_any(&self) -> bool {
        !self.kim_world_names().is_empty() || self.has_in_process()
    }

    pub fn from_capabilities(caps: &[CapabilityRef]) -> Self {
        let mut tools = ToolSet::default();
        for cap in caps.iter().filter(|c| c.enabled) {
            match cap.kind.as_str() {
                "im.send_message" => tools.send_message = true,
                "im.search_contacts" => tools.search_contacts = true,
                "im.search_messages" => tools.search_messages = true,
                "im.get_conversation_context" => tools.get_conversation_context = true,
                "im.read_clipboard" => tools.read_clipboard = true,
                "im.list_profiles" => tools.list_profiles = true,
                "fs" => {
                    tools.fs = true;
                    if cap
                        .params
                        .get("writable")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        tools.fs_write = true;
                    }
                }
                "bash" => tools.bash = true,
                "subagent" => tools.subagent = true,
                _ => {}
            }
        }
        tools
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDefault {
    AlwaysAllow,
    AskBefore,
    NeverAllow,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionConfig {
    #[serde(default)]
    pub tools: BTreeMap<String, PermissionDefault>,
}

impl PermissionConfig {
    pub fn default_for(name: &str, mode: ApprovalMode) -> PermissionDefault {
        if name == "activate_skill" {
            return PermissionDefault::AlwaysAllow;
        }
        match mode {
            ApprovalMode::Auto | ApprovalMode::SmartApprove => PermissionDefault::AlwaysAllow,
            ApprovalMode::Approve | ApprovalMode::Chat => PermissionDefault::AskBefore,
        }
    }

    pub fn resolve(&self, name: &str, mode: ApprovalMode) -> PermissionDefault {
        if name.contains("__") {
            if self.tools.get(name) == Some(&PermissionDefault::NeverAllow) {
                return PermissionDefault::NeverAllow;
            }
            return self
                .tools
                .get(name)
                .copied()
                .unwrap_or(PermissionDefault::AskBefore);
        }
        self.tools
            .get(name)
            .copied()
            .unwrap_or_else(|| Self::default_for(name, mode))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxPolicy {
    #[serde(default)]
    pub mode: SandboxMode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionSpec {
    pub name: String,
    pub transport: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityRef {
    pub kind: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default = "enabled_true")]
    pub enabled: bool,
}

impl CapabilityRef {
    pub fn from_legacy(tools: &ToolSet, extensions: &[ExtensionSpec]) -> Vec<Self> {
        let mut caps = Vec::new();
        let im = [
            ("send_message", "im.send_message", tools.send_message),
            (
                "search_contacts",
                "im.search_contacts",
                tools.search_contacts,
            ),
            (
                "search_messages",
                "im.search_messages",
                tools.search_messages,
            ),
            (
                "get_conversation_context",
                "im.get_conversation_context",
                tools.get_conversation_context,
            ),
            ("read_clipboard", "im.read_clipboard", tools.read_clipboard),
            ("list_profiles", "im.list_profiles", tools.list_profiles),
        ];
        for (_flag, kind, on) in im {
            if on {
                caps.push(CapabilityRef {
                    kind: kind.into(),
                    id: String::new(),
                    params: serde_json::json!({}),
                    enabled: true,
                });
            }
        }
        if tools.fs || tools.fs_write {
            caps.push(CapabilityRef {
                kind: "fs".into(),
                id: String::new(),
                params: serde_json::json!({ "writable": tools.fs_write }),
                enabled: true,
            });
        }
        if tools.bash {
            caps.push(CapabilityRef {
                kind: "bash".into(),
                id: String::new(),
                params: serde_json::json!({}),
                enabled: true,
            });
        }
        if tools.subagent {
            caps.push(CapabilityRef {
                kind: "subagent".into(),
                id: String::new(),
                params: serde_json::json!({}),
                enabled: true,
            });
        }
        for ext in extensions {
            let transport = if ext.transport.trim().is_empty() {
                "stdio"
            } else {
                ext.transport.trim()
            };
            caps.push(CapabilityRef {
                kind: "mcp".into(),
                id: format!("mcp:{}", ext.name),
                params: serde_json::json!({
                    "name": ext.name,
                    "command": ext.command,
                    "transport": transport,
                    "url": ext.url,
                }),
                enabled: true,
            });
        }
        caps
    }
}

pub fn extensions_from_capabilities(caps: &[CapabilityRef]) -> Vec<ExtensionSpec> {
    let mut out = Vec::new();
    for cap in caps.iter().filter(|c| c.enabled && c.kind == "mcp") {
        let name = cap
            .params
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if name.is_empty() {
            continue;
        }
        let command = cap
            .params
            .get("command")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let transport = cap
            .params
            .get("transport")
            .and_then(Value::as_str)
            .unwrap_or("stdio")
            .trim();
        let url = cap
            .params
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        out.push(ExtensionSpec {
            name: name.to_string(),
            transport: if transport.is_empty() {
                "stdio".into()
            } else {
                transport.to_string()
            },
            command,
            url,
            env: HashMap::new(),
        });
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "match", rename_all = "snake_case")]
pub enum PermissionMatch {
    Tool { name: String },
    FsPath { prefix: String },
    BashArgv { prefix: String },
    McpTool { extension: String, name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRule {
    pub r#match: PermissionMatch,
    pub effect: PermissionDefault,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillClass {
    #[default]
    Portable,
    App,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillRef {
    pub id: String,
    #[serde(default)]
    pub class: SkillClass,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub version: String,
    #[serde(default = "enabled_true")]
    pub enabled: bool,
}

impl Default for SkillRef {
    fn default() -> Self {
        Self {
            id: String::new(),
            class: SkillClass::App,
            origin: "bundled".into(),
            version: String::new(),
            enabled: true,
        }
    }
}

fn choice_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningChoice {
    #[serde(default = "choice_version")]
    pub v: u32,
    #[serde(flatten)]
    pub body: ReasoningChoiceBody,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReasoningChoiceBody {
    None,
    AlwaysOn,
    Toggle { on: bool },
    EffortEnum { value: String },
    BudgetTokens { value: u32 },
    Advanced { json: Value },
}

impl ReasoningChoice {
    pub fn kind_name(&self) -> &'static str {
        match self.body {
            ReasoningChoiceBody::None => "none",
            ReasoningChoiceBody::AlwaysOn => "always_on",
            ReasoningChoiceBody::Toggle { .. } => "toggle",
            ReasoningChoiceBody::EffortEnum { .. } => "effort_enum",
            ReasoningChoiceBody::BudgetTokens { .. } => "budget_tokens",
            ReasoningChoiceBody::Advanced { .. } => "advanced",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LegacyOpenOpts {
    pub model: String,
    pub llm_backend: String,
    pub base_url: String,
    pub enable_fs_tools: bool,
    pub bash_enabled: bool,
    pub enable_kim_tools: bool,
    pub enable_approvals: bool,
    pub thinking_effort: String,
    pub goose_mode: String,
    pub profile_id: String,
}

impl AgentProfile {
    pub fn from_legacy(opts: &LegacyOpenOpts) -> Self {
        let tools = ToolSet {
            fs: opts.enable_fs_tools,
            fs_write: false,
            bash: opts.bash_enabled,
            search_contacts: opts.enable_kim_tools,
            search_messages: opts.enable_kim_tools,
            get_conversation_context: opts.enable_kim_tools,
            list_profiles: opts.enable_kim_tools,
            send_message: opts.enable_kim_tools && opts.enable_approvals,
            read_clipboard: opts.enable_kim_tools && opts.enable_approvals,
            ..ToolSet::default()
        };
        let mode = if tools.has_any() {
            match parse_approval_mode(&opts.goose_mode) {
                Some(ApprovalMode::Chat) | Some(ApprovalMode::Auto) | None => {
                    ApprovalMode::SmartApprove
                }
                Some(mode) => mode,
            }
        } else {
            ApprovalMode::Chat
        };
        let id = if opts.profile_id.trim().is_empty() {
            DEFAULT_AGENT_ID.to_string()
        } else {
            opts.profile_id.trim().to_string()
        };
        Self {
            id: id.clone(),
            display_name: DEFAULT_AGENT_NAME.to_string(),
            aliases: vec!["助手".into()],
            provider: ProviderSpec {
                kind: opts.llm_backend.clone(),
                base_url: opts.base_url.clone(),
                key_ref: format!("agent.api_key.{id}"),
            },
            model: ModelSpec {
                name: opts.model.clone(),
                thinking_effort: parse_reasoning_effort(&opts.thinking_effort),
                ..Default::default()
            },
            reasoning: None,
            system_prompt: String::new(),
            mode,
            max_turns: Some(16),
            tools,
            capabilities: Vec::new(),
            permission_rules: Vec::new(),
            permissions: PermissionConfig::default(),
            sandbox: SandboxPolicy::default(),
            extensions: Vec::new(),
            enabled: true,
            steer: String::new(),
            workspace: WorkspaceSpec::default(),
            skills: Vec::new(),
            portable_denylist: Vec::new(),
            user_agents_skills: String::new(),
            runtime: String::new(),
            harness: None,
        }
    }

    pub fn normalize_mode(&mut self) {
        if self.mode != ApprovalMode::Auto {
            return;
        }
        let tools = self.project_toolset();
        self.mode = if tools.has_any()
            || !self.extensions.is_empty()
            || !self.skills.is_empty()
            || self
                .resolve_capabilities()
                .iter()
                .any(|cap| cap.enabled && cap.kind == "mcp")
        {
            ApprovalMode::SmartApprove
        } else {
            ApprovalMode::Chat
        };
    }

    pub fn fill_provider_from_legacy(&mut self, opts: &LegacyOpenOpts) {
        if self.provider.kind.trim().is_empty() {
            self.provider.kind = opts.llm_backend.clone();
        }
        if self.provider.base_url.trim().is_empty() {
            self.provider.base_url = opts.base_url.clone();
        }
    }

    pub fn resolve_capabilities(&self) -> Vec<CapabilityRef> {
        if !self.capabilities.is_empty() {
            return self.capabilities.clone();
        }
        CapabilityRef::from_legacy(&self.tools, &self.extensions)
    }

    pub fn runtime_kind(&self) -> RuntimeKind {
        match self.runtime.trim() {
            "codex" => RuntimeKind::Codex,
            _ => RuntimeKind::Goose,
        }
    }

    pub fn project_toolset(&self) -> ToolSet {
        ToolSet::from_capabilities(&self.resolve_capabilities())
    }

    pub fn project_extensions(&self) -> Vec<ExtensionSpec> {
        extensions_from_capabilities(&self.resolve_capabilities())
    }

    pub fn effective_identity_prompt(&self) -> &str {
        let text = self.system_prompt.trim();
        if text.is_empty() || is_legacy_tool_laundry_identity(text) {
            DEFAULT_IDENTITY_PROMPT
        } else {
            text
        }
    }

    pub fn effective_system_prompt(&self) -> &str {
        self.effective_identity_prompt()
    }
}

impl fmt::Display for ApprovalMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
