use std::path::PathBuf;

use goose_provider_types::goose_mode::GooseMode;
use goose_provider_types::thinking::ThinkingEffort;
pub use kim_agent_model::{
    parse_approval_mode, parse_reasoning_effort, AgentProfile, ApprovalMode, ExtensionSpec,
    HarnessSpec, LegacyOpenOpts, ModelSpec, PermissionConfig, PermissionDefault, ProviderSpec,
    ReasoningEffort, SandboxMode, SandboxPolicy, ToolSet, WorkspaceKind, WorkspaceSpec,
    DEFAULT_AGENT_ID, DEFAULT_AGENT_NAME, DEFAULT_IDENTITY_PROMPT,
};

#[cfg(test)]
use kim_agent_model::{
    is_legacy_tool_laundry_identity, CapabilityRef, LEGACY_TOOL_LAUNDRY_IDENTITY,
};

use crate::provider::ProviderConfig;
use crate::HostError;

pub fn to_goose_mode(mode: ApprovalMode) -> GooseMode {
    match mode {
        ApprovalMode::Auto => GooseMode::Auto,
        ApprovalMode::Approve => GooseMode::Approve,
        ApprovalMode::SmartApprove => GooseMode::SmartApprove,
        ApprovalMode::Chat => GooseMode::Chat,
    }
}

pub fn to_goose_effort(effort: ReasoningEffort) -> ThinkingEffort {
    match effort {
        ReasoningEffort::Off => ThinkingEffort::Off,
        ReasoningEffort::Low => ThinkingEffort::Low,
        ReasoningEffort::Medium => ThinkingEffort::Medium,
        ReasoningEffort::High => ThinkingEffort::High,
        ReasoningEffort::Max => ThinkingEffort::Max,
    }
}

pub fn parse_goose_mode(raw: &str) -> Option<GooseMode> {
    parse_approval_mode(raw).map(to_goose_mode)
}

pub fn parse_thinking_effort(raw: &str) -> Option<ThinkingEffort> {
    parse_reasoning_effort(raw).map(to_goose_effort)
}

/// Map persisted `reasoning` onto the model effort fields. Engine conversion stays at the Goose boundary.
pub fn apply_reasoning(profile: &mut AgentProfile) -> Result<(), HostError> {
    let Some(choice) = profile.reasoning.clone() else {
        return Ok(());
    };
    let apply =
        crate::catalog::to_model_spec(&profile.provider.kind, &profile.model.name, &choice)?;
    profile.model.thinking_effort = apply.thinking_effort;
    profile.model.extra_params = apply.extra_params;
    profile.model.reasoning = apply.reasoning;
    Ok(())
}

/// Secrets travel beside the profile, never inside it.
#[derive(Clone)]
pub struct ResolvedProfile {
    pub profile: AgentProfile,
    pub api_key: String,
    pub project_root: PathBuf,
}

impl std::fmt::Debug for ResolvedProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedProfile")
            .field("profile_id", &self.profile.id)
            .field("api_key", &"<redacted>")
            .field("project_root", &"<redacted>")
            .finish()
    }
}

impl std::fmt::Display for ResolvedProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "profile {}", self.profile.id)
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
                mode: ApprovalMode::Chat,
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
                runtime: String::new(),
                harness: None,
            },
            api_key: config.api_key,
            project_root: PathBuf::new(),
        })
    }
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
        mode: ApprovalMode::SmartApprove,
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
        runtime: String::new(),
        harness: None,
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
        system_prompt: "\
You are 译者, a translation specialist in the KIM messenger.
Translate the user's text between the languages they are using. Do not add commentary, opinions, or small talk.
Preserve meaning, tone, and formatting. Leave names, code, and URLs unchanged unless asked to localize them.
Output only the translation unless the user asks for notes (register, alternatives).
If they ask for something outside translation — coding, sending messages, browsing files — say that is out of scope and suggest another agent or capability they could enable.
Reply in the target language of the request."
            .into(),
        mode: ApprovalMode::Chat,
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
            runtime: String::new(),
            harness: None,
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
        system_prompt: "\
You are coder, a software assistant in the KIM messenger.
The workspace is this session's project root. Read existing code before changing it.
Make focused, reviewable edits. Fix root causes, not symptoms. Do not drive-by refactor unrelated code.
Match the project's language, style, and tests. Prefer evidence from the workspace over guessing.
When you finish, lead with what changed, then a short list of files touched.
If the user asks you to send messages or act outside this workspace, say that is out of scope and point them to an agent with that capability."
            .into(),
        mode: ApprovalMode::Approve,
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
            runtime: String::new(),
            harness: None,
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
        assert_eq!(profile.mode, ApprovalMode::SmartApprove);
        assert_ne!(profile.mode, ApprovalMode::Auto);
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
        assert_eq!(profile.mode, ApprovalMode::Chat);
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
        assert_eq!(profile.mode, ApprovalMode::SmartApprove);
    }

    #[test]
    fn from_legacy_auto_is_smart_approve() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_fs_tools: true,
            goose_mode: "auto".into(),
            ..LegacyOpenOpts::default()
        });
        assert_eq!(profile.mode, ApprovalMode::SmartApprove);
        assert_ne!(profile.mode, ApprovalMode::Auto);
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
        assert_eq!(profile.mode, ApprovalMode::Auto);
        profile.normalize_mode();
        assert_eq!(profile.mode, ApprovalMode::SmartApprove);
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
        assert_eq!(on.mode, ApprovalMode::SmartApprove);
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
        assert!(!DEFAULT_IDENTITY_PROMPT.contains("bash"));
        assert!(!DEFAULT_IDENTITY_PROMPT.contains("read_file"));
        assert!(!DEFAULT_IDENTITY_PROMPT.contains("write_file"));
        assert!(DEFAULT_IDENTITY_PROMPT.contains("# How you work"));
        assert!(DEFAULT_IDENTITY_PROMPT.contains("## Confirmations"));
        assert!(DEFAULT_IDENTITY_PROMPT.contains("## Communication"));
        assert!(!is_legacy_tool_laundry_identity(DEFAULT_IDENTITY_PROMPT));
    }

    #[test]
    fn translator_and_coder_templates_stay_single_persona() {
        let templates = builtin_templates();
        let translator = templates.iter().find(|p| p.id == "translator").unwrap();
        assert!(translator.system_prompt.contains("译者"));
        assert!(translator.system_prompt.contains("out of scope"));
        assert!(!translator.system_prompt.contains("send_message"));
        assert!(!translator.system_prompt.contains("search_contacts"));
        let coder = templates.iter().find(|p| p.id == "coder").unwrap();
        assert!(coder.system_prompt.contains("coder"));
        assert!(coder.system_prompt.contains("out of scope"));
        assert!(!coder.system_prompt.contains("send_message"));
        assert!(!coder.system_prompt.contains("read_file"));
        let goose = templates.iter().find(|p| p.id == "goose").unwrap();
        assert_eq!(goose.system_prompt, DEFAULT_IDENTITY_PROMPT);
    }

    #[test]
    fn model_context_tokens_round_trip() {
        let json = r#"{
            "id": "goose",
            "display_name": "助手",
            "model": {"name": "gpt-4o", "context_tokens": 128000}
        }"#;
        let profile: AgentProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.model.context_tokens, Some(128000));
        let out = serde_json::to_value(&profile).unwrap();
        assert_eq!(out["model"]["context_tokens"], 128000);
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
            env: Default::default(),
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
