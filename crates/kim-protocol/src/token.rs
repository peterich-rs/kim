use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::ProtocolError;

/// Demo-only. Same value as the booklet `token.DefaultSecret`. Production must
/// override via env / config; never use this as a live key.
pub const DEMO_DEFAULT_SECRET: &str = "jwt-1sNzdiSgnNuxyq2g7xml2JvLArU";

/// Booklet JWT payload: `acc` / `app` / `exp`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    #[serde(rename = "acc")]
    pub account: String,
    #[serde(rename = "app")]
    pub app: String,
    pub exp: i64,
}

pub fn generate(secret: &str, account: &str, app: &str, exp: i64) -> Result<String, ProtocolError> {
    check_account(account)?;
    let claims = Claims {
        account: account.to_string(),
        app: app.to_string(),
        exp,
    };
    jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(map_jwt)
}

pub fn parse(secret: &str, token: &str) -> Result<Claims, ProtocolError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.algorithms = vec![Algorithm::HS256];
    validation.validate_exp = true;
    let data = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(map_jwt)?;
    check_account(&data.claims.account)?;
    Ok(data.claims)
}

fn check_account(account: &str) -> Result<(), ProtocolError> {
    if account.is_empty() || account.chars().any(|c| c.is_ascii_control()) {
        return Err(ProtocolError::InvalidAccount);
    }
    Ok(())
}

fn map_jwt(err: jsonwebtoken::errors::Error) -> ProtocolError {
    match err.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => ProtocolError::TokenExpired,
        jsonwebtoken::errors::ErrorKind::InvalidSignature => ProtocolError::TokenSignature,
        _ => ProtocolError::InvalidToken,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_ts() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    #[test]
    fn roundtrip() {
        let exp = now_ts() + 3600;
        let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim", exp).unwrap();
        let claims = parse(DEMO_DEFAULT_SECRET, &token).unwrap();
        assert_eq!(claims.account, "alice");
        assert_eq!(claims.app, "kim");
        assert_eq!(claims.exp, exp);
    }

    #[test]
    fn expired() {
        let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim", 1).unwrap();
        assert!(matches!(
            parse(DEMO_DEFAULT_SECRET, &token),
            Err(ProtocolError::TokenExpired)
        ));
    }

    #[test]
    fn flipped_byte() {
        let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim", now_ts() + 3600).unwrap();
        let mut chars: Vec<char> = token.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'A' { 'B' } else { 'A' };
        let flipped: String = chars.into_iter().collect();
        assert!(matches!(
            parse(DEMO_DEFAULT_SECRET, &flipped),
            Err(ProtocolError::TokenSignature | ProtocolError::InvalidToken)
        ));
    }

    #[test]
    fn alg_none_rejected() {
        let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim", now_ts() + 3600).unwrap();
        let payload = token.split('.').nth(1).unwrap();
        let none_token = format!("eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.{payload}.");
        assert!(matches!(
            parse(DEMO_DEFAULT_SECRET, &none_token),
            Err(ProtocolError::InvalidToken | ProtocolError::TokenSignature)
        ));
    }

    #[test]
    fn empty_account() {
        let exp = now_ts() + 3600;
        assert!(matches!(
            generate(DEMO_DEFAULT_SECRET, "", "kim", exp),
            Err(ProtocolError::InvalidAccount)
        ));

        #[derive(Serialize)]
        struct Raw {
            acc: &'static str,
            app: &'static str,
            exp: i64,
        }
        let minted = jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &Raw {
                acc: "",
                app: "kim",
                exp,
            },
            &EncodingKey::from_secret(DEMO_DEFAULT_SECRET.as_bytes()),
        )
        .unwrap();
        assert!(matches!(
            parse(DEMO_DEFAULT_SECRET, &minted),
            Err(ProtocolError::InvalidAccount)
        ));
    }
}
