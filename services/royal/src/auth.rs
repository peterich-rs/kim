//! Register / login / logout protobuf endpoints.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use chat::users::UserError;
use kim_protocol::pkt::{AuthReq, AuthResp};
use kim_protocol::{generate, parse, ProtocolError};

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

fn issue(st: &RoyalState, account: &str) -> AuthResult<Bytes> {
    let ttl = if st.jwt.ttl_secs > 0 {
        st.jwt.ttl_secs
    } else {
        86_400
    };
    let exp = now_ts().saturating_add(ttl);
    let token = generate(&st.jwt.secret, account, &st.app, exp).map_err(|e| match e {
        ProtocolError::InvalidAccount => bad_request("invalid account"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "token".into()),
    })?;
    Ok(encode(&AuthResp {
        token,
        exp,
        account: account.to_string(),
    }))
}

pub async fn register(State(st): State<RoyalState>, body: Bytes) -> AuthResult<Bytes> {
    let req = decode::<AuthReq>(&body)?;
    let account = valid_account(&req.account)?.to_string();
    let password = valid_password(&req.password)?.to_string();
    let hashed = tokio::task::spawn_blocking(move || hash_password(password))
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "hash".into()))??;
    match st.users.create(&st.app, &account, &hashed).await {
        Ok(()) => issue(&st, &account),
        Err(UserError::Conflict) => Err((StatusCode::CONFLICT, "账号已存在".into())),
        Err(UserError::Backend(e)) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
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
    issue(&st, &account)
}

pub async fn logout(State(st): State<RoyalState>, headers: HeaderMap) -> AuthResult<StatusCode> {
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
