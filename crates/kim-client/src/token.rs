use serde::Deserialize;

use crate::ClientError;

#[derive(Debug, Deserialize)]
struct Payload {
    acc: Option<String>,
}

/// Read `acc` from a JWT payload. Does not verify the signature; the gateway does.
pub fn account_from_token(token: &str) -> Result<String, ClientError> {
    let mut parts = token.split('.');
    let _header = parts.next().ok_or(ClientError::InvalidToken)?;
    let payload = parts.next().ok_or(ClientError::InvalidToken)?;
    if payload.is_empty() {
        return Err(ClientError::InvalidToken);
    }
    let json = decode_b64url(payload)?;
    let body: Payload = serde_json::from_slice(&json).map_err(|_| ClientError::InvalidToken)?;
    match body.acc {
        Some(acc) if !acc.is_empty() => Ok(acc),
        _ => Err(ClientError::InvalidToken),
    }
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
    }
}
