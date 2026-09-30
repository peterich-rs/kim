//! Secure storage owned by Rust, executed by the platform. Keys are Rust
//! constants; the Keychain/Keystore backend runs in Flutter
//! (`flutter_secure_storage`) driven by a request/respond channel identical
//! in shape to `respond_agent_permission`.
//!
//! The key strings below are a data contract: they must match what the Dart
//! shell has written since v1.0. Renaming one silently loses credentials on
//! upgrade. Do not change them.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use crate::error::SdkError;

/// Keychain key for the Royal JWT.
pub const KEY_JWT: &str = "kim.jwt";
/// Keychain key for the signed-in account.
pub const KEY_ACCOUNT: &str = "kim.account";
/// Keychain key for the agent wrap key (P4).
pub const KEY_AGENT_WRAP: &str = "kim.agent_wrap";
/// Keychain key prefix for provider API keys (`account.<keyRef>`).
pub const KEY_PREFIX_ACCOUNT: &str = "account.";
/// Shared fallback provider key seeded by the Dart shell.
pub const KEY_AGENT_GOOSE: &str = "agent.api_key.goose";

#[must_use]
pub fn account_key(key_ref: &str) -> String {
    format!("{KEY_PREFIX_ACCOUNT}{key_ref}")
}

#[async_trait]
pub trait SecretStore: Send + Sync + 'static {
    async fn read(&self, key: &str) -> Result<String, SdkError>;
    async fn write(&self, key: &str, value: &str) -> Result<(), SdkError>;
    async fn delete(&self, key: &str) -> Result<(), SdkError>;
}

/// One platform round-trip. `delete` is expressed as `op = "delete"` with an
/// empty payload so FRB only needs one payload type.
#[derive(Clone, Debug)]
pub enum SecretOp {
    Read { id: u64, key: String },
    Write { id: u64, key: String, value: String },
    Delete { id: u64, key: String },
}

impl SecretOp {
    #[must_use]
    pub fn id(&self) -> u64 {
        match self {
            Self::Read { id, .. } | Self::Write { id, .. } | Self::Delete { id, .. } => *id,
        }
    }

    #[must_use]
    pub fn key(&self) -> &str {
        match self {
            Self::Read { key, .. } | Self::Write { key, .. } | Self::Delete { key, .. } => key,
        }
    }
}

const READ_TIMEOUT: Duration = Duration::from_secs(10);

static GLOBAL: Mutex<Option<Arc<ChannelSecretStore>>> = Mutex::new(None);
static PARKED_RX: Mutex<Option<tokio::sync::mpsc::Receiver<SecretOp>>> = Mutex::new(None);

/// Install the process-wide channel before Dart attaches. Ops sent in that
/// window sit in the mpsc buffer until [`take_parked_receiver`] hands the
/// receiver to the executor task. A second call is a no-op.
pub fn install_global_channel(cap: usize) {
    let mut guard = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return;
    }
    let (store, rx) = ChannelSecretStore::new(cap);
    *guard = Some(Arc::new(store));
    *PARKED_RX.lock().unwrap_or_else(|e| e.into_inner()) = Some(rx);
}

/// Take the receiver parked by [`install_global_channel`]. `None` means the
/// executor already attached, or the channel was never installed.
#[must_use]
pub fn take_parked_receiver() -> Option<tokio::sync::mpsc::Receiver<SecretOp>> {
    PARKED_RX.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// Install the process-wide secret backend. Called once by the FFI layer
/// when the Dart executor subscribes.
pub fn set_global(store: Arc<ChannelSecretStore>) {
    let mut g = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    *g = Some(store);
}

/// The installed backend, if any. Absent values degrade to signed-out.
#[must_use]
pub fn global() -> Option<Arc<ChannelSecretStore>> {
    GLOBAL.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Convenience read through the global backend. Empty when never installed.
pub async fn read_global(key: &str) -> Result<String, SdkError> {
    match global() {
        Some(store) => store.read(key).await,
        None => Ok(String::new()),
    }
}

/// Convenience write through the global backend.
pub async fn write_global(key: &str, value: &str) -> Result<(), SdkError> {
    match global() {
        Some(store) => store.write(key, value).await,
        None => Err(SdkError::Internal {
            message: "secret store not installed".into(),
        }),
    }
}

/// Convenience delete through the global backend.
pub async fn delete_global(key: &str) -> Result<(), SdkError> {
    match global() {
        Some(store) => store.delete(key).await,
        None => Ok(()),
    }
}

type Pending = HashMap<u64, tokio::sync::oneshot::Sender<Result<SecretOutcome, SdkError>>>;

/// Outcome of a platform round-trip. `None` = key absent / empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecretOutcome {
    Value(Option<String>),
}

/// Request/response channel over the FFI edge. Dart subscribes with
/// `watch_secret_requests`, executes against the Keychain, and answers with
/// `secret_store_respond`.
#[derive(Clone)]
pub struct ChannelSecretStore {
    tx: tokio::sync::mpsc::Sender<SecretOp>,
    pending: Arc<Mutex<Pending>>,
    next_id: Arc<AtomicU64>,
    timeout: Duration,
}

impl ChannelSecretStore {
    #[must_use]
    pub fn new(cap: usize) -> (Self, tokio::sync::mpsc::Receiver<SecretOp>) {
        Self::with_timeout(cap, READ_TIMEOUT)
    }

    #[must_use]
    pub fn with_timeout(
        cap: usize,
        timeout: Duration,
    ) -> (Self, tokio::sync::mpsc::Receiver<SecretOp>) {
        let (tx, rx) = tokio::sync::mpsc::channel(cap);
        (
            Self {
                tx,
                pending: Arc::new(Mutex::new(HashMap::new())),
                next_id: Arc::new(AtomicU64::new(1)),
                timeout,
            },
            rx,
        )
    }

    /// Dart answered. `Ok(None)` means "no such key".
    pub fn respond(&self, id: u64, outcome: Result<Option<String>, String>) {
        let waiter = {
            let mut pending = lock_pending(&self.pending);
            pending.remove(&id)
        };
        if let Some(tx) = waiter {
            let result = outcome
                .map(SecretOutcome::Value)
                .map_err(|message| SdkError::Internal { message });
            let _ = tx.send(result);
        }
    }

    async fn round_trip(
        &self,
        make: impl Fn(u64) -> SecretOp,
        empty_on_timeout: bool,
    ) -> Result<Result<SecretOutcome, SdkError>, SdkError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = lock_pending(&self.pending);
            pending.insert(id, tx);
        }
        let op = make(id);
        if self.tx.send(op).await.is_err() {
            let mut pending = lock_pending(&self.pending);
            pending.remove(&id);
            return Err(SdkError::Internal {
                message: "secret executor detached".into(),
            });
        }
        match tokio::time::timeout(self.timeout, rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(SdkError::Internal {
                message: "secret executor dropped request".into(),
            }),
            Err(_) => {
                let mut pending = lock_pending(&self.pending);
                pending.remove(&id);
                tracing::warn!(id, "secret store round-trip timed out");
                if empty_on_timeout {
                    return Ok(Ok(SecretOutcome::Value(None)));
                }
                Err(SdkError::Internal {
                    message: "secret store timeout".into(),
                })
            }
        }
    }
}

fn lock_pending(m: &Mutex<Pending>) -> std::sync::MutexGuard<'_, Pending> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[async_trait]
impl SecretStore for ChannelSecretStore {
    async fn read(&self, key: &str) -> Result<String, SdkError> {
        let result = self
            .round_trip(
                |id| SecretOp::Read {
                    id,
                    key: key.to_string(),
                },
                true,
            )
            .await??;
        match result {
            SecretOutcome::Value(Some(v)) => Ok(v),
            SecretOutcome::Value(None) => Ok(String::new()),
        }
    }

    async fn write(&self, key: &str, value: &str) -> Result<(), SdkError> {
        self.round_trip(
            |id| SecretOp::Write {
                id,
                key: key.to_string(),
                value: value.to_string(),
            },
            false,
        )
        .await??;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), SdkError> {
        self.round_trip(
            |id| SecretOp::Delete {
                id,
                key: key.to_string(),
            },
            false,
        )
        .await??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> (ChannelSecretStore, tokio::sync::mpsc::Receiver<SecretOp>) {
        ChannelSecretStore::new(8)
    }

    #[tokio::test]
    async fn read_round_trip() {
        let (store, mut rx) = env();
        let store2 = store.clone();
        let exec = tokio::spawn(async move {
            let op = rx.recv().await.expect("op");
            assert!(matches!(op, SecretOp::Read { .. }));
            assert_eq!(op.key(), KEY_JWT);
            store2.respond(op.id(), Ok(Some("secret-value".into())));
        });
        let v = store.read(KEY_JWT).await.expect("read");
        assert_eq!(v, "secret-value");
        exec.abort();
    }

    #[tokio::test]
    async fn write_then_missing_read() {
        let (store, mut rx) = env();
        let store2 = store.clone();
        let exec = tokio::spawn(async move {
            while let Some(op) = rx.recv().await {
                match &op {
                    SecretOp::Read { .. } => store2.respond(op.id(), Ok(None)),
                    SecretOp::Write { .. } => store2.respond(op.id(), Ok(None)),
                    SecretOp::Delete { .. } => store2.respond(op.id(), Ok(None)),
                }
            }
        });
        store.write(KEY_JWT, "tok").await.expect("write");
        let v = store.read(KEY_JWT).await.expect("read");
        assert_eq!(v, "");
        exec.abort();
    }

    #[tokio::test]
    async fn timeout_degrades_to_empty() {
        let (store, mut rx) = ChannelSecretStore::with_timeout(8, Duration::from_millis(20));
        let exec = tokio::spawn(async move {
            // Swallow the op; never respond.
            while rx.recv().await.is_some() {}
        });
        let value = store.read(KEY_JWT).await.expect("read degrades");
        assert!(value.is_empty());
        exec.abort();
    }

    #[test]
    fn account_key_prefix() {
        assert_eq!(account_key("vendor1"), "account.vendor1");
    }
}
