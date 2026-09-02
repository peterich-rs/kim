//! Register / login / logout protobuf endpoints.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chat::users::UserError;
use kim_protocol::pkt::{AuthReq, AuthResp, PasswordChangeReq};
use kim_protocol::{generate_with_device, parse, ProtocolError};
use serde::Serialize;

use crate::device::{hash_secret, new_device_id, new_secret};
use crate::{decode, encode, now_ts, RoyalState};

const ACCOUNT_MIN: usize = 3;
const ACCOUNT_MAX: usize = 32;
const PASSWORD_MIN: usize = 8;
const PASSWORD_MAX: usize = 128;

type AuthResult<T> = Result<T, (StatusCode, String)>;

fn bad_request(msg: &str) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.into())
}

fn unauthorized() -> (StatusCode, String) {
    (StatusCode::UNAUTHORIZED, "账号或密码错误".into())
}

fn valid_account(raw: &str) -> AuthResult<&str> {
    let s = raw.trim();
    if s.len() < ACCOUNT_MIN || s.len() > ACCOUNT_MAX {
        return Err(bad_request("invalid account"));
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(bad_request("invalid account"));
    }
    Ok(s)
}

fn valid_password(raw: &str) -> AuthResult<&str> {
    if raw.len() < PASSWORD_MIN || raw.len() > PASSWORD_MAX {
        return Err(bad_request("invalid password"));
    }
    Ok(raw)
}

fn hash_password(password: String) -> AuthResult<String> {
    let salt = SaltString::generate(&mut rand_core::OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "hash".into()))
}

fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

struct IssuedAuth {
    bytes: Bytes,
}

async fn issue(
    st: &RoyalState,
    account: &str,
    did: Option<&str>,
    device_secret: Option<String>,
) -> AuthResult<IssuedAuth> {
    let ttl = if st.jwt.ttl_secs > 0 {
        st.jwt.ttl_secs
    } else {
        86_400
    };
    let exp = now_ts().saturating_add(ttl);
    let ver = st
        .users
        .token_epoch(&st.app, account)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let token = generate_with_device(
        &st.jwt.secret,
        account,
        &st.app,
        exp,
        &uuid::Uuid::new_v4().to_string(),
        ver,
        did,
    )
    .map_err(|e| match e {
        ProtocolError::InvalidAccount => bad_request("invalid account"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "token".into()),
    })?;
    Ok(IssuedAuth {
        bytes: encode(&AuthResp {
            token,
            exp,
            account: account.to_string(),
            device_id: did.unwrap_or("").to_string(),
            device_credential: device_secret.unwrap_or_default(),
        }),
    })
}

struct BoundDevice {
    id: String,
    secret: Option<String>,
}

async fn bind_device(
    st: &RoyalState,
    account: &str,
    req: &AuthReq,
) -> AuthResult<Option<BoundDevice>> {
    let cred = req.device_credential.trim();
    if !cred.is_empty() {
        let hash = hash_secret(cred);
        let rec = st
            .devices
            .lookup_hash(&st.app, account, &hash)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let Some(rec) = rec else {
            return Err(unauthorized());
        };
        st.device_hot
            .put(&rec.device_id, account)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Some(BoundDevice {
            id: rec.device_id,
            secret: None,
        }));
    }
    if req.enroll_device {
        let id = new_device_id();
        let secret = new_secret();
        let hash = hash_secret(&secret);
        st.device_hot
            .put(&id, account)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if let Err(err) = st.devices.enroll(&st.app, account, &id, &hash).await {
            if let Err(hot_err) = st.device_hot.drop_key(&id).await {
                tracing::warn!(%hot_err, "device hot drop after enroll fail");
            }
            return Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string()));
        }
        return Ok(Some(BoundDevice {
            id,
            secret: Some(secret),
        }));
    }
    Ok(None)
}

async fn finish_auth(st: &RoyalState, account: &str, req: &AuthReq) -> AuthResult<Bytes> {
    let bound = bind_device(st, account, req).await?;
    match bound {
        Some(b) => Ok(issue(st, account, Some(&b.id), b.secret).await?.bytes),
        None => Ok(issue(st, account, None, None).await?.bytes),
    }
}

pub async fn register(State(st): State<RoyalState>, body: Bytes) -> AuthResult<Bytes> {
    let req = decode::<AuthReq>(&body)?;
    let account = valid_account(&req.account)?.to_string();
    let password = valid_password(&req.password)?.to_string();
    let hashed = tokio::task::spawn_blocking(move || hash_password(password))
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "hash".into()))??;
    match st.users.create(&st.app, &account, &hashed).await {
        Ok(()) => finish_auth(&st, &account, &req).await,
        Err(UserError::Conflict) => Err((StatusCode::CONFLICT, "账号已存在".into())),
        Err(UserError::Backend(e)) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
        Err(UserError::NotFound | UserError::InvalidProfile) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, "create".into()))
        }
    }
}

pub async fn login(State(st): State<RoyalState>, body: Bytes) -> AuthResult<Bytes> {
    let req = decode::<AuthReq>(&body)?;
    let account = valid_account(&req.account)?.to_string();
    let password = valid_password(&req.password)?.to_string();
    let stored = st
        .users
        .password_hash(&st.app, &account)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let Some(hash) = stored else {
        return Err(unauthorized());
    };
    let ok = tokio::task::spawn_blocking(move || verify_password(&password, &hash))
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "verify".into()))?;
    if !ok {
        return Err(unauthorized());
    }
    finish_auth(&st, &account, &req).await
}

pub async fn logout(State(st): State<RoyalState>, headers: HeaderMap) -> AuthResult<StatusCode> {
    let claims = bearer_claims(&st, &headers)?;
    if let Some(jti) = claims.jti.as_deref() {
        let ttl = claims.exp.saturating_sub(now_ts()).max(1);
        let ttl = u64::try_from(ttl).unwrap_or(1);
        st.revoke
            .revoke(jti, ttl)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    crate::kick_account(&st, &claims.account).await;
    Ok(StatusCode::NO_CONTENT)
}

fn bearer_claims(st: &RoyalState, headers: &HeaderMap) -> AuthResult<kim_protocol::Claims> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .unwrap_or("")
        .trim();
    if token.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "unauthorized".into()));
    }
    let claims = parse(&st.jwt.secret, token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "unauthorized".into()))?;
    if claims.app != st.app || claims.account.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "unauthorized".into()));
    }
    Ok(claims)
}

async fn live_claims(st: &RoyalState, headers: &HeaderMap) -> AuthResult<kim_protocol::Claims> {
    let claims = bearer_claims(st, headers)?;
    if let Some(jti) = claims.jti.as_deref() {
        if st
            .revoke
            .is_revoked(jti)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        {
            return Err((StatusCode::UNAUTHORIZED, "unauthorized".into()));
        }
    }
    let epoch = account_epoch(st, &claims.account).await?;
    if claims.ver < epoch {
        return Err((StatusCode::UNAUTHORIZED, "unauthorized".into()));
    }
    Ok(claims)
}

fn epoch_ttl_secs(st: &RoyalState) -> u64 {
    if st.jwt.ttl_secs > 0 {
        u64::try_from(st.jwt.ttl_secs).unwrap_or(86_400)
    } else {
        86_400
    }
}

/// Revoke cache can miss after restart or Redis eviction; `users.token_epoch` is durable.
pub(crate) async fn account_epoch(st: &RoyalState, account: &str) -> AuthResult<u32> {
    let cached = st
        .revoke
        .get_epoch(account)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let durable = st
        .users
        .token_epoch(&st.app, account)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let epoch = cached.max(durable);
    if durable > cached {
        if let Err(err) = st
            .revoke
            .set_epoch(account, epoch, epoch_ttl_secs(st))
            .await
        {
            tracing::warn!(%err, account, "warm token epoch");
        }
    }
    Ok(epoch)
}

#[derive(Serialize)]
pub struct MeBody {
    pub account: String,
    pub app: String,
}

pub async fn me(State(st): State<RoyalState>, headers: HeaderMap) -> AuthResult<Json<MeBody>> {
    let claims = live_claims(&st, &headers).await?;
    Ok(Json(MeBody {
        account: claims.account,
        app: st.app.clone(),
    }))
}

pub async fn change_password(
    State(st): State<RoyalState>,
    headers: HeaderMap,
    body: Bytes,
) -> AuthResult<StatusCode> {
    let claims = live_claims(&st, &headers).await?;
    let account = claims.account;
    let req = decode::<PasswordChangeReq>(&body)?;
    let old = valid_password(&req.old_password)?.to_string();
    let new = valid_password(&req.new_password)?.to_string();
    let stored = st
        .users
        .password_hash(&st.app, &account)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let Some(hash) = stored else {
        return Err(unauthorized());
    };
    let old_ok = tokio::task::spawn_blocking({
        let hash = hash.clone();
        move || verify_password(&old, &hash)
    })
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "verify".into()))?;
    if !old_ok {
        return Err(unauthorized());
    }
    let hashed = tokio::task::spawn_blocking(move || hash_password(new))
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "hash".into()))??;
    let ver = st
        .users
        .set_password_and_bump_epoch(&st.app, &account, &hashed)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Err(err) = st
        .revoke
        .set_epoch(&account, ver, epoch_ttl_secs(&st))
        .await
    {
        tracing::error!(%err, account, "set token epoch");
        return Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string()));
    }
    if let Some(jti) = claims.jti.as_deref() {
        let jti_ttl = claims.exp.saturating_sub(now_ts()).max(1);
        let jti_ttl = u64::try_from(jti_ttl).unwrap_or(1);
        if let Err(err) = st.revoke.revoke(jti, jti_ttl).await {
            tracing::error!(%err, account, "revoke current jti");
        }
    }
    crate::kick_account(&st, &account).await;
    Ok(StatusCode::NO_CONTENT)
}
