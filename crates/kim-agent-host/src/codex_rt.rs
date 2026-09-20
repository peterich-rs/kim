//! One Codex turn, embedded. Not wired into the Goose loop or FFI.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use codex_core_api::build_models_manager;
use codex_core_api::built_in_model_providers;
use codex_core_api::init_state_db;
use codex_core_api::local_agent_graph_store_from_state_db;
use codex_core_api::passthrough_image_store;
use codex_core_api::resolve_installation_id;
use codex_core_api::set_default_originator;
use codex_core_api::thread_store_from_config;
use codex_core_api::AbsolutePathBuf;
use codex_core_api::AltScreenMode;
use codex_core_api::ApprovalsReviewer;
use codex_core_api::AskForApproval;
use codex_core_api::AuthCredentialsStoreMode;
use codex_core_api::AuthManager;
use codex_core_api::AutoCompactTokenLimitScope;
use codex_core_api::CodexAppsToolsCache;
use codex_core_api::CodexAuth;
use codex_core_api::CodexHomeUserInstructionsProvider;
use codex_core_api::CodexThread;
use codex_core_api::Config;
use codex_core_api::ConfigLayerStack;
use codex_core_api::Constrained;
use codex_core_api::DynamicToolSpec;
use codex_core_api::EnvironmentManager;
use codex_core_api::EventMsg;
use codex_core_api::ExecServerRuntimePaths;
use codex_core_api::ExtensionRegistryBuilder;
use codex_core_api::Features;
use codex_core_api::GhostSnapshotConfig;
use codex_core_api::History;
use codex_core_api::MemoriesConfig;
use codex_core_api::ModelAvailabilityNuxConfig;
use codex_core_api::MultiAgentV2Config;
use codex_core_api::Notice;
use codex_core_api::OAuthCredentialsStoreMode;
use codex_core_api::OtelConfig;
use codex_core_api::PermissionProfile;
use codex_core_api::Permissions;
use codex_core_api::ProjectConfig;
use codex_core_api::RealtimeAudioConfig;
use codex_core_api::RealtimeConfig;
use codex_core_api::SessionPickerViewMode;
use codex_core_api::SessionSource;
use codex_core_api::SqliteConfig;
use codex_core_api::StartIfIdleSubmission;
use codex_core_api::StartThreadOptions;
use codex_core_api::TerminalResizeReflowConfig;
use codex_core_api::ThreadId;
use codex_core_api::ThreadManager;
use codex_core_api::ThreadStoreConfig;
use codex_core_api::ToolSuggestConfig;
use codex_core_api::TuiKeymap;
use codex_core_api::TuiNotificationSettings;
use codex_core_api::TuiPetAnchor;
use codex_core_api::TurnInputRequest;
use codex_core_api::UriBasedFileOpener;
use codex_core_api::UserInput;
use codex_core_api::WebSearchMode;
use codex_core_api::OPENAI_PROVIDER_ID;

use crate::events::HostError;

/// Inputs for a single embedded turn. Profile projection is intentionally absent.
pub struct CodexEmbedOpts {
    pub codex_home: PathBuf,
    pub codex_self_exe: Option<PathBuf>,
    /// Linux only. A symlink named `codex-linux-sandbox` pointing at the helper.
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub cwd: PathBuf,
    /// Empty selects the Codex default model.
    pub model: String,
    #[cfg_attr(not(test), allow(dead_code))]
    pub api_key: String,
    #[cfg_attr(not(test), allow(dead_code))]
    pub prompt: String,
}

#[cfg_attr(not(test), allow(dead_code))]
pub async fn run_turn(opts: CodexEmbedOpts) -> Result<String, HostError> {
    if opts.api_key.is_empty() {
        return Err(HostError::MissingApiKey);
    }
    if helper_path_missing(&opts.codex_self_exe) {
        return Err(HostError::Failed(
            "codex helper executable is not configured".to_string(),
        ));
    }
    if opts.prompt.is_empty() {
        return Err(HostError::Failed("prompt is empty".to_string()));
    }
    require_absolute("codex helper", opts.codex_self_exe.as_deref())?;
    require_absolute(
        "linux sandbox helper",
        opts.codex_linux_sandbox_exe.as_deref(),
    )?;

    if let Err(err) = set_default_originator("kim".to_string()) {
        tracing::debug!("codex originator: {err:?}");
    }

    let config = embed_config(&opts)?;
    let started = start_thread(config, &opts.api_key, Vec::new()).await?;
    let turn_result = collect_assistant_text(&started.thread, &opts.prompt).await;
    let shutdown_result = started.thread.shutdown_and_wait().await;
    let _ = started.manager.remove_thread(&started.thread_id).await;
    let text = turn_result?;
    shutdown_result.map_err(|err| HostError::Failed(err.to_string()))?;
    Ok(text)
}

pub(crate) struct StartedCodex {
    pub manager: ThreadManager,
    pub thread: Arc<CodexThread>,
    pub thread_id: ThreadId,
}

pub(crate) async fn start_thread(
    config: Config,
    api_key: &str,
    dynamic_tools: Vec<DynamicToolSpec>,
) -> Result<StartedCodex, HostError> {
    let state_db = init_state_db(&config).await;
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key(api_key));
    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        config.codex_self_exe.clone(),
        config.codex_linux_sandbox_exe.clone(),
    )
    .map_err(|err| HostError::Failed(err.to_string()))?;
    let thread_store = thread_store_from_config(&config, state_db.clone());
    let environment_manager = Arc::new(
        EnvironmentManager::from_codex_home(
            config.codex_home.clone(),
            Some(local_runtime_paths),
            config.http_client_factory(),
        )
        .await
        .map_err(|err| HostError::Failed(err.to_string()))?,
    );
    let installation_id = resolve_installation_id(&config.codex_home)
        .await
        .map_err(|err| HostError::Failed(err.to_string()))?;
    let user_instructions_provider = Arc::new(CodexHomeUserInstructionsProvider::new(
        config.codex_home.clone(),
    ));
    let extensions = ExtensionRegistryBuilder::<Config>::new();
    let manager = ThreadManager::new(
        &config,
        Arc::clone(&auth_manager),
        build_models_manager(&config, auth_manager),
        CodexAppsToolsCache::default(),
        SessionSource::Exec,
        environment_manager,
        Arc::new(extensions.build()),
        user_instructions_provider,
        /*analytics_events_client*/ None,
        passthrough_image_store(),
        Arc::clone(&thread_store),
        local_agent_graph_store_from_state_db(state_db.as_ref()),
        installation_id,
        /*attestation_provider*/ None,
        /*external_time_provider*/ None,
    );
    let mut options = StartThreadOptions::new(config);
    options.dynamic_tools = dynamic_tools;
    options.session_source = Some(SessionSource::Exec);
    let started = manager
        .start_thread(options)
        .await
        .map_err(|err| HostError::Failed(err.to_string()))?;
    Ok(StartedCodex {
        thread_id: started.thread_id,
        thread: started.thread,
        manager,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
fn helper_path_missing(path: &Option<PathBuf>) -> bool {
    match path {
        None => true,
        Some(path) => path.as_os_str().is_empty(),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn require_absolute(label: &str, path: Option<&std::path::Path>) -> Result<(), HostError> {
    let Some(path) = path else {
        return Ok(());
    };
    if path.is_absolute() {
        Ok(())
    } else {
        Err(HostError::Failed(format!("{label} path must be absolute")))
    }
}

pub(crate) fn embed_config(opts: &CodexEmbedOpts) -> Result<Config, HostError> {
    let codex_home = AbsolutePathBuf::from_absolute_path_checked(&opts.codex_home)
        .map_err(|err| HostError::Failed(format!("codex_home: {err}")))?;
    let cwd = AbsolutePathBuf::from_absolute_path_checked(&opts.cwd)
        .map_err(|err| HostError::Failed(format!("cwd: {err}")))?;
    let model = if opts.model.is_empty() {
        None
    } else {
        Some(opts.model.clone())
    };
    let model_provider_id = OPENAI_PROVIDER_ID.to_string();
    let model_providers = built_in_model_providers(/*openai_base_url*/ None);
    let model_provider = model_providers
        .get(&model_provider_id)
        .cloned()
        .ok_or_else(|| {
            HostError::Failed("OpenAI model provider should be available".to_string())
        })?;

    let mut config = Config {
        config_layer_stack: ConfigLayerStack::default(),
        startup_warnings: Vec::new(),
        bypass_hook_trust: false,
        model,
        service_tier: None,
        review_model: None,
        model_context_window: None,
        model_auto_compact_token_limit: None,
        model_auto_compact_token_limit_scope: AutoCompactTokenLimitScope::Total,
        model_provider_id,
        model_provider,
        personality: None,
        permissions: Permissions::from_approval_and_profile(
            Constrained::allow_any(AskForApproval::Never),
            Constrained::allow_any(PermissionProfile::read_only()),
        )
        .map_err(|err| HostError::Failed(err.to_string()))?,
        explicit_permission_profile_mode: false,
        custom_permission_profiles: Vec::new(),
        approvals_reviewer: ApprovalsReviewer::User,
        enforce_residency: Constrained::allow_any(/*initial_value*/ None),
        hide_agent_reasoning: false,
        show_raw_agent_reasoning: false,
        base_instructions: None,
        base_instructions_provenance: None,
        developer_instructions: None,
        guardian_policy_config: None,
        include_permissions_instructions: false,
        include_apps_instructions: false,
        include_collaboration_mode_instructions: false,
        include_skill_instructions: false,
        skill_max_context_tokens: None,
        orchestrator_skills_enabled: false,
        orchestrator_mcp_enabled: false,
        include_environment_context: false,
        compact_prompt: None,
        notify: None,
        tui_notifications: TuiNotificationSettings::default(),
        animations: true,
        tui_whimsy: true,
        show_tooltips: true,
        tui_show_server_version_notice: true,
        tui_auto_recap: true,
        model_availability_nux: ModelAvailabilityNuxConfig::default(),
        tui_alternate_screen: AltScreenMode::Auto,
        tui_status_line: None,
        tui_status_line_use_colors: true,
        tui_terminal_title: None,
        tui_theme: None,
        tui_raw_output_mode: false,
        tui_pet: None,
        tui_pet_anchor: TuiPetAnchor::Composer,
        terminal_resize_reflow: TerminalResizeReflowConfig::default(),
        tui_keymap: TuiKeymap::default(),
        tui_session_picker_view: SessionPickerViewMode::Dense,
        tui_resume_cwd: None,
        tui_vim_mode_default: false,
        tui_question_esc_back: true,
        cwd: cwd.clone(),
        workspace_roots: vec![cwd],
        workspace_roots_explicit: false,
        cli_auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        mcp_servers: Constrained::allow_any(HashMap::new()),
        non_prefixed_mcp_tool_servers: None,
        mcp_oauth_credentials_store_mode: OAuthCredentialsStoreMode::File,
        mcp_oauth_callback_port: None,
        mcp_oauth_callback_url: None,
        mcp_optional_startup_grace: std::time::Duration::from_secs(1),
        model_providers,
        project_doc_max_bytes: 32 * 1024,
        project_doc_fallback_filenames: Vec::new(),
        tool_output_token_limit: None,
        agents_enabled: true,
        agent_max_threads: Some(6),
        agent_default_subagent_model: None,
        agent_default_subagent_reasoning_effort: None,
        agent_interrupt_message_enabled: false,
        agent_max_depth: 1,
        agent_roles: BTreeMap::new(),
        memories: MemoriesConfig::default(),
        sqlite: SqliteConfig::from_sqlite_home(codex_home.clone()),
        log_dir: codex_home.join("log").to_path_buf(),
        codex_home,
        history: History::default(),
        ephemeral: true,
        extra_config: None,
        file_opener: UriBasedFileOpener::VsCode,
        codex_self_exe: opts.codex_self_exe.clone(),
        codex_linux_sandbox_exe: opts.codex_linux_sandbox_exe.clone(),
        main_execve_wrapper_exe: None,
        zsh_path: None,
        model_reasoning_effort: None,
        plan_mode_reasoning_effort: None,
        model_reasoning_summary: None,
        model_catalog: None,
        model_verbosity: None,
        chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
        respect_system_proxy: false,
        apps_mcp_product_sku: None,
        responses_api_metadata: BTreeMap::new(),
        realtime_audio: RealtimeAudioConfig::default(),
        experimental_realtime_ws_base_url: None,
        experimental_realtime_webrtc_call_base_url: None,
        experimental_realtime_ws_model: None,
        realtime: RealtimeConfig::default(),
        experimental_realtime_ws_backend_prompt: None,
        experimental_realtime_ws_startup_context: None,
        experimental_realtime_start_instructions: None,
        experimental_thread_store: ThreadStoreConfig::Local,
        forced_chatgpt_workspace_id: None,
        forced_login_method: None,
        web_search_mode: Constrained::allow_any(WebSearchMode::Disabled),
        web_search_config: None,
        experimental_request_user_input_enabled: true,
        update_plan_enabled: true,
        tool_registry: Default::default(),
        code_mode: Default::default(),
        background_terminal_max_timeout: 300_000,
        thread_unload_delay: std::time::Duration::from_secs(60),
        ghost_snapshot: GhostSnapshotConfig::default(),
        multi_agent_v2: MultiAgentV2Config::default(),
        max_goal_token_budget: None,
        token_budget: None,
        token_budget_startup_config: None,
        rollout_budget: None,
        current_time_reminder: None,
        sleep_tool_mode: Default::default(),
        features: Default::default(),
        suppress_unstable_features_warning: false,
        active_project: ProjectConfig { trust_level: None },
        notices: Notice::default(),
        check_for_update_on_startup: false,
        disable_paste_burst: false,
        analytics_enabled: Some(false),
        feedback_enabled: false,
        tool_suggest: ToolSuggestConfig::default(),
        otel: OtelConfig::default(),
    };
    config
        .features
        .set(Features::with_defaults())
        .map_err(|err| HostError::Failed(err.to_string()))?;
    Ok(config)
}

#[cfg_attr(not(test), allow(dead_code))]
async fn collect_assistant_text(thread: &CodexThread, prompt: &str) -> Result<String, HostError> {
    let submission = thread
        .start_turn_if_idle(TurnInputRequest::user_input(vec![UserInput::Text {
            text: prompt.to_string(),
            text_elements: Vec::new(),
        }]))
        .await
        .map_err(|err| HostError::Failed(err.to_string()))?;
    if let StartIfIdleSubmission::NotSubmitted { reason } = submission {
        return Err(HostError::Failed(format!(
            "turn input was not submitted: {reason:?}"
        )));
    }

    let mut last_agent_message = String::new();
    loop {
        let event = thread
            .next_event()
            .await
            .map_err(|err| HostError::Failed(err.to_string()))?;
        if let EventMsg::AgentMessage(message) = &event.msg {
            if !message.message.is_empty() {
                last_agent_message = message.message.clone();
            }
        }
        match event.msg {
            EventMsg::TurnComplete(done) => {
                if let Some(error) = done.error {
                    return Err(HostError::Failed(error.message));
                }
                if let Some(text) = done.last_agent_message.filter(|text| !text.is_empty()) {
                    return Ok(text);
                }
                return Ok(last_agent_message);
            }
            EventMsg::Error(error) => return Err(HostError::Failed(error.message)),
            EventMsg::TurnAborted(_) => {
                return Err(HostError::Failed("turn aborted".to_string()));
            }
            EventMsg::ExecApprovalRequest(_) => {
                return Err(HostError::Failed(
                    "turn requested exec approval".to_string(),
                ));
            }
            EventMsg::ApplyPatchApprovalRequest(_) => {
                return Err(HostError::Failed(
                    "turn requested patch approval".to_string(),
                ));
            }
            EventMsg::RequestPermissions(_) => {
                return Err(HostError::Failed("turn requested permissions".to_string()));
            }
            EventMsg::RequestUserInput(_) => {
                return Err(HostError::Failed("turn requested user input".to_string()));
            }
            EventMsg::DynamicToolCallRequest(_) => {
                return Err(HostError::Failed(
                    "turn requested a dynamic tool call".to_string(),
                ));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(home: &std::path::Path, api_key: &str, exe: Option<PathBuf>) -> CodexEmbedOpts {
        CodexEmbedOpts {
            codex_home: home.join("codex"),
            codex_self_exe: exe,
            codex_linux_sandbox_exe: None,
            cwd: home.to_path_buf(),
            model: String::new(),
            api_key: api_key.to_string(),
            prompt: "ping".to_string(),
        }
    }

    #[tokio::test]
    async fn empty_api_key_returns_before_codex_home_io() {
        let home = tempfile::tempdir().expect("tempdir");
        let codex_home = home.path().join("codex");
        let err = run_turn(opts(
            home.path(),
            "",
            Some(home.path().join("kim-codex-helper")),
        ))
        .await
        .expect_err("empty key");
        assert!(matches!(err, HostError::MissingApiKey));
        assert!(!codex_home.exists());
        assert!(!home.path().join(".codex").exists());
    }

    #[tokio::test]
    async fn missing_helper_returns_before_codex_home_io() {
        let home = tempfile::tempdir().expect("tempdir");
        let codex_home = home.path().join("codex");
        let err = run_turn(opts(home.path(), "test-key", None))
            .await
            .expect_err("missing helper");
        assert!(matches!(err, HostError::Failed(_)));
        assert!(!codex_home.exists());
        assert!(!home.path().join(".codex").exists());
    }

    #[tokio::test]
    #[ignore = "needs KIM_CODEX_LIVE=1, KIM_CODEX_HELPER, and KIM_CODEX_API_KEY"]
    async fn live_turn_returns_assistant_text() {
        if std::env::var("KIM_CODEX_LIVE").ok().as_deref() != Some("1") {
            return;
        }
        let helper = std::env::var("KIM_CODEX_HELPER").expect("KIM_CODEX_HELPER");
        let api_key = std::env::var("KIM_CODEX_API_KEY").expect("KIM_CODEX_API_KEY");
        let home = tempfile::tempdir().expect("tempdir");
        let text = run_turn(CodexEmbedOpts {
            codex_home: home.path().join("codex"),
            codex_self_exe: Some(PathBuf::from(helper)),
            codex_linux_sandbox_exe: std::env::var_os("KIM_CODEX_LINUX_SANDBOX").map(PathBuf::from),
            cwd: home.path().to_path_buf(),
            model: std::env::var("KIM_CODEX_MODEL").unwrap_or_default(),
            api_key,
            prompt: "Reply with the single word pong.".to_string(),
        })
        .await
        .expect("live turn");
        assert!(!text.is_empty());
    }
}
