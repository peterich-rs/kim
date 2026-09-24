//! Open a host from the fields the FFI used to interpret itself.

use std::path::{Path, PathBuf};

use crate::{
    runtime_from_harness_json, AgentHost, AgentProfile, AgentRuntime, HostError, LegacyOpenOpts,
    ResolvedProfile,
};

/// Fields copied off the FFI `SessionOpenOpts`. The host owns the decision.
#[derive(Clone)]
pub struct OpenRequest {
    pub model: String,
    pub llm_backend: String,
    pub base_url: String,
    pub api_key: String,
    pub enable_fs_tools: bool,
    pub bash_enabled: bool,
    pub profile_id: String,
    pub profile_json: String,
    pub thinking_effort: String,
    pub goose_mode: String,
    pub enable_kim_tools: bool,
    pub enable_approvals: bool,
    pub harness_json: String,
}

/// Empty and `"goose"` are Goose. `"codex"` is Codex. Anything else is a
/// configuration error, not a silent fallback.
pub fn parse_agent_runtime(runtime: &str) -> Result<AgentRuntime, HostError> {
    match runtime.trim() {
        "" | "goose" => Ok(AgentRuntime::Goose),
        "codex" => Ok(AgentRuntime::Codex),
        other => Err(HostError::Failed(format!("unknown runtime {other}"))),
    }
}

pub fn resolve_open(
    opts: &OpenRequest,
    project_root: PathBuf,
) -> Result<ResolvedProfile, HostError> {
    let mut profile = if opts.profile_json.trim().is_empty() {
        if !opts.profile_id.trim().is_empty() {
            return Err(HostError::Profile(
                "profile_json required when profile_id is set".into(),
            ));
        }
        AgentProfile::from_legacy(&LegacyOpenOpts {
            model: opts.model.clone(),
            llm_backend: opts.llm_backend.clone(),
            base_url: opts.base_url.clone(),
            enable_fs_tools: opts.enable_fs_tools,
            bash_enabled: opts.bash_enabled,
            enable_kim_tools: opts.enable_kim_tools,
            enable_approvals: opts.enable_approvals,
            thinking_effort: opts.thinking_effort.clone(),
            goose_mode: opts.goose_mode.clone(),
            profile_id: if opts.profile_id.trim().is_empty() {
                "goose".into()
            } else {
                opts.profile_id.clone()
            },
        })
    } else {
        serde_json::from_str(&opts.profile_json)
            .map_err(|err| HostError::Profile(format!("profile_json: {err}")))?
    };
    parse_agent_runtime(&profile.runtime)?;
    profile.fill_provider_from_legacy(&LegacyOpenOpts {
        llm_backend: opts.llm_backend.clone(),
        base_url: opts.base_url.clone(),
        ..LegacyOpenOpts::default()
    });
    profile.normalize_mode();
    profile.apply_reasoning()?;
    tracing::info!(
        profile_id = %profile.id,
        provider = %profile.provider.kind,
        model = %profile.model.name,
        runtime = %profile.runtime,
        "session_open"
    );
    Ok(ResolvedProfile {
        profile,
        api_key: opts.api_key.clone(),
        project_root,
    })
}

#[cfg(feature = "codex")]
pub async fn attach_codex_for_open(
    host: &AgentHost,
    opts: &OpenRequest,
    profile: &AgentProfile,
    sqlite_path: &str,
    project_root: &str,
) -> Result<(), HostError> {
    let from_profile = profile.agent_runtime() == AgentRuntime::Codex;
    let from_harness = runtime_from_harness_json(&opts.harness_json) == AgentRuntime::Codex;
    if !from_profile && !from_harness {
        return Ok(());
    }
    let helper = crate::resolve_codex_helper()?;
    let linux = crate::linux_sandbox_beside(&helper);
    let home = crate::session_file_from_sqlite_path(sqlite_path)
        .map(|file| crate::codex_home_from_session_file(&file))
        .unwrap_or_else(|| {
            let root = Path::new(project_root);
            if root.is_absolute() {
                root.join("codex-home")
            } else {
                std::env::temp_dir().join("kim-codex")
            }
        });
    let transcript = crate::session_file_from_sqlite_path(sqlite_path)
        .map(|file| PathBuf::from(format!("{}.codex-transcript", file.display())));
    host.use_codex(crate::CodexLaunch {
        codex_home: home,
        helper,
        linux_sandbox: linux,
        api_key: opts.api_key.clone(),
        transcript,
    })
    .await
}

#[cfg(not(feature = "codex"))]
pub async fn attach_codex_for_open(
    _host: &AgentHost,
    opts: &OpenRequest,
    profile: &AgentProfile,
    _sqlite_path: &str,
    _project_root: &str,
) -> Result<(), HostError> {
    let from_profile = profile.agent_runtime() == AgentRuntime::Codex;
    let from_harness = runtime_from_harness_json(&opts.harness_json) == AgentRuntime::Codex;
    if from_profile || from_harness {
        return Err(HostError::Failed(
            "codex runtime is not compiled into this host".into(),
        ));
    }
    Ok(())
}
