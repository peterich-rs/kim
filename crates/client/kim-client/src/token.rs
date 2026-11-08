use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::ClientError;

const EXP_SKEW_SECS: i64 = 30;

#[derive(Debug, Deserialize)]
struct Payload {
    acc: Option<String>,
    exp: Option<i64>,
}

/// Unverified JWT payload. The gateway checks the signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnverifiedClaims {
    pub account: String,
    pub exp: i64,
}

/// Read `acc` from a JWT payload. Does not verify the signature; the gateway does.
pub fn account_from_token(token: &str) -> Result<String, ClientError> {
    Ok(unverified_claims(token)?.account)
}

pub fn unverified_claims(token: &str) -> Result<UnverifiedClaims, ClientError> {
    let mut parts = token.split('.');
    let _header = parts.next().ok_or(ClientError::InvalidToken)?;
    let payload = parts.next().ok_or(ClientError::InvalidToken)?;
    if payload.is_empty() {
        return Err(ClientError::InvalidToken);
    }
    let json = decode_b64url(payload)?;
    let body: Payload = serde_json::from_slice(&json).map_err(|_| ClientError::InvalidToken)?;
    let account = match body.acc {
        Some(acc) if !acc.is_empty() => acc,
        _ => return Err(ClientError::InvalidToken),
    };
    let Some(exp) = body.exp else {
        return Err(ClientError::InvalidToken);
    };
    Ok(UnverifiedClaims { account, exp })
}

/// `Some` when this token must not be retried (expired, malformed, missing claims).
pub fn token_unusable(token: &str) -> Option<ClientError> {
    match unverified_claims(token) {
        Err(err) => Some(err),
        Ok(claims) if claims.exp <= now_unix().saturating_add(EXP_SKEW_SECS) => {
            Some(ClientError::Unauthorized)
        }
        Ok(_) => None,
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn decode_b64url(s: &str) -> Result<Vec<u8>, ClientError> {
    use base64::engine::{general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| ClientError::InvalidToken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_protocol::{generate, DEMO_DEFAULT_SECRET};

    #[test]
    fn reads_acc_without_verifying() {
        let tok = generate(DEMO_DEFAULT_SECRET, "alice", "kim", 4_000_000_000).unwrap();
        assert_eq!(account_from_token(&tok).unwrap(), "alice");
    }

    #[test]
    fn rejects_garbage() {
        assert!(account_from_token("not-a-jwt").is_err());
        assert!(account_from_token("a.b").is_err());
        assert!(account_from_token("").is_err());
        assert!(token_unusable("not-a-jwt").is_some());
    }

    #[test]
    fn expired_token_is_unusable() {
        let tok = generate(DEMO_DEFAULT_SECRET, "alice", "kim", 1).unwrap();
        assert!(matches!(
            token_unusable(&tok),
            Some(ClientError::Unauthorized)
        ));
    }

    #[test]
    fn future_token_is_usable() {
        let tok = generate(DEMO_DEFAULT_SECRET, "alice", "kim", 4_000_000_000).unwrap();
        assert!(token_unusable(&tok).is_none());
        assert_eq!(unverified_claims(&tok).unwrap().account, "alice");
    }
}
