//! Project one `AgentProfile` onto an in-memory Codex `Config`.
//!
//! The shared `codex_home` is not the source of truth. Nothing here writes
//! `config.toml` or `auth.json`.

use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::McpServerAuth;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_config::SkillConfig;
use codex_config::SkillsConfig;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_core_api::built_in_model_providers;
use codex_core_api::AskForApproval;
use codex_core_api::AutoCompactTokenLimitScope;
use codex_core_api::Config;
use codex_core_api::Constrained;
use codex_core_api::DynamicToolFunctionSpec;
use codex_core_api::DynamicToolNamespaceSpec;
use codex_core_api::DynamicToolNamespaceTool;
use codex_core_api::DynamicToolSpec;
use codex_core_api::PermissionProfile;
use codex_core_api::Permissions;
use codex_core_api::OPENAI_PROVIDER_ID;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::openai_models::ReasoningEffort;
use codex_utils_redacted_string::RedactedString;
use goose_provider_types::goose_mode::GooseMode;
use goose_provider_types::thinking::ThinkingEffort;

use crate::capability::interpolate_identity;
use crate::events::HostError;
use crate::profile::AgentProfile;
use crate::profile::ExtensionSpec;
use crate::profile::PermissionDefault;

use std::collections::HashMap;
use std::path::Path;

pub(crate) fn apply_profile(
    config: &mut Config,
    profile: &AgentProfile,
    api_key: &str,
) -> Result<(), HostError> {
    if api_key.trim().is_empty() {
        return Err(HostError::MissingApiKey);
    }
    reject_unsupported_provider(&profile.provider.kind)?;

    let model = profile.model.name.trim();
    if !model.is_empty() {
        config.model = Some(model.to_string());
    }
    if let Some(tokens) = profile.model.context_tokens.filter(|n| *n > 0) {
        let window = i64::from(tokens);
        config.model_context_window = Some(window);
        config.model_auto_compact_token_limit = Some(window * 70 / 100);
        config.model_auto_compact_token_limit_scope = AutoCompactTokenLimitScope::Total;
    }
    if let Some(effort) = profile.model.thinking_effort {
        config.model_reasoning_effort = Some(map_effort(effort));
    }
    if profile.model.temperature.is_some()
        || profile.model.max_tokens.is_some()
        || !profile.model.extra_params.is_empty()
    {
        tracing::debug!(
            profile_id = %profile.id,
            "codex config has no temperature, max_tokens, or extra_params; left on the profile only"
        );
    }

    if official_openai(&profile.provider.kind, &profile.provider.base_url) {
        config.forced_login_method = Some(ForcedLoginMethod::Api);
    } else {
        install_custom_provider(config, profile, api_key)?;
    }
    config.model_provider.supports_websockets = false;
    if let Some(slot) = config.model_providers.get_mut(&config.model_provider_id) {
        slot.supports_websockets = false;
    }

    let writable = writable_shell(profile);
    config.permissions = Permissions::from_approval_and_profile(
        Constrained::allow_any(approval_policy(profile.mode, writable)),
        Constrained::allow_any(permission_profile(profile.mode, writable)),
    )
    .map_err(|err| HostError::Failed(err.to_string()))?;

    config.developer_instructions = Some(developer_instructions(profile, writable));
    config.hide_agent_reasoning = true;
    config.show_raw_agent_reasoning = false;
    config.ephemeral = false;
    config.analytics_enabled = Some(false);
    project_extensions(config, profile)?;
    project_skills(config, profile)?;
    if !profile.skills.is_empty() {
        tracing::debug!(
            profile_id = %profile.id,
            "app skill refs stay on the KIM catalog; Codex only loads portable SKILL.md directories"
        );
    }
    Ok(())
}

pub(crate) fn kim_dynamic_tools(profile: &AgentProfile) -> Vec<DynamicToolSpec> {
    let tools = profile.project_toolset();
    let mut specs = Vec::new();
    for name in tools.kim_world_names() {
        if profile.permissions.tools.get(name) == Some(&PermissionDefault::NeverAllow) {
            continue;
        }
        specs.push(DynamicToolNamespaceTool::Function(function_spec(name)));
    }
    if specs.is_empty() {
        return Vec::new();
    }
    vec![DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
        name: "kim".to_string(),
        description: "KIM messenger tools. They run in the app, not in a shell.".to_string(),
        tools: specs,
    })]
}

pub(crate) fn confirms_in_app(profile: &AgentProfile, name: &str) -> bool {
    match profile.permissions.tools.get(name) {
        Some(PermissionDefault::AlwaysAllow) => false,
        Some(PermissionDefault::NeverAllow) => false,
        Some(PermissionDefault::AskBefore) => true,
        None => matches!(name, "send_message" | "read_clipboard"),
    }
}

fn install_custom_provider(
    config: &mut Config,
    profile: &AgentProfile,
    api_key: &str,
) -> Result<(), HostError> {
    let base_url = profile
        .provider
        .base_url
        .trim()
        .trim_end_matches('/')
        .to_string();
    if base_url.is_empty() {
        return Err(HostError::InvalidUrl(
            "custom provider base_url is empty".into(),
        ));
    }
    let providers = built_in_model_providers(None);
    let mut info = providers
        .get(OPENAI_PROVIDER_ID)
        .cloned()
        .ok_or_else(|| HostError::Failed("OpenAI model provider should be available".into()))?;
    info.name = profile.provider.kind.trim().to_string();
    info.base_url = Some(base_url);
    info.env_key = None;
    info.requires_openai_auth = false;
    // Cloned from the OpenAI built-in (`supports_websockets = true`). Most
    // gateways only speak HTTP SSE Responses; leave WS on and Codex tries
    // `wss://…/responses` first, which hangs or fails before HTTP fallback.
    info.supports_websockets = false;
    info.experimental_bearer_token = Some(RedactedString::from(api_key));
    let id = "kim".to_string();
    config.model_provider_id = id.clone();
    config.model_provider = info.clone();
    if let Some(slot) = config.model_providers.get_mut(&id) {
        *slot = info;
    } else {
        config.model_providers.insert(id, info);
    }
    config.forced_login_method = Some(ForcedLoginMethod::Api);
    Ok(())
}

fn official_openai(kind: &str, base_url: &str) -> bool {
    let kind = kind.trim().to_ascii_lowercase();
    let url = base_url.trim().trim_end_matches('/');
    let kind_ok = matches!(
        kind.as_str(),
        "" | "openai" | "live" | "responses" | "responses_http"
    );
    let url_ok = url.is_empty()
        || url.eq_ignore_ascii_case("https://api.openai.com/v1")
        || url.eq_ignore_ascii_case("https://api.openai.com");
    kind_ok && url_ok
}

fn reject_unsupported_provider(kind: &str) -> Result<(), HostError> {
    let kind = kind.trim().to_ascii_lowercase();
    if kind.contains("bedrock") {
        return Err(HostError::Failed(
            "bedrock providers are not supported on the codex runtime".into(),
        ));
    }
    if matches!(kind.as_str(), "anthropic" | "gemini" | "ollama") {
        return Err(HostError::Failed(format!(
            "provider {kind} is not a Responses endpoint"
        )));
    }
    Ok(())
}

fn writable_shell(profile: &AgentProfile) -> bool {
    let tools = profile.project_toolset();
    tools.bash || tools.fs_write
}

fn approval_policy(mode: GooseMode, writable: bool) -> AskForApproval {
    match mode {
        GooseMode::Chat => AskForApproval::Never,
        GooseMode::Auto => AskForApproval::Never,
        GooseMode::Approve | GooseMode::SmartApprove if writable => AskForApproval::OnRequest,
        GooseMode::Approve | GooseMode::SmartApprove => AskForApproval::Never,
    }
}

fn permission_profile(mode: GooseMode, writable: bool) -> PermissionProfile {
    if mode == GooseMode::Chat || !writable {
        PermissionProfile::read_only()
    } else {
        PermissionProfile::workspace_write()
    }
}

fn developer_instructions(profile: &AgentProfile, writable: bool) -> String {
    let mut text = interpolate_identity(profile.effective_identity_prompt(), profile);
    let steer = profile.steer.trim();
    if !steer.is_empty() {
        text.push_str("\n\n# Steer\n");
        text.push_str(steer);
    }
    if profile.mode == GooseMode::Chat || !writable {
        text.push_str(
            "\n\nYou do not have permission to write files or run shell commands. 没有文件写入和 shell 权限。",
        );
    }
    text
}

fn map_effort(effort: ThinkingEffort) -> ReasoningEffort {
    match effort {
        ThinkingEffort::Off => ReasoningEffort::None,
        ThinkingEffort::Low => ReasoningEffort::Low,
        ThinkingEffort::Medium => ReasoningEffort::Medium,
        ThinkingEffort::High => ReasoningEffort::High,
        ThinkingEffort::Max => ReasoningEffort::Max,
    }
}

fn project_extensions(config: &mut Config, profile: &AgentProfile) -> Result<(), HostError> {
    let specs = profile.project_extensions();
    if specs.is_empty() {
        return Ok(());
    }
    let mut servers = HashMap::new();
    for spec in &specs {
        let name = spec.name.trim();
        if name.is_empty() {
            return Err(HostError::Failed("mcp extension name is empty".into()));
        }
        if servers.contains_key(name) {
            return Err(HostError::Failed(format!(
                "mcp extension name {name} is duplicated"
            )));
        }
        servers.insert(name.to_string(), mcp_server(spec)?);
    }
    config.mcp_servers = Constrained::allow_any(servers);
    Ok(())
}

fn mcp_server(spec: &ExtensionSpec) -> Result<McpServerConfig, HostError> {
    let transport_name = spec.transport.trim();
    let http = !spec.url.trim().is_empty()
        && (spec.command.is_empty()
            || matches!(
                transport_name,
                "http" | "streamable_http" | "sse" | "streamable-http"
            ));
    let transport = if http {
        McpServerTransportConfig::StreamableHttp {
            url: spec.url.trim().to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
            http_headers_helper: None,
        }
    } else if !spec.command.is_empty() && (transport_name.is_empty() || transport_name == "stdio") {
        McpServerTransportConfig::Stdio {
            command: spec.command[0].clone(),
            args: spec.command[1..].to_vec(),
            env: None,
            env_vars: Vec::new(),
            cwd: None,
        }
    } else {
        return Err(HostError::Failed(format!(
            "mcp transport {transport_name} is not supported"
        )));
    };
    Ok(McpServerConfig {
        transport,
        auth: McpServerAuth::default(),
        environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        omit_tools_from: None,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    })
}

/// Skill enablement stays on this session's layer stack. A user layer is added
/// only when `user_agents_skills` is an absolute `.../skills` directory, so an
/// empty field does not make Codex scan `$HOME`.
fn project_skills(config: &mut Config, profile: &AgentProfile) -> Result<(), HostError> {
    let mut layers = Vec::new();
    if let Some(layer) = user_skills_layer(&profile.user_agents_skills)? {
        layers.push(layer);
    }
    if let Some(layer) = denylist_layer(&profile.portable_denylist)? {
        layers.push(layer);
    }
    if layers.is_empty() {
        if !profile.skills.is_empty() {
            config.include_skill_instructions = true;
        }
        return Ok(());
    }
    config.include_skill_instructions = true;
    config.config_layer_stack = ConfigLayerStack::new(
        layers,
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .map_err(|err| HostError::Failed(format!("skill config layer: {err}")))?;
    Ok(())
}

fn user_skills_layer(raw: &str) -> Result<Option<ConfigLayerEntry>, HostError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let path = Path::new(raw);
    if path.file_name().and_then(|name| name.to_str()) != Some("skills") {
        return Err(HostError::Failed(
            "user_agents_skills must be an absolute directory named skills".into(),
        ));
    }
    let Some(folder) = path.parent() else {
        return Err(HostError::Failed(
            "user_agents_skills must be an absolute directory named skills".into(),
        ));
    };
    let file = codex_config::AbsolutePathBuf::from_absolute_path(folder.join("config.toml"))
        .map_err(|_| {
            HostError::Failed(
                "user_agents_skills must be an absolute directory named skills".into(),
            )
        })?;
    Ok(Some(ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file,
            profile: None,
        },
        toml::Value::Table(toml::map::Map::new()),
    )))
}

fn denylist_layer(ids: &[String]) -> Result<Option<ConfigLayerEntry>, HostError> {
    let config: Vec<SkillConfig> = ids
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
        .map(|id| SkillConfig {
            path: None,
            name: Some(id.to_string()),
            enabled: false,
        })
        .collect();
    if config.is_empty() {
        return Ok(None);
    }
    let skills = SkillsConfig {
        config,
        include_instructions: Some(true),
        ..SkillsConfig::default()
    };
    let skills_value = toml::Value::try_from(skills)
        .map_err(|err| HostError::Failed(format!("skills config: {err}")))?;
    let mut root = toml::map::Map::new();
    root.insert("skills".to_string(), skills_value);
    Ok(Some(ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::Value::Table(root),
    )))
}

fn function_spec(name: &str) -> DynamicToolFunctionSpec {
    let (description, input_schema) = match name {
        "search_contacts" => (
            "Search the user's contacts by name or account.",
            object_schema(&[("query", "string", "Name or account fragment.")], &[]),
        ),
        "search_messages" => (
            "Search messages. Pass dest to limit the search to one conversation.",
            object_schema(
                &[
                    ("query", "string", "Text to search for."),
                    ("dest", "string", "Conversation id. Empty searches broadly."),
                ],
                &["query"],
            ),
        ),
        "get_conversation_context" => (
            "Read recent messages from one conversation.",
            object_schema(
                &[
                    (
                        "dest",
                        "string",
                        "Conversation id. Empty means the current chat.",
                    ),
                    ("limit", "integer", "How many messages to return, 1 to 50."),
                ],
                &[],
            ),
        ),
        "list_profiles" => (
            "List agent profiles on this device.",
            object_schema(&[], &[]),
        ),
        "send_message" => (
            "Send a text message to a person or group. Requires the user's confirmation.",
            object_schema(
                &[
                    ("dest", "string", "Recipient conversation id."),
                    ("text", "string", "Message body."),
                ],
                &["dest", "text"],
            ),
        ),
        "read_clipboard" => (
            "Read the current clipboard text. Requires the user's confirmation.",
            object_schema(&[], &[]),
        ),
        other => (other, object_schema(&[], &[])),
    };
    DynamicToolFunctionSpec {
        name: name.to_string(),
        description: description.to_string(),
        input_schema,
        defer_loading: false,
    }
}

fn object_schema(props: &[(&str, &str, &str)], required: &[&str]) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    for (name, ty, desc) in props {
        properties.insert(
            (*name).to_string(),
            serde_json::json!({ "type": ty, "description": desc }),
        );
    }
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex_rt::embed_config;
    use crate::codex_rt::CodexEmbedOpts;
    use crate::profile::LegacyOpenOpts;
    use crate::profile::ToolSet;

    async fn config_for(profile: &AgentProfile) -> Config {
        let home = tempfile::tempdir().expect("tempdir");
        let opts = CodexEmbedOpts {
            codex_home: home.path().join("codex"),
            codex_self_exe: Some(home.path().join("kim-codex-helper")),
            codex_linux_sandbox_exe: None,
            cwd: home.path().to_path_buf(),
            model: profile.model.name.clone(),
            api_key: "test-key".into(),
            prompt: "ping".into(),
        };
        let mut config = embed_config(&opts).await.expect("config");
        apply_profile(&mut config, profile, "test-key").expect("apply");
        config
    }

    fn legacy(kind: &str, mode: &str) -> AgentProfile {
        AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "gpt-4o".into(),
            llm_backend: kind.into(),
            goose_mode: mode.into(),
            profile_id: "goose".into(),
            ..LegacyOpenOpts::default()
        })
    }

    #[tokio::test]
    async fn chat_profile_is_read_only_and_compacts_at_seventy_percent() {
        let mut profile = legacy("openai", "chat");
        profile.mode = GooseMode::Chat;
        profile.model.context_tokens = Some(1000);
        let config = config_for(&profile).await;
        assert_eq!(
            config.permissions.approval_policy.get(),
            &AskForApproval::Never
        );
        assert_eq!(
            config.permissions.effective_permission_profile(),
            PermissionProfile::read_only()
        );
        assert_eq!(config.model_context_window, Some(1000));
        assert_eq!(config.model_auto_compact_token_limit, Some(700));
        assert!(!config.ephemeral);
        assert_eq!(config.forced_login_method, Some(ForcedLoginMethod::Api));
        assert_eq!(config.model_provider_id, OPENAI_PROVIDER_ID);
        let instructions = config.developer_instructions.unwrap_or_default();
        assert!(instructions.contains("没有文件写入"));
        assert!(instructions.contains("助手"));
        assert!(!config.model_provider.supports_websockets);
        assert!(
            !config
                .model_providers
                .get(&config.model_provider_id)
                .expect("openai provider")
                .supports_websockets
        );
    }

    #[tokio::test]
    async fn custom_provider_keeps_the_key_on_the_bearer() {
        let mut profile = legacy("openai_compatible", "auto");
        profile.provider.base_url = "https://example.test/v1".into();
        profile.mode = GooseMode::Auto;
        profile.tools.bash = true;
        let config = config_for(&profile).await;
        assert_eq!(config.model_provider_id, "kim");
        assert!(!config.model_provider.requires_openai_auth);
        assert!(!config.model_provider.supports_websockets);
        assert_eq!(
            config.model_provider.base_url.as_deref(),
            Some("https://example.test/v1")
        );
        assert!(config.model_provider.experimental_bearer_token.is_some());
        assert_eq!(
            config.permissions.approval_policy.get(),
            &AskForApproval::Never
        );
        assert_ne!(
            config.permissions.effective_permission_profile(),
            PermissionProfile::read_only()
        );
    }

    #[tokio::test]
    async fn smart_approve_with_bash_asks_on_request() {
        let mut profile = legacy("openai", "smart_approve");
        profile.mode = GooseMode::SmartApprove;
        profile.tools.bash = true;
        let config = config_for(&profile).await;
        assert_eq!(
            config.permissions.approval_policy.get(),
            &AskForApproval::OnRequest
        );
    }

    #[tokio::test]
    async fn anthropic_is_rejected_before_a_thread_starts() {
        let profile = legacy("anthropic", "chat");
        let home = tempfile::tempdir().expect("tempdir");
        let opts = CodexEmbedOpts {
            codex_home: home.path().join("codex"),
            codex_self_exe: Some(home.path().join("kim-codex-helper")),
            codex_linux_sandbox_exe: None,
            cwd: home.path().to_path_buf(),
            model: "claude".into(),
            api_key: "test-key".into(),
            prompt: "ping".into(),
        };
        let mut config = embed_config(&opts).await.expect("config");
        let err = apply_profile(&mut config, &profile, "test-key").unwrap_err();
        assert!(err.to_string().contains("Responses"), "{err}");
    }

    #[test]
    fn send_message_is_a_kim_namespace_tool() {
        let mut profile = legacy("openai", "smart_approve");
        profile.tools = ToolSet {
            send_message: true,
            search_contacts: true,
            ..ToolSet::default()
        };
        let tools = kim_dynamic_tools(&profile);
        assert_eq!(tools.len(), 1);
        assert!(confirms_in_app(&profile, "send_message"));
        assert!(!confirms_in_app(&profile, "search_contacts"));
    }

    #[tokio::test]
    async fn stdio_extension_becomes_an_mcp_server() {
        let mut profile = legacy("openai", "chat");
        profile.extensions = vec![ExtensionSpec {
            name: "github".into(),
            transport: "stdio".into(),
            command: vec!["npx".into(), "-y".into(), "srv".into()],
            url: String::new(),
            env: Default::default(),
        }];
        let config = config_for(&profile).await;
        let server = config
            .mcp_servers
            .get()
            .get("github")
            .expect("github server");
        match &server.transport {
            McpServerTransportConfig::Stdio { command, args, .. } => {
                assert_eq!(command, "npx");
                assert_eq!(args, &["-y".to_string(), "srv".to_string()]);
            }
            other => panic!("expected stdio, got {other:?}"),
        }
        assert!(server.oauth.is_none());
    }

    #[tokio::test]
    async fn skill_denylist_is_a_session_layer_and_empty_root_skips_home() {
        let mut profile = legacy("openai", "chat");
        profile.portable_denylist = vec!["mute-me".into()];
        let config = config_for(&profile).await;
        assert!(config.config_layer_stack.get_active_user_layer().is_none());
        let rules = codex_config::skill_config_rules_from_stack(&config.config_layer_stack);
        assert_eq!(rules.entries.len(), 1);
        assert!(!rules.entries[0].enabled);
        assert!(matches!(
            &rules.entries[0].selector,
            codex_config::SkillConfigRuleSelector::Name(name) if name == "mute-me"
        ));
    }

    #[tokio::test]
    async fn absolute_skills_directory_becomes_the_user_scan_root() {
        let mut profile = legacy("openai", "chat");
        profile.user_agents_skills = "/tmp/kim-agents/skills".into();
        let config = config_for(&profile).await;
        let folder = config
            .config_layer_stack
            .get_active_user_layer()
            .and_then(|layer| layer.config_folder())
            .expect("user skill folder");
        assert_eq!(
            folder.join("skills").as_path(),
            Path::new("/tmp/kim-agents/skills")
        );
    }
}
