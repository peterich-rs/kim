//! One-shot platform bootstrap and the Dart-side secure-storage executor
//! channel. After these two calls the FFI surface carries no paths, no
//! versions, and no secrets.

use kim_sdk::{PlatformBootstrap, SecretOp};

use super::failure::ApiFailure;
use crate::frb_generated::StreamSink;

/// The only platform facts Rust cannot derive: directory roots, build
/// identity, simulator flag. Idempotent: a second call is `Ok(())` without
/// overwriting.
pub async fn platform_bootstrap(
    documents: String,
    support: String,
    cache: String,
    temp: String,
    app_version: String,
    build_number: String,
    simulator: bool,
) -> Result<(), ApiFailure> {
    let bootstrap = PlatformBootstrap {
        documents: documents.into(),
        support: support.into(),
        cache: cache.into(),
        temp: temp.into(),
        app_version: app_version.trim().to_string(),
        build_number: build_number.trim().to_string(),
        simulator,
    };
    for dir in [
        &bootstrap.documents,
        &bootstrap.support,
        &bootstrap.cache,
        &bootstrap.temp,
    ] {
        std::fs::create_dir_all(dir).map_err(|e| ApiFailure::InvalidArgument {
            message: format!("bootstrap dir {}: {e}", dir.display()),
        })?;
    }
    kim_sdk::set_bootstrap(bootstrap).map_err(|e| ApiFailure::InvalidArgument {
        message: e.to_string(),
    })?;
    // Park the secret channel before Dart subscribes so reads issued during
    // attach queue instead of resolving as "no store".
    kim_sdk::install_global_secret_channel(16);
    Ok(())
}

/// Projection of a [`SecretOp`] for FRB. `SecretRequest` is what Dart sees.
pub struct SecretRequest {
    pub id: u64,
    pub op: String,
    pub key: String,
    pub value: String,
}

impl From<&SecretOp> for SecretRequest {
    fn from(op: &SecretOp) -> Self {
        match op {
            SecretOp::Read { id, key } => Self {
                id: *id,
                op: "read".into(),
                key: key.clone(),
                value: String::new(),
            },
            SecretOp::Write { id, key, value } => Self {
                id: *id,
                op: "write".into(),
                key: key.clone(),
                value: value.clone(),
            },
            SecretOp::Delete { id, key } => Self {
                id: *id,
                op: "delete".into(),
                key: key.clone(),
                value: String::new(),
            },
        }
    }
}

/// Dart subscribes once at boot; each event is a Keychain/Keystore action to
/// execute via the `kim.keystore` platform channel. Answer with `secret_store_respond`.
pub fn watch_secret_requests(sink: StreamSink<SecretRequest>) -> Result<(), ApiFailure> {
    if kim_sdk::global_secret_store().is_none() {
        kim_sdk::install_global_secret_channel(16);
    }
    let Some(mut rx) = kim_sdk::take_parked_secret_receiver() else {
        return Err(ApiFailure::InvalidArgument {
            message: "secret executor already attached".into(),
        });
    };
    let _guard = super::rt().enter();
    super::rt().spawn(async move {
        while let Some(op) = rx.recv().await {
            if sink.add(SecretRequest::from(&op)).is_err() {
                tracing::warn!("secret request sink closed; executor detached");
                break;
            }
        }
    });
    Ok(())
}

/// Dart answers a [`SecretRequest`]. `ok == false` is an executor error;
/// `ok == true` with `value == null` means "key absent".
pub fn secret_store_respond(id: u64, ok: bool, value: Option<String>) {
    let Some(store) = kim_sdk::global_secret_store() else {
        return;
    };
    let outcome = if ok {
        Ok(value.filter(|v| !v.trim().is_empty()))
    } else {
        Err("executor error".to_string())
    };
    store.respond(id, outcome);
}
