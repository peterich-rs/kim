//! Desktop agent orchestration. Profile rows, tool results, and transcripts stay
//! in Rust. Dart only receives UI events and sends permission decisions.

mod drive;
mod permissions;
mod tools;
mod ui;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use kim_sdk::{
    AgentProfileRow, AgentRunResult, AgentRuntime, DeviceOverlayRow, KimSdk, MobileAgent,
    ProviderAccountRow, SdkError, SessionEpoch,
};

pub use kim_agent_host::THREAD_STACK_SIZE_BYTES;
pub use permissions::{PermissionEvent, PermissionMailbox};
pub use tools::execute_im_tool;
pub use ui::{AgentUiStatus, UiBus};

static INSTALLED: OnceLock<Arc<HostAgentRuntime>> = OnceLock::new();

/// Replaces [`kim_sdk::FfiAgentRuntime`] on desktop. Phone never calls this.
pub fn install(sdk: KimSdk) -> Arc<HostAgentRuntime> {
    let runtime = HostAgentRuntime::host(sdk.clone());
    sdk.set_agent(Arc::new(MobileAgent::new(sdk.clone(), runtime.clone())));
    let _ = INSTALLED.set(runtime.clone());
    runtime
}

pub fn installed() -> Option<Arc<HostAgentRuntime>> {
    INSTALLED.get().cloned()
}

pub fn cache_secret(key_ref: &str, secret: &str) {
    if let Some(rt) = installed() {
        rt.cache_secret(key_ref, secret);
    }
}

pub fn respond_permission(call_id: &str, allow: bool) {
    if let Some(rt) = installed() {
        rt.respond_permission(call_id, allow);
    }
}

pub struct PreparedTurn {
    pub dest: String,
    pub profile_id: String,
    pub text: String,
    pub epoch: u64,
    pub profile_json: String,
    pub api_key: String,
    pub project_root: String,
    pub account_id: String,
    /// Provider account vendor. Disk profiles omit `provider` (C-KD 1).
    pub vendor_id: String,
    /// Provider account base URL. Not the catalog default.
    pub base_url: String,
}

enum Engine {
    Host,
    Scripted(String),
}

pub struct HostAgentRuntime {
    sdk: KimSdk,
    secrets: Mutex<HashMap<String, String>>,
    engine: Engine,
    pub permissions: PermissionMailbox,
    pub ui: UiBus,
}

impl HostAgentRuntime {
    #[must_use]
    pub fn host(sdk: KimSdk) -> Arc<Self> {
        Arc::new(Self {
            sdk,
            secrets: Mutex::new(HashMap::new()),
            engine: Engine::Host,
            permissions: PermissionMailbox::new(),
            ui: UiBus::new(),
        })
    }

    #[must_use]
    pub fn scripted(sdk: KimSdk, output: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            sdk,
            secrets: Mutex::new(HashMap::new()),
            engine: Engine::Scripted(output.into()),
            permissions: PermissionMailbox::new(),
            ui: UiBus::new(),
        })
    }

    pub fn cache_secret(&self, key_ref: &str, secret: &str) {
        if key_ref.is_empty() || secret.is_empty() {
            return;
        }
        self.secrets
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(key_ref.to_string(), secret.to_string());
    }

    /// Vault lookup with Keychain fallback: in-memory first, then the global
    /// channel store (`account.<keyRef>` keys), then the shared goose key.
    pub async fn resolve_secret(&self, key_ref: &str) -> String {
        let vault = self.secret(key_ref);
        if !vault.is_empty() {
            return vault;
        }
        if key_ref != "agent.api_key.goose" {
            let via_keychain = kim_sdk::read_secret(&kim_sdk::account_key(key_ref))
                .await
                .unwrap_or_default();
            if !via_keychain.is_empty() {
                return via_keychain;
            }
        } else {
            let goose = kim_sdk::read_secret("agent.api_key.goose")
                .await
                .unwrap_or_default();
            if !goose.is_empty() {
                return goose;
            }
        }
        String::new()
    }

    pub fn respond_permission(&self, call_id: &str, allow: bool) {
        self.permissions.respond(call_id, allow);
    }

    pub fn secret(&self, key_ref: &str) -> String {
        self.secrets
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get(key_ref)
            .cloned()
            .unwrap_or_default()
    }

    /// Loads the persona from the SDK store. No Dart list/decode round-trip.
    pub async fn prepare(
        &self,
        dest: &str,
        profile_id: &str,
        text: &str,
        epoch: u64,
    ) -> Result<PreparedTurn, SdkError> {
        let profiles = self.sdk.list_agent_profiles().await?;
        let row = find_profile(&profiles, profile_id).ok_or_else(|| SdkError::NotFound {
            what: format!("agent profile {profile_id}"),
        })?;
        let profile_json = profile_body(row)?;
        let account_id = account_id_from_json(&profile_json);
        if account_id.is_empty() {
            return Err(SdkError::InvalidArgument {
                message: format!("agent profile {profile_id} has no provider account"),
            });
        }
        let accounts = self.sdk.list_provider_accounts().await?;
        let account = accounts
            .into_iter()
            .find(|row| row.id == account_id)
            .ok_or_else(|| SdkError::InvalidArgument {
                message: format!("provider account {account_id} missing for {profile_id}"),
            })?;
        let vendor_id = account.vendor_id.trim().to_string();
        let base_url = account.base_url.trim().to_string();
        if vendor_id.is_empty() || base_url.is_empty() {
            return Err(SdkError::InvalidArgument {
                message: format!("provider account {account_id} has no vendor or base url"),
            });
        }
        let mut api_key = self.resolve_secret(&account.key_ref).await;
        if api_key.is_empty() {
            api_key = self.secret("agent.api_key.goose");
        }
        if api_key.trim().is_empty() {
            return Err(SdkError::InvalidArgument {
                message: format!("api key missing for {profile_id}"),
            });
        }
        let overlay = self.sdk.get_device_overlay(profile_id.to_string()).await?;
        let project_root = self
            .resolve_project_root(profile_id, dest, overlay.as_ref())
            .await;
        Ok(PreparedTurn {
            dest: dest.to_string(),
            profile_id: profile_id.to_string(),
            text: text.to_string(),
            epoch,
            profile_json,
            api_key,
            project_root,
            account_id,
            vendor_id,
            base_url,
        })
    }
}

impl HostAgentRuntime {
    /// Provider model inventory. The key resolves through the vault by
    /// `key_ref`; plaintext never crosses the FFI edge.
    pub async fn fetch_models(
        &self,
        vendor_id: &str,
        base_url: &str,
        key_ref: &str,
    ) -> Result<Vec<String>, SdkError> {
        let mut api_key = self.resolve_secret(key_ref).await;
        if api_key.is_empty() {
            api_key = self.secret("agent.api_key.goose");
        }
        if api_key.trim().is_empty() {
            return Err(SdkError::InvalidArgument {
                message: format!("api key missing for {key_ref}"),
            });
        }
        let spec = kim_agent_host::ProviderSpec {
            kind: vendor_id.to_string(),
            base_url: base_url.to_string(),
            key_ref: key_ref.to_string(),
        };
        kim_agent_host::fetch_models(&spec, &api_key)
            .await
            .map_err(|err| SdkError::InvalidArgument {
                message: err.to_string(),
            })
    }
}

fn find_profile<'a>(rows: &'a [AgentProfileRow], profile_id: &str) -> Option<&'a AgentProfileRow> {
    rows.iter().find(|row| row.profile_id == profile_id)
}

fn bind_account_provider(
    profile: &mut kim_agent_host::AgentProfile,
    vendor_id: &str,
    base_url: &str,
) {
    if profile.provider.kind.trim().is_empty() {
        profile.provider.kind = vendor_id.trim().to_string();
    }
    if profile.provider.base_url.trim().is_empty() {
        profile.provider.base_url = base_url.trim().to_string();
    }
}

/// Preview projection: tool names + warnings.
#[derive(Clone, Debug)]
pub struct CapabilityPreview {
    pub tools: Vec<PreviewTool>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct PreviewTool {
    pub name: String,
    pub source: String,
    pub executor: String,
}

impl HostAgentRuntime {
    /// Assemble the capability preview from store rows only: profile,
    /// overlay, provider account. Dart passes an id, never JSON.
    pub async fn preview_profile(&self, profile_id: &str) -> Result<CapabilityPreview, SdkError> {
        let profiles = self.sdk.list_agent_profiles().await?;
        let row = find_profile(&profiles, profile_id).ok_or_else(|| SdkError::NotFound {
            what: format!("agent profile {profile_id}"),
        })?;
        let mut profile: kim_agent_host::AgentProfile =
            serde_json::from_str(&profile_body(row)?).map_err(|err| SdkError::InvalidArgument {
                message: format!("profile json: {err}"),
            })?;
        let account_id = account_id_from_json(&profile_body(row)?);
        if !account_id.is_empty() {
            if let Some(account) = self
                .sdk
                .list_provider_accounts()
                .await?
                .into_iter()
                .find(|a| a.id == account_id)
            {
                bind_account_provider(&mut profile, &account.vendor_id, &account.base_url);
            }
        }
        let overlay = self.sdk.get_device_overlay(profile_id.to_string()).await?;
        let project_root = self
            .resolve_project_root(profile_id, "", overlay.as_ref())
            .await;
        if !overlay
            .as_ref()
            .map(|o| o.user_agents_skills.trim())
            .unwrap_or("")
            .is_empty()
        {
            profile.user_agents_skills = overlay
                .as_ref()
                .map(|o| o.user_agents_skills.clone())
                .unwrap_or_default();
        }
        let preview =
            kim_agent_host::preview_assembled(&profile, std::path::Path::new(&project_root))
                .map_err(|err| SdkError::InvalidArgument {
                    message: err.to_string(),
                })?;
        Ok(CapabilityPreview {
            tools: preview
                .tools
                .into_iter()
                .map(|tool| PreviewTool {
                    name: tool.name,
                    source: tool.source,
                    executor: tool.executor,
                })
                .collect(),
            warnings: preview.warnings,
        })
    }
}

fn profile_body(row: &AgentProfileRow) -> Result<String, SdkError> {
    if !row.body_json.trim().is_empty() {
        return Ok(row.body_json.clone());
    }
    if row.body_blob.is_empty() {
        return Err(SdkError::InvalidArgument {
            message: format!("agent profile {} has empty spec", row.profile_id),
        });
    }
    kim_agent_codec::blob_to_json(&row.body_blob).map_err(|err| SdkError::InvalidArgument {
        message: err.to_string(),
    })
}

fn account_id_from_json(profile_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(profile_json)
        .ok()
        .and_then(|v| {
            v.get("accountId")
                .or_else(|| v.get("account_id"))
                .and_then(|id| id.as_str())
                .map(str::to_string)
        })
        .unwrap_or_default()
}

impl HostAgentRuntime {
    /// Workspace cwd: registered grant, then the overlay path, then the
    /// layout sandbox. Dart no longer computes the directory convention.
    async fn resolve_project_root(
        &self,
        profile_id: &str,
        dest: &str,
        overlay: Option<&DeviceOverlayRow>,
    ) -> String {
        if !profile_id.is_empty() {
            if let Ok(Some(grant)) = self.sdk.workspace_grant(profile_id.to_string()).await {
                let path = grant.path.trim();
                if !path.is_empty() {
                    return path.to_string();
                }
            }
        }
        if let Some(path) = overlay
            .map(|row| row.workspace_path.trim())
            .filter(|p| !p.is_empty())
        {
            return path.to_string();
        }
        if !profile_id.is_empty() {
            if let Ok(layout) = kim_sdk::Layout::current() {
                if let Ok(dir) = layout.ensure_sandbox(profile_id) {
                    return dir.to_string_lossy().into_owned();
                }
            }
        }
        let leaf = if profile_id.is_empty() {
            dest
        } else {
            profile_id
        };
        std::env::temp_dir()
            .join("kim-agent")
            .join(leaf)
            .display()
            .to_string()
    }
}

#[async_trait]
impl AgentRuntime for HostAgentRuntime {
    async fn run_turn(
        &self,
        dest: &str,
        profile_id: &str,
        text: &str,
        _in_reply_to: i64,
        epoch: SessionEpoch,
    ) -> Result<AgentRunResult, SdkError> {
        self.ui.publish(AgentUiStatus {
            dest: dest.to_string(),
            phase: "running".into(),
        });
        let result = self.run_turn_inner(dest, profile_id, text, epoch).await;
        let phase = match &result {
            Ok(row) => ui_phase(row),
            Err(_) => "failed",
        };
        self.ui.publish(AgentUiStatus {
            dest: dest.to_string(),
            phase: phase.into(),
        });
        result
    }
}

impl HostAgentRuntime {
    async fn run_turn_inner(
        &self,
        dest: &str,
        profile_id: &str,
        text: &str,
        epoch: SessionEpoch,
    ) -> Result<AgentRunResult, SdkError> {
        if let Engine::Scripted(output) = &self.engine {
            return Ok(AgentRunResult::ok(
                dest.to_string(),
                profile_id.to_string(),
                epoch.0,
                output.clone(),
            ));
        }
        let prepared = self.prepare(dest, profile_id, text, epoch.0).await?;
        drive::drive_host(self, prepared).await
    }
}

fn ui_phase(result: &AgentRunResult) -> &'static str {
    match result.stop_reason.as_str() {
        "completed" | "side_effect" | "empty" | "" => "done",
        _ => "failed",
    }
}

/// Test seam: provider rows are not required to build a secret lookup.
#[must_use]
pub fn key_ref_of(accounts: &[ProviderAccountRow], account_id: &str) -> String {
    accounts
        .iter()
        .find(|row| row.id == account_id)
        .map(|row| row.key_ref.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scripted_turn_never_lists_profiles() {
        let sdk = KimSdk::protocol_only();
        let runtime = HostAgentRuntime::scripted(sdk.as_ref().clone(), "hello");
        let result = runtime
            .run_turn("dest", "goose", "hi", 0, SessionEpoch(1))
            .await
            .expect("scripted");
        assert_eq!(result.output, "hello");
        assert!(result.replied);
        assert_eq!(result.stop_reason, "completed");
    }

    #[test]
    fn permission_respond_unblocks_waiter() {
        let box_ = PermissionMailbox::new();
        let mut rx = box_.subscribe();
        let wait = box_.publish(PermissionEvent {
            dest: "d".into(),
            call_id: "c1".into(),
            name: "shell".into(),
            preview: "ls".into(),
        });
        let event = rx.try_recv().expect("listener");
        assert_eq!(event.call_id, "c1");
        box_.respond("c1", true);
        assert!(wait.blocking_recv().expect("decision"));
    }

    #[test]
    fn key_ref_matches_account() {
        let rows = vec![ProviderAccountRow {
            id: "acc".into(),
            key_ref: "agent.api_key.acc".into(),
            ..ProviderAccountRow::default()
        }];
        assert_eq!(key_ref_of(&rows, "acc"), "agent.api_key.acc");
        assert!(key_ref_of(&rows, "missing").is_empty());
    }

    #[tokio::test]
    async fn scripted_turn_publishes_presence() {
        let sdk = KimSdk::protocol_only();
        let runtime = HostAgentRuntime::scripted(sdk.as_ref().clone(), "hello");
        let mut rx = runtime.ui.subscribe();
        runtime
            .run_turn("dest", "goose", "hi", 0, SessionEpoch(1))
            .await
            .expect("scripted");
        let running = rx.try_recv().expect("running");
        assert_eq!(running.phase, "running");
        let done = rx.try_recv().expect("done");
        assert_eq!(done.dest, "dest");
        assert_eq!(done.phase, "done");
    }
}
