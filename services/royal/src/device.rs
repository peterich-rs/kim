//! Device credential storage. Secret is shown once; JWT carries `did` only.

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("{0}")]
    Backend(String),
}

#[derive(Clone, Debug)]
pub struct DeviceRecord {
    pub device_id: String,
    pub account: String,
    pub secret_hash: String,
    pub revoked: bool,
}

#[async_trait]
pub trait DeviceDirectory: Send + Sync {
    async fn enroll(
        &self,
        app: &str,
        account: &str,
        device_id: &str,
        secret_hash: &str,
    ) -> Result<(), DeviceError>;
    async fn lookup_hash(
        &self,
        app: &str,
        account: &str,
        secret_hash: &str,
    ) -> Result<Option<DeviceRecord>, DeviceError>;
    async fn get(&self, device_id: &str) -> Result<Option<DeviceRecord>, DeviceError>;
    async fn revoke(&self, device_id: &str) -> Result<(), DeviceError>;
}

pub struct MemoryDeviceDirectory {
    inner: RwLock<HashMap<String, (String, String, String, bool)>>,
}

impl MemoryDeviceDirectory {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryDeviceDirectory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DeviceDirectory for MemoryDeviceDirectory {
    async fn enroll(
        &self,
        app: &str,
        account: &str,
        device_id: &str,
        secret_hash: &str,
    ) -> Result<(), DeviceError> {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                device_id.to_string(),
                (
                    app.to_string(),
                    account.to_string(),
                    secret_hash.to_string(),
                    false,
                ),
            );
        Ok(())
    }

    async fn lookup_hash(
        &self,
        app: &str,
        account: &str,
        secret_hash: &str,
    ) -> Result<Option<DeviceRecord>, DeviceError> {
        Ok(self
            .inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|(_, (a, acc, hash, revoked))| {
                a == app && acc == account && hash == secret_hash && !*revoked
            })
            .map(|(id, (_, acc, hash, _))| DeviceRecord {
                device_id: id.clone(),
                account: acc.clone(),
                secret_hash: hash.clone(),
                revoked: false,
            }))
    }

    async fn get(&self, device_id: &str) -> Result<Option<DeviceRecord>, DeviceError> {
        Ok(self
            .inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(device_id)
            .map(|(app, account, hash, revoked)| {
                let _ = app;
                DeviceRecord {
                    device_id: device_id.to_string(),
                    account: account.clone(),
                    secret_hash: hash.clone(),
                    revoked: *revoked,
                }
            }))
    }

    async fn revoke(&self, device_id: &str) -> Result<(), DeviceError> {
        if let Some(row) = self
            .inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(device_id)
        {
            row.3 = true;
        }
        Ok(())
    }
}

#[cfg(feature = "postgres")]
pub struct PostgresDeviceDirectory {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresDeviceDirectory {
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl DeviceDirectory for PostgresDeviceDirectory {
    async fn enroll(
        &self,
        app: &str,
        account: &str,
        device_id: &str,
        secret_hash: &str,
    ) -> Result<(), DeviceError> {
        sqlx::query(
            "INSERT INTO device_credentials (device_id, app, account, secret_hash)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(device_id)
        .bind(app)
        .bind(account)
        .bind(secret_hash)
        .execute(&self.pool)
        .await
        .map_err(|e| DeviceError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn lookup_hash(
        &self,
        app: &str,
        account: &str,
        secret_hash: &str,
    ) -> Result<Option<DeviceRecord>, DeviceError> {
        let row: Option<(String, String, String)> = sqlx::query_as(
            "SELECT device_id, account, secret_hash FROM device_credentials
             WHERE app = $1 AND account = $2 AND secret_hash = $3 AND revoked_at IS NULL",
        )
        .bind(app)
        .bind(account)
        .bind(secret_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DeviceError::Backend(e.to_string()))?;
        Ok(row.map(|(device_id, account, secret_hash)| DeviceRecord {
            device_id,
            account,
            secret_hash,
            revoked: false,
        }))
    }

    async fn get(&self, device_id: &str) -> Result<Option<DeviceRecord>, DeviceError> {
        let row: Option<(String, String, String, i32)> = sqlx::query_as(
            "SELECT device_id, account, secret_hash,
                    CASE WHEN revoked_at IS NULL THEN 0 ELSE 1 END
             FROM device_credentials WHERE device_id = $1",
        )
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DeviceError::Backend(e.to_string()))?;
        Ok(
            row.map(|(device_id, account, secret_hash, revoked)| DeviceRecord {
                device_id,
                account,
                secret_hash,
                revoked: revoked != 0,
            }),
        )
    }

    async fn revoke(&self, device_id: &str) -> Result<(), DeviceError> {
        sqlx::query(
            "UPDATE device_credentials SET revoked_at = now()
             WHERE device_id = $1 AND revoked_at IS NULL",
        )
        .bind(device_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DeviceError::Backend(e.to_string()))?;
        Ok(())
    }
}

pub fn hash_secret(secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    digest.iter().fold(String::with_capacity(64), |mut out, b| {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
        out
    })
}

pub fn new_secret() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn new_device_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[async_trait]
pub trait DeviceHot: Send + Sync {
    async fn put(&self, device_id: &str, account: &str) -> Result<(), DeviceError>;
    async fn drop_key(&self, device_id: &str) -> Result<(), DeviceError>;
    async fn ok(&self, device_id: &str, account: &str) -> Result<bool, DeviceError>;
}

pub struct MemoryDeviceHot {
    inner: RwLock<HashMap<String, String>>,
}

impl MemoryDeviceHot {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryDeviceHot {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DeviceHot for MemoryDeviceHot {
    async fn put(&self, device_id: &str, account: &str) -> Result<(), DeviceError> {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(device_id.to_string(), account.to_string());
        Ok(())
    }

    async fn drop_key(&self, device_id: &str) -> Result<(), DeviceError> {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(device_id);
        Ok(())
    }

    async fn ok(&self, device_id: &str, account: &str) -> Result<bool, DeviceError> {
        Ok(self
            .inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(device_id)
            .is_some_and(|a| a == account))
    }
}

#[cfg(feature = "redis")]
pub struct RedisDeviceHot {
    conn: redis::aio::ConnectionManager,
}

#[cfg(feature = "redis")]
impl RedisDeviceHot {
    pub async fn open(url: &str) -> Result<Self, DeviceError> {
        let client = redis::Client::open(url).map_err(|e| DeviceError::Backend(e.to_string()))?;
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| DeviceError::Backend(e.to_string()))?;
        Ok(Self { conn })
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl DeviceHot for RedisDeviceHot {
    async fn put(&self, device_id: &str, account: &str) -> Result<(), DeviceError> {
        let mut conn = self.conn.clone();
        redis::cmd("SET")
            .arg(kim_protocol::device_hot_key(device_id))
            .arg(account)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| DeviceError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn drop_key(&self, device_id: &str) -> Result<(), DeviceError> {
        let mut conn = self.conn.clone();
        redis::cmd("DEL")
            .arg(kim_protocol::device_hot_key(device_id))
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| DeviceError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn ok(&self, device_id: &str, account: &str) -> Result<bool, DeviceError> {
        let mut conn = self.conn.clone();
        let found: Option<String> = redis::cmd("GET")
            .arg(kim_protocol::device_hot_key(device_id))
            .query_async(&mut conn)
            .await
            .map_err(|e| DeviceError::Backend(e.to_string()))?;
        Ok(found.is_some_and(|a| a == account))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enroll_and_lookup() {
        let dir = MemoryDeviceDirectory::new();
        let hash = hash_secret("secret");
        dir.enroll("kim", "alice", "d1", &hash).await.unwrap();
        let got = dir.lookup_hash("kim", "alice", &hash).await.unwrap();
        assert_eq!(got.unwrap().device_id, "d1");
        dir.revoke("d1").await.unwrap();
        assert!(dir
            .lookup_hash("kim", "alice", &hash)
            .await
            .unwrap()
            .is_none());
    }
}
