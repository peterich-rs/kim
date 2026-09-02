//! JWT `jti` revocation and account `token_epoch`. Memory is process-local;
//! Redis is shared with the gateway.

use std::collections::HashMap;
use std::sync::{RwLock, RwLockWriteGuard};
use std::time::{Duration, Instant};

use async_trait::async_trait;
#[cfg(feature = "redis")]
use kim_protocol::{token_epoch_key, token_revoke_key};

#[derive(Debug, thiserror::Error)]
pub enum RevokeError {
    #[error("{0}")]
    Backend(String),
}

#[async_trait]
pub trait TokenRevocation: Send + Sync {
    async fn revoke(&self, jti: &str, ttl_secs: u64) -> Result<(), RevokeError>;
    async fn is_revoked(&self, jti: &str) -> Result<bool, RevokeError>;
    async fn get_epoch(&self, account: &str) -> Result<u32, RevokeError>;
    async fn set_epoch(&self, account: &str, ver: u32, ttl_secs: u64) -> Result<(), RevokeError>;
}

pub struct MemoryRevocation {
    inner: RwLock<HashMap<String, Instant>>,
    epochs: RwLock<HashMap<String, u32>>,
}

impl MemoryRevocation {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            epochs: RwLock::new(HashMap::new()),
        }
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashMap<String, Instant>> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for MemoryRevocation {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenRevocation for MemoryRevocation {
    async fn revoke(&self, jti: &str, ttl_secs: u64) -> Result<(), RevokeError> {
        let ttl = ttl_secs.max(1);
        self.write()
            .insert(jti.to_string(), Instant::now() + Duration::from_secs(ttl));
        Ok(())
    }

    async fn is_revoked(&self, jti: &str) -> Result<bool, RevokeError> {
        let now = Instant::now();
        let mut inner = self.write();
        inner.retain(|_, exp| *exp > now);
        Ok(inner.contains_key(jti))
    }

    async fn get_epoch(&self, account: &str) -> Result<u32, RevokeError> {
        Ok(self
            .epochs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(account)
            .copied()
            .unwrap_or(0))
    }

    async fn set_epoch(&self, account: &str, ver: u32, _ttl_secs: u64) -> Result<(), RevokeError> {
        self.epochs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(account.to_string(), ver);
        Ok(())
    }
}

#[cfg(feature = "redis")]
pub struct RedisRevocation {
    conn: redis::aio::ConnectionManager,
}

#[cfg(feature = "redis")]
impl RedisRevocation {
    pub async fn open(url: &str) -> Result<Self, RevokeError> {
        let client = redis::Client::open(url).map_err(|e| RevokeError::Backend(e.to_string()))?;
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| RevokeError::Backend(e.to_string()))?;
        Ok(Self { conn })
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl TokenRevocation for RedisRevocation {
    async fn revoke(&self, jti: &str, ttl_secs: u64) -> Result<(), RevokeError> {
        let mut conn = self.conn.clone();
        let ttl = i64::try_from(ttl_secs.max(1)).unwrap_or(i64::MAX);
        redis::cmd("SET")
            .arg(token_revoke_key(jti))
            .arg("1")
            .arg("EX")
            .arg(ttl)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| RevokeError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn is_revoked(&self, jti: &str) -> Result<bool, RevokeError> {
        let mut conn = self.conn.clone();
        let found: Option<String> = redis::cmd("GET")
            .arg(token_revoke_key(jti))
            .query_async(&mut conn)
            .await
            .map_err(|e| RevokeError::Backend(e.to_string()))?;
        Ok(found.is_some())
    }

    async fn get_epoch(&self, account: &str) -> Result<u32, RevokeError> {
        let mut conn = self.conn.clone();
        let found: Option<String> = redis::cmd("GET")
            .arg(token_epoch_key(account))
            .query_async(&mut conn)
            .await
            .map_err(|e| RevokeError::Backend(e.to_string()))?;
        Ok(found.and_then(|s| s.parse().ok()).unwrap_or(0))
    }

    async fn set_epoch(&self, account: &str, ver: u32, ttl_secs: u64) -> Result<(), RevokeError> {
        let mut conn = self.conn.clone();
        let ttl = i64::try_from(ttl_secs.max(1)).unwrap_or(i64::MAX);
        redis::cmd("SET")
            .arg(token_epoch_key(account))
            .arg(ver.to_string())
            .arg("EX")
            .arg(ttl)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| RevokeError::Backend(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_revoke_roundtrip() {
        let store = MemoryRevocation::new();
        assert!(!store.is_revoked("a").await.unwrap());
        store.revoke("a", 60).await.unwrap();
        assert!(store.is_revoked("a").await.unwrap());
    }

    #[tokio::test]
    async fn memory_epoch_defaults_zero() {
        let store = MemoryRevocation::new();
        assert_eq!(store.get_epoch("alice").await.unwrap(), 0);
        store.set_epoch("alice", 2, 60).await.unwrap();
        assert_eq!(store.get_epoch("alice").await.unwrap(), 2);
    }
}
