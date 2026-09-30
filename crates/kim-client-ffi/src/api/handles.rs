//! Permission, secret, and preview methods on [`KimUiHandle`].

use super::client::KimUiHandle;
use super::failure::ApiFailure;
use crate::frb_generated::StreamSink;

impl KimUiHandle {
    /// One-shot secret handoff (form save). Persists under a Rust-owned
    /// keyRef: Keychain via the executor channel + in-memory vault mirror.
    /// After this call plaintext keys never cross the boundary again.
    pub async fn store_agent_secret(
        &self,
        key_ref: String,
        secret: String,
    ) -> Result<(), ApiFailure> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            if key_ref.is_empty() || secret.is_empty() {
                return Ok(());
            }
            kim_sdk::write_secret(&kim_sdk::account_key(&key_ref), &secret)
                .await
                .map_err(ApiFailure::from)?;
            kim_desktop_runtime::cache_secret(&key_ref, &secret);
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = (key_ref, secret);
            Ok(())
        }
    }

    /// Provider model inventory by keyRef. Desktop only; key resolves
    /// through the vault / Keychain channel. A successful fetch seeds the
    /// per-vendor catalog cache.
    pub async fn fetch_models(
        &self,
        vendor_id: String,
        base_url: String,
        key_ref: String,
    ) -> Result<Vec<String>, ApiFailure> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            let runtime = kim_desktop_runtime::installed()
                .ok_or_else(|| ApiFailure::unavailable("host agent runtime"))?;
            let models = runtime
                .fetch_models(&vendor_id, &base_url, &key_ref)
                .await
                .map_err(ApiFailure::from)?;
            let vendor = vendor_id.trim().to_string();
            if !vendor.is_empty() {
                let _ = self
                    .inner
                    .set_catalog_model_cache(&vendor, models.clone())
                    .await;
            }
            Ok(models)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = (vendor_id, base_url, key_ref);
            Err(ApiFailure::unavailable("host agent runtime"))
        }
    }

    /// Capability preview assembled from store rows. Dart passes an id.
    pub async fn preview_profile(
        &self,
        profile_id: String,
    ) -> Result<CapabilityPreview, ApiFailure> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            let runtime = kim_desktop_runtime::installed()
                .ok_or_else(|| ApiFailure::unavailable("host agent runtime"))?;
            let preview = runtime
                .preview_profile(&profile_id)
                .await
                .map_err(ApiFailure::from)?;
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
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = profile_id;
            Err(ApiFailure::unavailable("host agent runtime"))
        }
    }

    pub fn respond_agent_permission(&self, call_id: String, allow: bool) {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        kim_desktop_runtime::respond_permission(&call_id, allow);
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        let _ = (call_id, allow);
    }

    /// Desktop permission cards. Phone returns an idle stream.
    pub fn watch_agent_permission(
        &self,
        sink: StreamSink<AgentPermissionEvent>,
    ) -> Result<(), ApiFailure> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            let Some(runtime) = kim_desktop_runtime::installed() else {
                return Err(ApiFailure::unavailable("host agent runtime"));
            };
            let mut rx = runtime.permissions.subscribe();
            let _guard = super::rt().enter();
            super::rt().spawn(async move {
                while let Some(event) = rx.recv().await {
                    if sink
                        .add(AgentPermissionEvent {
                            dest: event.dest,
                            call_id: event.call_id,
                            name: event.name,
                            preview: event.preview,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = sink;
            Ok(())
        }
    }

    /// Desktop pet presence (`running` / `done` / `failed`). Phone is idle.
    pub fn watch_agent_ui(&self, sink: StreamSink<AgentUiStatus>) -> Result<(), ApiFailure> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            let Some(runtime) = kim_desktop_runtime::installed() else {
                return Err(ApiFailure::unavailable("host agent runtime"));
            };
            let mut rx = runtime.ui.subscribe();
            let _guard = super::rt().enter();
            super::rt().spawn(async move {
                while let Some(event) = rx.recv().await {
                    if sink
                        .add(AgentUiStatus {
                            dest: event.dest,
                            phase: event.phase,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = sink;
            Ok(())
        }
    }
}

pub struct AgentPermissionEvent {
    pub dest: String,
    pub call_id: String,
    pub name: String,
    pub preview: String,
}

pub struct AgentUiStatus {
    pub dest: String,
    pub phase: String,
}

pub struct PreviewTool {
    pub name: String,
    pub source: String,
    pub executor: String,
}

pub struct CapabilityPreview {
    pub tools: Vec<PreviewTool>,
    pub warnings: Vec<String>,
}
