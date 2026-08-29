//! Demo-grade user upsert for Royal token issuance.

use std::collections::HashSet;
use std::sync::{RwLock, RwLockWriteGuard};

use async_trait::async_trait;

use crate::store::StoreError;

#[async_trait]
pub trait UserDirectory: Send + Sync {
    async fn upsert(&self, app: &str, account: &str) -> Result<(), StoreError>;
}

pub struct MemoryUserDirectory {
    inner: RwLock<HashSet<(String, String)>>,
}

impl MemoryUserDirectory {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashSet::new()),
        }
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashSet<(String, String)>> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for MemoryUserDirectory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UserDirectory for MemoryUserDirectory {
    async fn upsert(&self, app: &str, account: &str) -> Result<(), StoreError> {
        self.write().insert((app.to_string(), account.to_string()));
        Ok(())
    }
}

#[cfg(feature = "postgres")]
pub struct PostgresUserDirectory {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresUserDirectory {
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl UserDirectory for PostgresUserDirectory {
    async fn upsert(&self, app: &str, account: &str) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO users (app, account) VALUES ($1, $2)
             ON CONFLICT (app, account) DO NOTHING",
        )
        .bind(app)
        .bind(account)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_upsert_is_idempotent() {
        let dir = MemoryUserDirectory::new();
        dir.upsert("kim", "alice").await.unwrap();
        dir.upsert("kim", "alice").await.unwrap();
        assert_eq!(dir.write().len(), 1);
    }
}
