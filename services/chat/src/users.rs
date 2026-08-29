//! Account directory for Royal register / login.

use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("conflict")]
    Conflict,
    #[error("{0}")]
    Backend(String),
}

#[async_trait]
pub trait UserDirectory: Send + Sync {
    async fn upsert(&self, app: &str, account: &str) -> Result<(), UserError>;
    async fn create(&self, app: &str, account: &str, password_hash: &str) -> Result<(), UserError>;
    async fn password_hash(&self, app: &str, account: &str) -> Result<Option<String>, UserError>;
    async fn exists(&self, app: &str, account: &str) -> Result<bool, UserError>;
}

pub struct MemoryUserDirectory {
    inner: RwLock<HashMap<(String, String), Option<String>>>,
}

impl MemoryUserDirectory {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, HashMap<(String, String), Option<String>>> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashMap<(String, String), Option<String>>> {
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
    async fn upsert(&self, app: &str, account: &str) -> Result<(), UserError> {
        self.write()
            .entry((app.to_string(), account.to_string()))
            .or_insert(None);
        Ok(())
    }

    async fn create(&self, app: &str, account: &str, password_hash: &str) -> Result<(), UserError> {
        let mut inner = self.write();
        let key = (app.to_string(), account.to_string());
        if inner.contains_key(&key) {
            return Err(UserError::Conflict);
        }
        inner.insert(key, Some(password_hash.to_string()));
        Ok(())
    }

    async fn password_hash(&self, app: &str, account: &str) -> Result<Option<String>, UserError> {
        Ok(self
            .read()
            .get(&(app.to_string(), account.to_string()))
            .cloned()
            .flatten())
    }

    async fn exists(&self, app: &str, account: &str) -> Result<bool, UserError> {
        Ok(self
            .read()
            .contains_key(&(app.to_string(), account.to_string())))
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
fn pg_err(e: sqlx::Error) -> UserError {
    UserError::Backend(e.to_string())
}

#[cfg(feature = "postgres")]
#[async_trait]
impl UserDirectory for PostgresUserDirectory {
    async fn upsert(&self, app: &str, account: &str) -> Result<(), UserError> {
        sqlx::query(
            "INSERT INTO users (app, account) VALUES ($1, $2)
             ON CONFLICT (app, account) DO NOTHING",
        )
        .bind(app)
        .bind(account)
        .execute(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(())
    }

    async fn create(&self, app: &str, account: &str, password_hash: &str) -> Result<(), UserError> {
        let res = sqlx::query(
            "INSERT INTO users (app, account, password_hash) VALUES ($1, $2, $3)
             ON CONFLICT (app, account) DO NOTHING",
        )
        .bind(app)
        .bind(account)
        .bind(password_hash)
        .execute(&self.pool)
        .await
        .map_err(pg_err)?;
        if res.rows_affected() == 0 {
            return Err(UserError::Conflict);
        }
        Ok(())
    }

    async fn password_hash(&self, app: &str, account: &str) -> Result<Option<String>, UserError> {
        let row: Option<Option<String>> =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE app = $1 AND account = $2")
                .bind(app)
                .bind(account)
                .fetch_optional(&self.pool)
                .await
                .map_err(pg_err)?;
        Ok(row.flatten())
    }

    async fn exists(&self, app: &str, account: &str) -> Result<bool, UserError> {
        let found: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM users WHERE app = $1 AND account = $2")
                .bind(app)
                .bind(account)
                .fetch_optional(&self.pool)
                .await
                .map_err(pg_err)?;
        Ok(found.is_some())
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
        assert_eq!(dir.password_hash("kim", "alice").await.unwrap(), None);
    }

    #[tokio::test]
    async fn memory_create_conflict_and_hash() {
        let dir = MemoryUserDirectory::new();
        dir.create("kim", "alice", "hash-1").await.unwrap();
        assert!(matches!(
            dir.create("kim", "alice", "hash-2").await,
            Err(UserError::Conflict)
        ));
        assert_eq!(
            dir.password_hash("kim", "alice").await.unwrap().as_deref(),
            Some("hash-1")
        );
        assert_eq!(dir.password_hash("kim", "bob").await.unwrap(), None);
    }

    #[tokio::test]
    async fn memory_upsert_then_create_conflicts() {
        let dir = MemoryUserDirectory::new();
        dir.upsert("kim", "alice").await.unwrap();
        assert!(matches!(
            dir.create("kim", "alice", "hash").await,
            Err(UserError::Conflict)
        ));
    }

    #[tokio::test]
    async fn memory_exists_after_upsert_or_create() {
        let dir = MemoryUserDirectory::new();
        assert!(!dir.exists("kim", "alice").await.unwrap());
        dir.upsert("kim", "alice").await.unwrap();
        assert!(dir.exists("kim", "alice").await.unwrap());
        assert!(!dir.exists("kim", "bob").await.unwrap());
        dir.create("kim", "bob", "h").await.unwrap();
        assert!(dir.exists("kim", "bob").await.unwrap());
    }
}
