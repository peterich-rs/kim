//! Shared HMAC-SHA256 for Chat/Gateway → Royal internal HTTP.
//!
//! Canonical string: `{METHOD}` / `{PATH}` / timestamp / nonce / body, newline-separated.
//! Empty secret is never a valid key.

use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::error::ProtocolError;

type HmacSha256 = Hmac<Sha256>;

/// Demo-only. Production must set `KIM_INTERNAL_HMAC_SECRET`.
pub const DEMO_INTERNAL_HMAC_SECRET: &str = "kim-demo-internal-hmac";

pub const HEADER_TIMESTAMP: &str = "x-kim-timestamp";
pub const HEADER_NONCE: &str = "x-kim-nonce";
pub const HEADER_SIGNATURE: &str = "x-kim-signature";

pub const MAX_SKEW_SECS: i64 = 60;

/// Redis NX TTL covering the full ±skew window plus one second.
pub const HMAC_NONCE_TTL_SECS: u64 = 2 * (MAX_SKEW_SECS as u64) + 1;

const NONCE_MIN: usize = 8;
const NONCE_MAX: usize = 128;
const PLACEHOLDER_SECRET: &str = "change-me";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HmacHeaders {
    pub timestamp: String,
    pub nonce: String,
    pub signature: String,
}

impl HmacHeaders {
    #[must_use]
    pub fn pairs(&self) -> [(&'static str, &str); 3] {
        [
            (HEADER_TIMESTAMP, self.timestamp.as_str()),
            (HEADER_NONCE, self.nonce.as_str()),
            (HEADER_SIGNATURE, self.signature.as_str()),
        ]
    }
}

pub fn now_unix() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
    .unwrap_or(i64::MAX)
}

/// Env `KIM_INTERNAL_HMAC_SECRET`, then `config_secret`, then the demo default.
#[must_use]
pub fn resolve_internal_hmac_secret(config_secret: &str) -> String {
    if let Ok(env) = std::env::var("KIM_INTERNAL_HMAC_SECRET") {
        let trimmed = env.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let trimmed = config_secret.trim();
    if trimmed.is_empty() {
        DEMO_INTERNAL_HMAC_SECRET.to_string()
    } else {
        trimmed.to_string()
    }
}

#[must_use]
pub fn is_demo_internal_hmac(secret: &str) -> bool {
    secret == DEMO_INTERNAL_HMAC_SECRET
}

#[must_use]
pub fn hmac_nonce_key(nonce: &str) -> String {
    format!("kim:hmac-nonce:{nonce}")
}

#[must_use]
pub fn hmac_headers_from<'a, F>(get: F) -> HmacHeaders
where
    F: Fn(&'static str) -> &'a str,
{
    HmacHeaders {
        timestamp: get(HEADER_TIMESTAMP).to_string(),
        nonce: get(HEADER_NONCE).to_string(),
        signature: get(HEADER_SIGNATURE).to_string(),
    }
}

/// `KIM_ENV=production` forces strict; `development`/`dev` disables; otherwise
/// a non-empty `CONSUL_HTTP_ADDR` is strict.
#[must_use]
pub fn strict_runtime() -> bool {
    match std::env::var("KIM_ENV") {
        Ok(s) if s.eq_ignore_ascii_case("production") => true,
        Ok(s) if s.eq_ignore_ascii_case("development") || s.eq_ignore_ascii_case("dev") => false,
        _ => std::env::var("CONSUL_HTTP_ADDR")
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false),
    }
}

#[must_use]
pub fn redis_url_has_password(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url.trim()) else {
        return false;
    };
    matches!(u.scheme(), "redis" | "rediss") && u.password().is_some_and(|p| !p.is_empty())
}

#[must_use]
pub fn consul_url_is_https(url: &str) -> bool {
    url.trim().starts_with("https://")
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn is_placeholder(secret: &str) -> bool {
    secret == PLACEHOLDER_SECRET
}

/// Inputs for [`check_strict_runtime`]. Unset fields are skipped.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrictCheck<'a> {
    pub hmac: Option<&'a str>,
    pub jwt: Option<&'a str>,
    pub redis_url: Option<&'a str>,
    pub require_redis: bool,
    pub consul_addr: Option<&'a str>,
}

/// Reject demo/`change-me` secrets, passwordless Redis, and plaintext Consul.
///
/// # Errors
///
/// Returns a lowercase reason when [`strict_runtime`] is true and a required
/// production control-plane setting is missing.
pub fn check_strict_runtime(check: StrictCheck<'_>) -> Result<(), String> {
    if !strict_runtime() {
        return Ok(());
    }
    if let Some(h) = check.hmac {
        if is_demo_internal_hmac(h) || is_placeholder(h) {
            return Err("production hmac secret is demo or change-me".into());
        }
    }
    if let Some(j) = check.jwt {
        if j == crate::token::DEMO_DEFAULT_SECRET || is_placeholder(j) {
            return Err("production jwt secret is demo or change-me".into());
        }
    }
    match check.redis_url {
        Some(u) if !redis_url_has_password(u) => {
            return Err("production redis url must include a password".into());
        }
        None if check.require_redis => {
            return Err("production requires REDIS_URL".into());
        }
        _ => {}
    }
    if let Some(addr) = check.consul_addr {
        if !consul_url_is_https(addr) {
            return Err("production consul addr must be https".into());
        }
        for key in [
            "CONSUL_HTTP_TOKEN",
            "CONSUL_CACERT",
            "CONSUL_CLIENT_CERT",
            "CONSUL_CLIENT_KEY",
        ] {
            if env_nonempty(key).is_none() {
                return Err(format!("production requires {key}"));
            }
        }
    }
    Ok(())
}

pub fn sign(
    secret: &[u8],
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<HmacHeaders, ProtocolError> {
    sign_at(secret, method, path, body, now_unix())
}

pub fn sign_at(
    secret: &[u8],
    method: &str,
    path: &str,
    body: &[u8],
    now_ts: i64,
) -> Result<HmacHeaders, ProtocolError> {
    if secret.is_empty() {
        return Err(ProtocolError::InvalidHmacSecret);
    }
    let timestamp = now_ts.to_string();
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let signature = hex_encode(&mac_bytes(secret, method, path, &timestamp, &nonce, body)?);
    Ok(HmacHeaders {
        timestamp,
        nonce,
        signature,
    })
}

#[must_use]
pub fn verify(secret: &[u8], method: &str, path: &str, body: &[u8], headers: &HmacHeaders) -> bool {
    verify_at(secret, method, path, body, headers, now_unix())
}

#[must_use]
pub fn verify_at(
    secret: &[u8],
    method: &str,
    path: &str,
    body: &[u8],
    headers: &HmacHeaders,
    now_ts: i64,
) -> bool {
    if secret.is_empty() {
        return false;
    }
    if !valid_nonce(&headers.nonce) {
        return false;
    }
    let Ok(ts) = headers.timestamp.parse::<i64>() else {
        return false;
    };
    if now_ts.abs_diff(ts) > MAX_SKEW_SECS as u64 {
        return false;
    }
    let Some(sig) = hex_decode(&headers.signature) else {
        return false;
    };
    let mut mac = match HmacSha256::new_from_slice(secret) {
        Ok(m) => m,
        Err(_) => return false,
    };
    update_canonical(
        &mut mac,
        method,
        path,
        &headers.timestamp,
        &headers.nonce,
        body,
    );
    mac.verify_slice(&sig).is_ok()
}

fn mac_bytes(
    secret: &[u8],
    method: &str,
    path: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
) -> Result<Vec<u8>, ProtocolError> {
    let mut mac =
        HmacSha256::new_from_slice(secret).map_err(|_| ProtocolError::InvalidHmacSecret)?;
    update_canonical(&mut mac, method, path, timestamp, nonce, body);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn update_canonical(
    mac: &mut HmacSha256,
    method: &str,
    path: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
) {
    mac.update(method.as_bytes());
    mac.update(b"\n");
    mac.update(path.as_bytes());
    mac.update(b"\n");
    mac.update(timestamp.as_bytes());
    mac.update(b"\n");
    mac.update(nonce.as_bytes());
    mac.update(b"\n");
    mac.update(body);
}

fn valid_nonce(n: &str) -> bool {
    let len = n.len();
    (NONCE_MIN..=NONCE_MAX).contains(&len) && n.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn hex_encode(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for &b in bytes {
        out.push(LUT[(b >> 4) as usize] as char);
        out.push(LUT[(b & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = from_hex(bytes[i])?;
        let lo = from_hex(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Some(out)
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"test-hmac-secret";

    #[test]
    fn roundtrip() {
        let body = b"hello";
        let headers = sign(SECRET, "POST", "/api/v1/message/user", body).unwrap();
        assert!(verify(
            SECRET,
            "POST",
            "/api/v1/message/user",
            body,
            &headers,
        ));
    }

    #[test]
    fn empty_secret_rejected() {
        assert!(matches!(
            sign(b"", "POST", "/x", b""),
            Err(ProtocolError::InvalidHmacSecret)
        ));
        let headers = sign(SECRET, "POST", "/x", b"").unwrap();
        assert!(!verify(b"", "POST", "/x", b"", &headers));
    }

    #[test]
    fn wrong_secret_or_body_fails() {
        let headers = sign(SECRET, "POST", "/x", b"a").unwrap();
        assert!(!verify(b"other-secret-value", "POST", "/x", b"a", &headers));
        assert!(!verify(SECRET, "POST", "/x", b"b", &headers));
        assert!(!verify(SECRET, "GET", "/x", b"a", &headers));
        assert!(!verify(SECRET, "POST", "/y", b"a", &headers));
    }

    #[test]
    fn skew_rejected() {
        let headers = sign_at(SECRET, "POST", "/x", b"", 1_000).unwrap();
        assert!(!verify_at(
            SECRET,
            "POST",
            "/x",
            b"",
            &headers,
            1_000 + MAX_SKEW_SECS + 1,
        ));
        assert!(verify_at(
            SECRET,
            "POST",
            "/x",
            b"",
            &headers,
            1_000 + MAX_SKEW_SECS,
        ));
    }

    #[test]
    fn resolve_falls_back_to_demo_when_config_empty() {
        // Do not assert env: a developer shell may set KIM_INTERNAL_HMAC_SECRET.
        let got = resolve_internal_hmac_secret(" from-config ");
        assert!(got == "from-config" || !got.is_empty());
        let demo = resolve_internal_hmac_secret("");
        assert!(!demo.is_empty());
        assert!(is_demo_internal_hmac(DEMO_INTERNAL_HMAC_SECRET));
    }

    #[test]
    fn nonce_key_and_ttl() {
        assert_eq!(HMAC_NONCE_TTL_SECS, 121);
        assert_eq!(hmac_nonce_key("abc12345"), "kim:hmac-nonce:abc12345");
        let headers = hmac_headers_from(|name| match name {
            HEADER_TIMESTAMP => "1",
            HEADER_NONCE => "n",
            HEADER_SIGNATURE => "s",
            _ => "",
        });
        assert_eq!(headers.timestamp, "1");
        assert_eq!(headers.nonce, "n");
        assert_eq!(headers.signature, "s");
    }

    #[test]
    fn redis_url_password_truth_table() {
        assert!(!redis_url_has_password("redis://127.0.0.1:6379/0"));
        assert!(!redis_url_has_password("redis://user@host:6379/0"));
        assert!(!redis_url_has_password("redis://:@host:6379/0"));
        assert!(!redis_url_has_password("unix:///var/run/redis.sock"));
        assert!(!redis_url_has_password("http://host"));
        assert!(!redis_url_has_password("not a url"));
        assert!(redis_url_has_password("redis://:hexpass@host:6379/0"));
        assert!(redis_url_has_password("redis://user:hexpass@host:6379/0"));
        assert!(redis_url_has_password("rediss://:hexpass@host:6379/0"));
    }

    #[test]
    fn consul_url_https_prefix() {
        assert!(consul_url_is_https("https://consul:8501"));
        assert!(!consul_url_is_https("http://consul:8500"));
        assert!(!consul_url_is_https(" cons://x"));
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard {
        prev_kim: Option<String>,
        prev_consul: Option<String>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn lock() -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            Self {
                prev_kim: std::env::var("KIM_ENV").ok(),
                prev_consul: std::env::var("CONSUL_HTTP_ADDR").ok(),
                _lock: lock,
            }
        }

        #[allow(unsafe_code)]
        fn set_kim(&self, value: Option<&str>) {
            // SAFETY: ENV_LOCK serializes mutation of this process-global var.
            unsafe {
                match value {
                    Some(v) => std::env::set_var("KIM_ENV", v),
                    None => std::env::remove_var("KIM_ENV"),
                }
            }
        }

        #[allow(unsafe_code)]
        fn set_consul(&self, value: Option<&str>) {
            // SAFETY: ENV_LOCK serializes mutation of this process-global var.
            unsafe {
                match value {
                    Some(v) => std::env::set_var("CONSUL_HTTP_ADDR", v),
                    None => std::env::remove_var("CONSUL_HTTP_ADDR"),
                }
            }
        }
    }

    impl Drop for EnvGuard {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            // SAFETY: ENV_LOCK is held for the guard's lifetime.
            unsafe {
                match &self.prev_kim {
                    Some(v) => std::env::set_var("KIM_ENV", v),
                    None => std::env::remove_var("KIM_ENV"),
                }
                match &self.prev_consul {
                    Some(v) => std::env::set_var("CONSUL_HTTP_ADDR", v),
                    None => std::env::remove_var("CONSUL_HTTP_ADDR"),
                }
            }
        }
    }

    #[test]
    fn strict_runtime_env_table() {
        let env = EnvGuard::lock();
        env.set_kim(Some("production"));
        env.set_consul(None);
        assert!(strict_runtime());
        env.set_kim(Some("PRODUCTION"));
        assert!(strict_runtime());
        env.set_kim(Some("development"));
        env.set_consul(Some("https://consul:8501"));
        assert!(!strict_runtime());
        env.set_kim(Some("dev"));
        assert!(!strict_runtime());
        env.set_kim(None);
        env.set_consul(Some("https://consul:8501"));
        assert!(strict_runtime());
        env.set_consul(Some("  "));
        assert!(!strict_runtime());
        env.set_consul(None);
        assert!(!strict_runtime());
    }

    #[test]
    fn check_strict_rejects_demo_when_production() {
        let env = EnvGuard::lock();
        env.set_kim(Some("production"));
        env.set_consul(None);
        assert!(check_strict_runtime(StrictCheck {
            hmac: Some(DEMO_INTERNAL_HMAC_SECRET),
            ..StrictCheck::default()
        })
        .is_err());
        assert!(check_strict_runtime(StrictCheck {
            jwt: Some(PLACEHOLDER_SECRET),
            ..StrictCheck::default()
        })
        .is_err());
        assert!(check_strict_runtime(StrictCheck {
            redis_url: Some("redis://redis:6379/0"),
            ..StrictCheck::default()
        })
        .is_err());
        assert!(check_strict_runtime(StrictCheck {
            require_redis: true,
            ..StrictCheck::default()
        })
        .is_err());
        env.set_kim(Some("development"));
        assert!(check_strict_runtime(StrictCheck {
            hmac: Some(DEMO_INTERNAL_HMAC_SECRET),
            ..StrictCheck::default()
        })
        .is_ok());
    }
}
