//! Production startup checks. The wire protocol does not know about Consul,
//! Redis passwords, or demo secrets; the processes that bind them do.

use kim_protocol::{is_demo_internal_hmac, DEMO_DEFAULT_SECRET, DEMO_INTERNAL_HMAC_SECRET};

const PLACEHOLDER_SECRET: &str = "change-me";

fn is_placeholder(secret: &str) -> bool {
    secret == PLACEHOLDER_SECRET
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

/// Inputs for [`check_strict_runtime`]. Unset fields are skipped.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrictCheck<'a> {
    pub hmac: Option<&'a str>,
    pub jwt: Option<&'a str>,
    pub redis_url: Option<&'a str>,
    pub require_redis: bool,
    pub consul_addr: Option<&'a str>,
    /// `Some(false)` when `KIM_AUTH_PASSWORD_SEAL_PRIVATE` is unset.
    pub password_seal_configured: Option<bool>,
    /// `Some(true)` when plaintext passwords are accepted (`KIM_AUTH_ALLOW_PLAINTEXT_PASSWORD`).
    pub allow_plaintext_password: Option<bool>,
}

/// Reject demo/`change-me` secrets, passwordless Redis, plaintext Consul,
/// and an unsealed password login path.
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
        if j == kim_protocol::DEMO_DEFAULT_SECRET || j == "change-me" {
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
    if check.password_seal_configured == Some(false) {
        return Err("production requires KIM_AUTH_PASSWORD_SEAL_PRIVATE".into());
    }
    if check.allow_plaintext_password == Some(true) {
        return Err("production must not set KIM_AUTH_ALLOW_PLAINTEXT_PASSWORD".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn check_strict_rejects_unsealed_password_when_production() {
        let env = EnvGuard::lock();
        env.set_kim(Some("production"));
        env.set_consul(None);
        assert!(check_strict_runtime(StrictCheck {
            password_seal_configured: Some(false),
            ..StrictCheck::default()
        })
        .is_err());
        assert!(check_strict_runtime(StrictCheck {
            password_seal_configured: Some(true),
            allow_plaintext_password: Some(true),
            ..StrictCheck::default()
        })
        .is_err());
        assert!(check_strict_runtime(StrictCheck {
            password_seal_configured: Some(true),
            allow_plaintext_password: Some(false),
            ..StrictCheck::default()
        })
        .is_ok());
        env.set_kim(Some("development"));
        assert!(check_strict_runtime(StrictCheck {
            password_seal_configured: Some(false),
            allow_plaintext_password: Some(true),
            ..StrictCheck::default()
        })
        .is_ok());
    }
}
