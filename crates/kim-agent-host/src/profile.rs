use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::str::FromStr;

use goose_provider_types::goose_mode::GooseMode;
use goose_provider_types::thinking::ThinkingEffort;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::{CapabilityRef, PermissionRule};
use crate::catalog::ReasoningChoice;
use crate::provider::ProviderConfig;
use crate::skills::SkillRef;
use crate::{HostError, DEFAULT_AGENT_ID, DEFAULT_AGENT_NAME, DEFAULT_IDENTITY_PROMPT};

fn enabled_true() -> bool {
    true
}

/// Pre-B-KD 4 identity that enumerated every IM tool. Treat as empty.
const LEGACY_TOOL_LAUNDRY_IDENTITY: &str = concat!(
    "You are 助手, a local desktop agent inside the KIM messenger. ",
    "You run on the user's machine (not a cloud bot). Reply in the user's language. ",
    "Be concise. You can see the current conversation because the host pasted it into this session. ",
    "You have search_contacts, search_messages, get_conversation_context, list_profiles, ",
    "send_message, and read_clipboard. send_message and clipboard require user confirmation. ",
    "You do not have filesystem or shell access. Do not claim you have tools you were not given.",
);

fn is_legacy_tool_laundry_identity(prompt: &str) -> bool {
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

fn default_smart_approve() -> GooseMode {
    GooseMode::SmartApprove
}

/// Stable id. Default persona is "goose" (IM dest alias too).
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
    /// Missing JSON → SmartApprove. Never GooseMode::Auto (enum Default).
    #[serde(default = "default_smart_approve")]
    pub mode: GooseMode,
    #[serde(default)]
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub tools: ToolSet,
    /// Authoritative capability list. Empty → derived from `tools` + `extensions`.
    #[serde(default)]
    pub capabilities: Vec<CapabilityRef>,
    /// Ordered permission rules (Phase 3 engine). Phase 1 merges Tool matches into the map.
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
    /// Missing JSON → sandbox (S-KD 4).
    #[serde(default)]
    pub workspace: WorkspaceSpec,
    /// App-skill references only. Portable skills are discovered, not stored.
    #[serde(default)]
    pub skills: Vec<SkillRef>,
    /// Portable ids the user muted for this profile.
    #[serde(default)]
    pub portable_denylist: Vec<String>,
    /// Absolute path to the real `~/.agents/skills`. Empty disables the user
    /// shelf; the host never guesses it, because a sandboxed macOS container
    /// `$HOME` is not the ecosystem directory (S-KD 23).
    #[serde(default)]
    pub user_agents_skills: String,
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
    /// sandbox: empty. repo: absolute path, for display and validation only —
    /// the security-scoped bookmark stays in Dart.
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
    pub thinking_effort: Option<ThinkingEffort>,
    #[serde(default)]
    pub temperature: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<i32>,
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
    /// Opt-in `delegate` tool. Default 助手 stays IM-only.
    #[serde(default)]
    pub subagent: bool,
}

impl ToolSet {
    pub fn kim_world_names(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.send_message {
            v.push("send_message");
        }
        if self.search_contacts {
            v.push("search_contacts");
        }
        if self.search_messages {
            v.push("search_messages");
        }
        if self.get_conversation_context {
            v.push("get_conversation_context");
        }
        if self.read_clipboard {
            v.push("read_clipboard");
        }
        if self.list_profiles {
            v.push("list_profiles");
        }
        v
    }

    pub fn has_in_process(&self) -> bool {
        self.fs || self.fs_write || self.bash || self.subagent
    }

    pub fn has_any(&self) -> bool {
        !self.kim_world_names().is_empty() || self.has_in_process()
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
}

/// Secrets travel beside the profile, never inside it.
pub struct ResolvedProfile {
    pub profile: AgentProfile,
    pub api_key: String,
    pub project_root: PathBuf,
}

/// Fields `session_open` maps when `profile_json` is empty.
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
            match parse_goose_mode(&opts.goose_mode) {
                Some(GooseMode::Chat) | Some(GooseMode::Auto) | None => GooseMode::SmartApprove,
                Some(mode) => mode,
            }
        } else {
            GooseMode::Chat
        };

        let id = if opts.profile_id.trim().is_empty() {
            DEFAULT_AGENT_ID.to_string()
        } else {
            opts.profile_id.trim().to_string()
        };
        let key_ref = format!("agent.api_key.{id}");

        Self {
            id,
            display_name: DEFAULT_AGENT_NAME.to_string(),
            aliases: vec!["助手".into()],
            provider: ProviderSpec {
                kind: opts.llm_backend.clone(),
                base_url: opts.base_url.clone(),
                key_ref,
            },
            model: ModelSpec {
                name: opts.model.clone(),
                thinking_effort: parse_thinking_effort(&opts.thinking_effort),
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
        }
    }

    /// Untrusted JSON / `auto` must not silently AlwaysAllow send_message.
    pub fn normalize_mode(&mut self) {
        if self.mode != GooseMode::Auto {
            return;
        }
        let tools = self.project_toolset();
        self.mode = if tools.has_any()
            || !self.extensions.is_empty()
            || !self.skills.is_empty()
            || self
                .resolve_capabilities()
                .iter()
                .any(|c| c.enabled && c.kind == "mcp")
        {
            GooseMode::SmartApprove
        } else {
            GooseMode::Chat
        };
    }

    /// Map persisted `reasoning` onto Goose `ModelSpec` (effort + extra_params).
    /// Old `model.thinking_effort` is left alone when `reasoning` is absent.
    pub fn apply_reasoning(&mut self) -> Result<(), HostError> {
        let Some(choice) = self.reasoning.clone() else {
            return Ok(());
        };
        let apply = crate::catalog::to_model_spec(&self.provider.kind, &self.model.name, &choice)?;
        self.model.thinking_effort = apply.thinking_effort;
        self.model.extra_params = apply.extra_params;
        self.model.reasoning = apply.reasoning;
        Ok(())
    }

    pub fn fill_provider_from_legacy(&mut self, opts: &LegacyOpenOpts) {
        if self.provider.kind.trim().is_empty() {
            self.provider.kind = opts.llm_backend.clone();
        }
        if self.provider.base_url.trim().is_empty() {
            self.provider.base_url = opts.base_url.clone();
        }
    }

    /// Capabilities are authoritative when non-empty; otherwise derive from legacy tools.
    pub fn resolve_capabilities(&self) -> Vec<CapabilityRef> {
        if !self.capabilities.is_empty() {
            return self.capabilities.clone();
        }
        CapabilityRef::from_legacy(&self.tools, &self.extensions)
    }

    /// Project resolved capabilities onto the legacy `ToolSet` surface.
    pub fn project_toolset(&self) -> ToolSet {
        ToolSet::from_capabilities(&self.resolve_capabilities())
    }

    /// MCP rows implied by resolved capabilities. Empty when MCP is off.
    pub fn project_extensions(&self) -> Vec<ExtensionSpec> {
        crate::capability::extensions_from_capabilities(&self.resolve_capabilities())
    }

    /// Identity text only — never lists tools (B-KD 4).
    pub fn effective_identity_prompt(&self) -> &str {
        let t = self.system_prompt.trim();
        if t.is_empty() || is_legacy_tool_laundry_identity(t) {
            DEFAULT_IDENTITY_PROMPT
        } else {
            t
        }
    }

    /// Empty / whitespace prompt uses the identity-only default at assemble.
    pub fn effective_system_prompt(&self) -> &str {
        self.effective_identity_prompt()
    }
}

impl ResolvedProfile {
    pub fn from_provider_config(config: ProviderConfig) -> Result<Self, HostError> {
        if config.api_key.trim().is_empty() {
            return Err(HostError::MissingApiKey);
        }
        let kind = match config.kind {
            crate::provider::ProviderKind::OpenAi => "openai",
            crate::provider::ProviderKind::Anthropic => "anthropic",
        };
        Ok(Self {
            profile: AgentProfile {
                id: DEFAULT_AGENT_ID.to_string(),
                display_name: DEFAULT_AGENT_NAME.to_string(),
                aliases: vec!["助手".into()],
                provider: ProviderSpec {
                    kind: kind.into(),
                    base_url: config.base_url,
                    key_ref: "agent.api_key.goose".into(),
                },
                model: ModelSpec {
                    name: config.model,
                    ..Default::default()
                },
                reasoning: None,
                system_prompt: DEFAULT_IDENTITY_PROMPT.to_string(),
                mode: GooseMode::Chat,
                max_turns: Some(16),
                tools: ToolSet::default(),
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
            },
            api_key: config.api_key,
            project_root: PathBuf::new(),
        })
    }
}

pub fn parse_goose_mode(raw: &str) -> Option<GooseMode> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    GooseMode::from_str(trimmed).ok()
}

pub fn parse_thinking_effort(raw: &str) -> Option<ThinkingEffort> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    ThinkingEffort::from_str(trimmed).ok()
}

pub fn builtin_templates() -> Vec<AgentProfile> {
    vec![goose_template(), translator_template(), coder_template()]
}

fn goose_template() -> AgentProfile {
    AgentProfile {
        id: DEFAULT_AGENT_ID.to_string(),
        display_name: DEFAULT_AGENT_NAME.to_string(),
        aliases: vec!["助手".into()],
        provider: ProviderSpec {
            kind: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            key_ref: "agent.api_key.goose".into(),
        },
        model: ModelSpec {
            name: "gpt-4o".into(),
            ..Default::default()
        },
        reasoning: None,
        system_prompt: DEFAULT_IDENTITY_PROMPT.to_string(),
        mode: GooseMode::SmartApprove,
        max_turns: Some(16),
        tools: ToolSet {
            search_contacts: true,
            search_messages: true,
            get_conversation_context: true,
            list_profiles: true,
            send_message: true,
            read_clipboard: true,
            ..ToolSet::default()
        },
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
    }
}

fn translator_template() -> AgentProfile {
    AgentProfile {
        id: "translator".into(),
        display_name: "译者".into(),
        aliases: vec!["translator".into()],
        provider: ProviderSpec {
            kind: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            key_ref: "agent.api_key.translator".into(),
        },
        model: ModelSpec {
            name: "gpt-4o".into(),
            ..Default::default()
        },
        reasoning: None,
        system_prompt: "You are 译者. Translate between the user's languages. Do not chat. Do not claim you can send messages.".into(),
        mode: GooseMode::Chat,
        max_turns: Some(16),
        tools: ToolSet::default(),
        capabilities: Vec::new(),
        permission_rules: Vec::new(),
        permissions: PermissionConfig::default(),
        sandbox: SandboxPolicy::default(),
        extensions: Vec::new(),
        enabled: false,
        steer: String::new(),
        workspace: WorkspaceSpec::default(),
        skills: Vec::new(),
        portable_denylist: Vec::new(),
        user_agents_skills: String::new(),
    }
}

fn coder_template() -> AgentProfile {
    AgentProfile {
        id: "coder".into(),
        display_name: "coder".into(),
        aliases: Vec::new(),
        provider: ProviderSpec {
            kind: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            key_ref: "agent.api_key.coder".into(),
        },
        model: ModelSpec {
            name: "gpt-4o".into(),
            ..Default::default()
        },
        reasoning: None,
        system_prompt:
            "You are a software assistant. The workspace is project_root. Do not send messages."
                .into(),
        mode: GooseMode::Approve,
        max_turns: Some(32),
        tools: ToolSet {
            fs: true,
            fs_write: true,
            ..ToolSet::default()
        },
        capabilities: Vec::new(),
        permission_rules: Vec::new(),
        permissions: PermissionConfig::default(),
        sandbox: SandboxPolicy::default(),
        extensions: Vec::new(),
        enabled: false,
        steer: String::new(),
        // Coding profiles pick a repo in the desktop UI; the default stays
        // sandbox so a template can never point at the user's disk.
        workspace: WorkspaceSpec::default(),
        skills: Vec::new(),
        portable_denylist: Vec::new(),
        user_agents_skills: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_mode_with_tools_is_smart_approve_not_auto() {
        let json = r#"{
            "id": "goose",
            "display_name": "助手",
            "provider": {"kind": "openai", "key_ref": "agent.api_key.goose"},
            "model": {"name": "gpt-4o"},
            "system_prompt": "hi",
            "tools": {"fs": true}
        }"#;
        let profile: AgentProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.mode, GooseMode::SmartApprove);
        assert_ne!(profile.mode, GooseMode::Auto);
        assert!(profile.tools.fs);
    }

    #[test]
    fn serialized_profile_never_contains_api_key() {
        let mut profile = goose_template();
        let json = serde_json::to_string(&profile).unwrap();
        assert!(!json.contains("\"api_key\""), "{json}");
        assert!(!json.contains("sk-"), "{json}");

        let with_secret = r#"{
            "id": "goose",
            "display_name": "助手",
            "provider": {"kind": "openai", "key_ref": "agent.api_key.goose"},
            "model": {"name": "gpt-4o"},
            "system_prompt": "hi",
            "api_key": "sk-secret"
        }"#;
        profile = serde_json::from_str(with_secret).unwrap();
        let out = serde_json::to_string(&profile).unwrap();
        assert!(!out.contains("\"api_key\""), "{out}");
        assert!(!out.contains("sk-secret"), "{out}");
    }

    #[test]
    fn from_legacy_empty_tools_is_chat() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        assert_eq!(profile.mode, GooseMode::Chat);
        assert!(!profile.tools.has_any());
        assert_eq!(profile.max_turns, Some(16));
        assert_eq!(profile.id, DEFAULT_AGENT_ID);
    }

    #[test]
    fn from_legacy_fs_is_smart_approve() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_fs_tools: true,
            goose_mode: String::new(),
            ..LegacyOpenOpts::default()
        });
        assert!(profile.tools.fs);
        assert!(!profile.tools.bash);
        assert_eq!(profile.mode, GooseMode::SmartApprove);
    }

    #[test]
    fn from_legacy_auto_is_smart_approve() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_fs_tools: true,
            goose_mode: "auto".into(),
            ..LegacyOpenOpts::default()
        });
        assert_eq!(profile.mode, GooseMode::SmartApprove);
        assert_ne!(profile.mode, GooseMode::Auto);
    }

    #[test]
    fn json_auto_normalizes_to_smart_approve() {
        let json = r#"{
            "id": "goose",
            "display_name": "助手",
            "provider": {"kind": "openai", "key_ref": "agent.api_key.goose"},
            "model": {"name": "gpt-4o"},
            "system_prompt": "hi",
            "mode": "auto",
            "tools": {"send_message": true}
        }"#;
        let mut profile: AgentProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.mode, GooseMode::Auto);
        profile.normalize_mode();
        assert_eq!(profile.mode, GooseMode::SmartApprove);
    }

    #[test]
    fn from_legacy_bash_only_when_enabled() {
        let off = AgentProfile::from_legacy(&LegacyOpenOpts::default());
        assert!(!off.tools.bash);
        let on = AgentProfile::from_legacy(&LegacyOpenOpts {
            bash_enabled: true,
            ..LegacyOpenOpts::default()
        });
        assert!(on.tools.bash);
        assert_eq!(on.mode, GooseMode::SmartApprove);
    }

    #[test]
    fn builtin_goose_has_write_im_tools() {
        let goose = builtin_templates()
            .into_iter()
            .find(|p| p.id == "goose")
            .unwrap();
        assert!(goose.tools.search_contacts);
        assert!(goose.tools.search_messages);
        assert!(goose.tools.get_conversation_context);
        assert!(goose.tools.list_profiles);
        assert!(goose.tools.send_message);
        assert!(goose.tools.read_clipboard);
        assert!(!goose.tools.fs);
    }

    #[test]
    fn profile_without_provider_deserializes() {
        let json = r#"{
            "id": "goose",
            "display_name": "助手",
            "model": {"name": "deepseek-flash"},
            "reasoning": {"v": 1, "kind": "effort_enum", "value": "high"},
            "system_prompt": "hi"
        }"#;
        let profile: AgentProfile = serde_json::from_str(json).unwrap();
        assert!(profile.provider.kind.is_empty());
        assert_eq!(profile.model.name, "deepseek-flash");
        assert!(profile.reasoning.is_some());
    }

    #[test]
    fn missing_system_prompt_defaults_empty() {
        let json = r#"{
            "id": "goose",
            "display_name": "助手",
            "model": {"name": "gpt-4o"}
        }"#;
        let profile: AgentProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.system_prompt, "");
        assert_eq!(profile.effective_system_prompt(), DEFAULT_IDENTITY_PROMPT);
    }

    #[test]
    fn empty_system_prompt_uses_builtin_default() {
        let mut profile = goose_template();
        profile.system_prompt = String::new();
        assert_eq!(profile.effective_system_prompt(), DEFAULT_IDENTITY_PROMPT);
        profile.system_prompt = "  \n".into();
        assert_eq!(profile.effective_system_prompt(), DEFAULT_IDENTITY_PROMPT);
        profile.system_prompt = "Stay terse.".into();
        assert_eq!(profile.effective_system_prompt(), "Stay terse.");
    }

    #[test]
    fn from_legacy_writes_empty_system_prompt() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts::default());
        assert_eq!(profile.system_prompt, "");
        assert_eq!(profile.effective_system_prompt(), DEFAULT_IDENTITY_PROMPT);
    }

    #[test]
    fn identity_prompt_never_lists_tools() {
        assert!(!DEFAULT_IDENTITY_PROMPT.contains("send_message"));
        assert!(!DEFAULT_IDENTITY_PROMPT.contains("search_contacts"));
    }

    #[test]
    fn leftover_extensions_do_not_project_when_caps_omit_mcp() {
        let mut profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            enable_approvals: true,
            ..LegacyOpenOpts::default()
        });
        profile.capabilities = CapabilityRef::from_legacy(&profile.tools, &[]);
        profile.extensions = vec![ExtensionSpec {
            name: "github".into(),
            transport: "stdio".into(),
            command: vec!["npx".into()],
            url: String::new(),
        }];
        assert!(profile.project_extensions().is_empty());
    }

    #[test]
    fn laundry_list_identity_falls_back_to_tool_free_default() {
        let mut profile = goose_template();
        profile.system_prompt = LEGACY_TOOL_LAUNDRY_IDENTITY.to_string();
        assert_eq!(profile.effective_identity_prompt(), DEFAULT_IDENTITY_PROMPT);
        assert!(!profile.effective_identity_prompt().contains("send_message"));
    }
}
