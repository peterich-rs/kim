use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use goose_provider_types::goose_mode::GooseMode;
use goose_provider_types::thinking::ThinkingEffort;
use serde::{Deserialize, Serialize};

use crate::provider::ProviderConfig;
use crate::{HostError, DEFAULT_AGENT_ID, DEFAULT_AGENT_NAME, DEFAULT_SYSTEM_PROMPT};

fn enabled_true() -> bool {
    true
}

fn default_smart_approve() -> GooseMode {
    GooseMode::SmartApprove
}

/// Stable id. Default persona is "goose" (IM dest alias too).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProfile {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub provider: ProviderSpec,
    pub model: ModelSpec,
    pub system_prompt: String,
    /// Missing JSON → SmartApprove. Never GooseMode::Auto (enum Default).
    #[serde(default = "default_smart_approve")]
    pub mode: GooseMode,
    #[serde(default)]
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub tools: ToolSet,
    #[serde(default)]
    pub permissions: PermissionConfig,
    #[serde(default)]
    pub sandbox: SandboxPolicy,
    #[serde(default)]
    pub extensions: Vec<ExtensionSpec>,
    #[serde(default = "enabled_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderSpec {
    pub kind: String,
    #[serde(default)]
    pub base_url: String,
    pub key_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSpec {
    pub name: String,
    #[serde(default)]
    pub thinking_effort: Option<ThinkingEffort>,
    #[serde(default)]
    pub temperature: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<i32>,
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
        self.fs || self.fs_write || self.bash
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
            bash: false,
            search_contacts: opts.enable_kim_tools,
            search_messages: opts.enable_kim_tools,
            get_conversation_context: opts.enable_kim_tools,
            list_profiles: opts.enable_kim_tools,
            send_message: opts.enable_kim_tools && opts.enable_approvals,
            read_clipboard: opts.enable_kim_tools && opts.enable_approvals,
        };

        let mode = if tools.has_any() {
            match parse_goose_mode(&opts.goose_mode) {
                Some(GooseMode::Chat) | None => GooseMode::SmartApprove,
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
                temperature: None,
                max_tokens: None,
            },
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            mode,
            max_turns: Some(16),
            tools,
            permissions: PermissionConfig::default(),
            sandbox: SandboxPolicy::default(),
            extensions: Vec::new(),
            enabled: true,
        }
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
                    thinking_effort: None,
                    temperature: None,
                    max_tokens: None,
                },
                system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
                mode: GooseMode::Chat,
                max_turns: Some(16),
                tools: ToolSet::default(),
                permissions: PermissionConfig::default(),
                sandbox: SandboxPolicy::default(),
                extensions: Vec::new(),
                enabled: true,
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
            thinking_effort: None,
            temperature: None,
            max_tokens: None,
        },
        system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
        mode: GooseMode::SmartApprove,
        max_turns: Some(16),
        tools: ToolSet {
            search_contacts: true,
            search_messages: true,
            get_conversation_context: true,
            list_profiles: true,
            ..ToolSet::default()
        },
        permissions: PermissionConfig::default(),
        sandbox: SandboxPolicy::default(),
        extensions: Vec::new(),
        enabled: true,
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
            thinking_effort: None,
            temperature: None,
            max_tokens: None,
        },
        system_prompt: "You are 译者. Translate between the user's languages. Do not chat. Do not claim you can send messages.".into(),
        mode: GooseMode::Chat,
        max_turns: Some(16),
        tools: ToolSet::default(),
        permissions: PermissionConfig::default(),
        sandbox: SandboxPolicy::default(),
        extensions: Vec::new(),
        enabled: false,
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
            thinking_effort: None,
            temperature: None,
            max_tokens: None,
        },
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
        permissions: PermissionConfig::default(),
        sandbox: SandboxPolicy::default(),
        extensions: Vec::new(),
        enabled: false,
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
    fn builtin_goose_has_read_im_tools_only() {
        let goose = builtin_templates()
            .into_iter()
            .find(|p| p.id == "goose")
            .unwrap();
        assert!(goose.tools.search_contacts);
        assert!(goose.tools.search_messages);
        assert!(goose.tools.get_conversation_context);
        assert!(goose.tools.list_profiles);
        assert!(!goose.tools.send_message);
        assert!(!goose.tools.read_clipboard);
        assert!(!goose.tools.fs);
    }
}
