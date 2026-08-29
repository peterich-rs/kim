use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::ProtocolError;

/// Demo-only. Same value as the booklet `token.DefaultSecret`. Production must
/// override via env / config; never use this as a live key.
pub const DEMO_DEFAULT_SECRET: &str = "jwt-1sNzdiSgnNuxyq2g7xml2JvLArU";

/// JWT payload: `acc` / `app` / `exp` / optional `jti` (login/register).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    #[serde(rename = "acc")]
    pub account: String,
    #[serde(rename = "app")]
    pub app: String,
    pub exp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
}

pub fn token_revoke_key(jti: &str) -> String {
    format!("kim:revoke:{jti}")
}

pub fn generate(secret: &str, account: &str, app: &str, exp: i64) -> Result<String, ProtocolError> {
    generate_with_jti(secret, account, app, exp, &uuid::Uuid::new_v4().to_string())
}

pub fn generate_with_jti(
    secret: &str,
    account: &str,
    app: &str,
    exp: i64,
    jti: &str,
) -> Result<String, ProtocolError> {
    check_account(account)?;
    let jti = jti.trim();
    let claims = Claims {
        account: account.to_string(),
        app: app.to_string(),
        exp,
        jti: if jti.is_empty() {
            None
        } else {
            Some(jti.to_string())
        },
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

    #[derive(Serialize)]
    struct Raw {
        acc: String,
        app: &'static str,
        exp: i64,
    }

    fn mint_hs256(acc: &str, exp: i64) -> String {
        jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &Raw {
                acc: acc.to_string(),
                app: "kim",
                exp,
            },
            &EncodingKey::from_secret(DEMO_DEFAULT_SECRET.as_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn roundtrip() {
        let exp = now_ts() + 3600;
        let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim", exp).unwrap();
        let claims = parse(DEMO_DEFAULT_SECRET, &token).unwrap();
        assert_eq!(claims.account, "alice");
        assert_eq!(claims.app, "kim");
        assert_eq!(claims.exp, exp);
        assert!(claims.jti.as_ref().is_some_and(|j| !j.is_empty()));
    }

    #[test]
    fn renew_keeps_jti() {
        let exp = now_ts() + 3600;
        let token =
            generate_with_jti(DEMO_DEFAULT_SECRET, "alice", "kim", exp, "same-jti").unwrap();
        let claims = parse(DEMO_DEFAULT_SECRET, &token).unwrap();
        assert_eq!(claims.jti.as_deref(), Some("same-jti"));
        let later =
            generate_with_jti(DEMO_DEFAULT_SECRET, "alice", "kim", exp + 60, "same-jti").unwrap();
        let again = parse(DEMO_DEFAULT_SECRET, &later).unwrap();
        assert_eq!(again.jti.as_deref(), Some("same-jti"));
        assert_eq!(again.exp, exp + 60);
    }

    #[test]
    fn parse_legacy_without_jti() {
        let exp = now_ts() + 3600;
        let claims = parse(DEMO_DEFAULT_SECRET, &mint_hs256("alice", exp)).unwrap();
        assert_eq!(claims.account, "alice");
        assert_eq!(claims.jti, None);
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
            Err(ProtocolError::InvalidToken)
        ));
    }

    #[test]
    fn alg_rs256_rejected() {
        let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim", now_ts() + 3600).unwrap();
        let payload = token.split('.').nth(1).unwrap();
        let rs256_token = format!("eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.{payload}.dGVzdA");
        assert!(matches!(
            parse(DEMO_DEFAULT_SECRET, &rs256_token),
            Err(ProtocolError::InvalidToken)
        ));
    }

    #[test]
    fn empty_account() {
        let exp = now_ts() + 3600;
        assert!(matches!(
            generate(DEMO_DEFAULT_SECRET, "", "kim", exp),
            Err(ProtocolError::InvalidAccount)
        ));
        assert!(matches!(
            parse(DEMO_DEFAULT_SECRET, &mint_hs256("", exp)),
            Err(ProtocolError::InvalidAccount)
        ));
    }

    #[test]
    fn control_char_account() {
        let exp = now_ts() + 3600;
        for acc in ["a\nb", "a\0b"] {
            assert!(
                matches!(
                    generate(DEMO_DEFAULT_SECRET, acc, "kim", exp),
                    Err(ProtocolError::InvalidAccount)
                ),
                "generate({acc:?})"
            );
            assert!(
                matches!(
                    parse(DEMO_DEFAULT_SECRET, &mint_hs256(acc, exp)),
                    Err(ProtocolError::InvalidAccount)
                ),
                "parse({acc:?})"
            );
        }
    }
}
