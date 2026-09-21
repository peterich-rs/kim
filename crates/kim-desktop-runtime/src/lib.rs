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
        let accounts = self.sdk.list_provider_accounts().await?;
        let account = accounts.into_iter().find(|a| a.id == account_id);
        let key_ref = account.as_ref().map(|a| a.key_ref.as_str()).unwrap_or("");
        let mut api_key = self.secret(key_ref);
        if api_key.is_empty() {
            api_key = self.secret("agent.api_key.goose");
        }
        if api_key.trim().is_empty() {
            return Err(SdkError::InvalidArgument {
                message: format!("api key missing for {profile_id}"),
            });
        }
        let overlay = self.sdk.get_device_overlay(profile_id.to_string()).await?;
        let project_root = project_root(dest, overlay.as_ref());
        Ok(PreparedTurn {
            dest: dest.to_string(),
            profile_id: profile_id.to_string(),
            text: text.to_string(),
            epoch,
            profile_json,
            api_key,
            project_root,
            account_id,
        })
    }
}

fn find_profile<'a>(rows: &'a [AgentProfileRow], profile_id: &str) -> Option<&'a AgentProfileRow> {
    rows.iter().find(|row| row.profile_id == profile_id)
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

fn project_root(dest: &str, overlay: Option<&DeviceOverlayRow>) -> String {
    if let Some(path) = overlay
        .map(|row| row.workspace_path.trim())
        .filter(|p| !p.is_empty())
    {
        return path.to_string();
    }
    std::env::temp_dir()
        .join("kim-agent")
        .join(dest)
        .display()
        .to_string()
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
