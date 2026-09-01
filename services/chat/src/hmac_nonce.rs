//! HMAC nonce occupancy. Verify stays a pure function; this claims the nonce.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
#[cfg(feature = "redis")]
use kim_protocol::hmac_nonce_key;
use kim_protocol::HMAC_NONCE_TTL_SECS;

#[async_trait]
pub trait HmacNonceGuard: Send + Sync {
    /// `Ok(true)` first claim; `Ok(false)` replay; `Err` backend failure.
    async fn claim(&self, nonce: &str) -> Result<bool, String>;
}

pub struct MemoryHmacNonceGuard {
    seen: Mutex<HashMap<String, Instant>>,
}

impl MemoryHmacNonceGuard {
    #[must_use]
    pub fn new() -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
        }
    }

    fn claim_at(&self, nonce: &str, now: Instant) -> Result<bool, String> {
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        seen.retain(|_, t| {
            now.checked_duration_since(*t)
                .map(|d| d.as_secs() < HMAC_NONCE_TTL_SECS)
                .unwrap_or(true)
        });
        if seen.contains_key(nonce) {
            return Ok(false);
        }
        seen.insert(nonce.to_string(), now);
        Ok(true)
    }
}

impl Default for MemoryHmacNonceGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HmacNonceGuard for MemoryHmacNonceGuard {
    async fn claim(&self, nonce: &str) -> Result<bool, String> {
        self.claim_at(nonce, Instant::now())
    }
}

#[cfg(feature = "redis")]
pub struct RedisHmacNonceGuard {
    conn: redis::aio::ConnectionManager,
}

#[cfg(feature = "redis")]
impl RedisHmacNonceGuard {
    pub async fn open(url: &str) -> Result<Self, String> {
        let client = redis::Client::open(url).map_err(|e| e.to_string())?;
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl HmacNonceGuard for RedisHmacNonceGuard {
    async fn claim(&self, nonce: &str) -> Result<bool, String> {
        let mut conn = self.conn.clone();
        let set: Option<String> = redis::cmd("SET")
            .arg(hmac_nonce_key(nonce))
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(HMAC_NONCE_TTL_SECS)
            .query_async(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(set.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn memory_second_claim_is_replay() {
        let g = MemoryHmacNonceGuard::new();
        assert!(g.claim("abc12345").await.unwrap());
        assert!(!g.claim("abc12345").await.unwrap());
        assert!(g.claim("othernonce").await.unwrap());
    }

    #[test]
    fn memory_holds_until_ttl() {
        let g = MemoryHmacNonceGuard::new();
        let start = Instant::now();
        assert!(g.claim_at("abc12345", start).unwrap());
        let almost_expired = start + Duration::from_secs(HMAC_NONCE_TTL_SECS - 1);
        assert!(!g.claim_at("abc12345", almost_expired).unwrap());
        let expired = start + Duration::from_secs(HMAC_NONCE_TTL_SECS);
        assert!(g.claim_at("abc12345", expired).unwrap());
    }
}
