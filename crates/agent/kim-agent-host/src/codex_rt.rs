//! One Codex turn, embedded. Not wired into the Goose loop or FFI.

use std::path::PathBuf;
use std::sync::Arc;

use codex_core_api::build_models_manager;
use codex_core_api::init_state_db;
use codex_core_api::local_agent_graph_store_from_state_db;
use codex_core_api::passthrough_image_store;
use codex_core_api::resolve_installation_id;
use codex_core_api::set_default_originator;
use codex_core_api::thread_store_from_config;
use codex_core_api::AbsolutePathBuf;
use codex_core_api::AuthCredentialsStoreMode;
use codex_core_api::AuthManager;
use codex_core_api::CodexAppsToolsCache;
use codex_core_api::CodexAuth;
use codex_core_api::CodexHomeUserInstructionsProvider;
use codex_core_api::CodexThread;
use codex_core_api::Config;
use codex_core_api::DynamicToolSpec;
use codex_core_api::EnvironmentManager;
use codex_core_api::EventMsg;
use codex_core_api::ExecServerRuntimePaths;
use codex_core_api::ExtensionRegistryBuilder;
use codex_core_api::SessionSource;
use codex_core_api::StartIfIdleSubmission;
use codex_core_api::StartThreadOptions;
use codex_core_api::ThreadId;
use codex_core_api::ThreadManager;
use codex_core_api::TurnInputRequest;
use codex_core_api::UserInput;

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

    let config = embed_config(&opts).await?;
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

pub(crate) async fn embed_config(opts: &CodexEmbedOpts) -> Result<Config, HostError> {
    let cwd = AbsolutePathBuf::from_absolute_path_checked(&opts.cwd)
        .map_err(|err| HostError::Failed(format!("cwd: {err}")))?;
    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(opts.codex_home.clone())
        .harness_overrides(codex_core::config::ConfigOverrides {
            cwd: Some(opts.cwd.clone()),
            workspace_roots: Some(vec![cwd]),
            model: (!opts.model.is_empty()).then(|| opts.model.clone()),
            codex_self_exe: opts.codex_self_exe.clone(),
            codex_linux_sandbox_exe: opts.codex_linux_sandbox_exe.clone(),
            ephemeral: Some(true),
            ..codex_core::config::ConfigOverrides::default()
        })
        .loader_overrides(codex_config::LoaderOverrides {
            ignore_user_config: true,
            ignore_project_config: true,
            ignore_login_requirements: true,
            ..codex_config::LoaderOverrides::default()
        })
        .build()
        .await
        .map_err(|err| HostError::Failed(format!("codex config: {err}")))?;
    config.cli_auth_credentials_store_mode = AuthCredentialsStoreMode::Ephemeral;
    config.agents_enabled = false;
    config.agent_max_threads = Some(0);
    config.check_for_update_on_startup = false;
    config.analytics_enabled = Some(false);
    config.include_environment_context = true;
    config.include_permissions_instructions = true;
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
    async fn embed_config_disables_subagents_and_file_auth() {
        let home = tempfile::tempdir().expect("tempdir");
        let config = embed_config(&opts(
            home.path(),
            "test-key",
            Some(home.path().join("kim-codex-helper")),
        ))
        .await
        .expect("embed");
        assert!(!config.agents_enabled);
        assert_eq!(config.agent_max_threads, Some(0));
        assert!(!config.check_for_update_on_startup);
        assert_eq!(
            config.cli_auth_credentials_store_mode,
            AuthCredentialsStoreMode::Ephemeral
        );
        assert!(!home.path().join(".codex").exists());
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
