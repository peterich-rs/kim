//! Royal-owned account secrets. The shared user directory does not expose these writes.

use std::sync::Arc;

use async_trait::async_trait;
use kim_server_data::users::{MemoryUserDirectory, UserError};

#[async_trait]
pub trait AccountAuthStore: Send + Sync {
    async fn create(&self, app: &str, account: &str, password_hash: &str) -> Result<(), UserError>;
    async fn password_hash(&self, app: &str, account: &str) -> Result<Option<String>, UserError>;
    async fn set_password(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<(), UserError>;
    async fn token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError>;
    async fn bump_token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError>;
    async fn set_password_and_bump_epoch(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<u32, UserError>;
}

/// Same in-memory account map as the public directory. Registration stays visible to profile lookup.
pub struct MemoryAccountAuth {
    users: Arc<MemoryUserDirectory>,
}

impl MemoryAccountAuth {
    pub fn new(users: Arc<MemoryUserDirectory>) -> Self {
        Self { users }
    }
}

#[async_trait]
impl AccountAuthStore for MemoryAccountAuth {
    async fn create(&self, app: &str, account: &str, password_hash: &str) -> Result<(), UserError> {
        if account.starts_with("b_") {
            return Err(UserError::InvalidProfile);
        }
        self.users.create(app, account, password_hash).await
    }

    async fn password_hash(&self, app: &str, account: &str) -> Result<Option<String>, UserError> {
        self.users.password_hash(app, account).await
    }

    async fn set_password(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<(), UserError> {
        self.users.set_password(app, account, password_hash).await
    }

    async fn token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError> {
        self.users.token_epoch(app, account).await
    }

    async fn bump_token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError> {
        self.users.bump_token_epoch(app, account).await
    }

    async fn set_password_and_bump_epoch(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<u32, UserError> {
        self.users
            .set_password_and_bump_epoch(app, account, password_hash)
            .await
    }
}

#[cfg(feature = "postgres")]
pub struct PostgresAccountAuth {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresAccountAuth {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    pub async fn connect(url: &str) -> Result<Self, UserError> {
        let pool =
            kim_server_data::store::connect_pool(url, kim_server_data::store::PoolOpts::default())
                .await
                .map_err(|err| UserError::Backend(err.to_string()))?;
        Ok(Self { pool })
    }
}

#[cfg(feature = "postgres")]
fn pg_err(err: sqlx::Error) -> UserError {
    UserError::Backend(err.to_string())
}

#[cfg(feature = "postgres")]
#[async_trait]
impl AccountAuthStore for PostgresAccountAuth {
    async fn create(&self, app: &str, account: &str, password_hash: &str) -> Result<(), UserError> {
        if account.starts_with("b_") {
            return Err(UserError::InvalidProfile);
        }
        let res = sqlx::query(
            "INSERT INTO users (app, account, password_hash, nickname)
             VALUES ($1, $2, $3, $2)
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

    async fn set_password(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<(), UserError> {
        let res =
            sqlx::query("UPDATE users SET password_hash = $3 WHERE app = $1 AND account = $2")
                .bind(app)
                .bind(account)
                .bind(password_hash)
                .execute(&self.pool)
                .await
                .map_err(pg_err)?;
        if res.rows_affected() == 0 {
            return Err(UserError::NotFound);
        }
        Ok(())
    }

    async fn token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError> {
        let row: Option<i32> =
            sqlx::query_scalar("SELECT token_epoch FROM users WHERE app = $1 AND account = $2")
                .bind(app)
                .bind(account)
                .fetch_optional(&self.pool)
                .await
                .map_err(pg_err)?;
        Ok(row.and_then(|n| u32::try_from(n).ok()).unwrap_or(0))
    }

    async fn bump_token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError> {
        let row: Option<i32> = sqlx::query_scalar(
            "UPDATE users SET token_epoch = token_epoch + 1
             WHERE app = $1 AND account = $2
             RETURNING token_epoch",
        )
        .bind(app)
        .bind(account)
        .fetch_optional(&self.pool)
        .await
        .map_err(pg_err)?;
        let n = row.ok_or(UserError::NotFound)?;
        u32::try_from(n).map_err(|err| UserError::Backend(err.to_string()))
    }

    async fn set_password_and_bump_epoch(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<u32, UserError> {
        let row: Option<i32> = sqlx::query_scalar(
            "UPDATE users SET password_hash = $3, token_epoch = token_epoch + 1
             WHERE app = $1 AND account = $2
             RETURNING token_epoch",
        )
        .bind(app)
        .bind(account)
        .bind(password_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(pg_err)?;
        let n = row.ok_or(UserError::NotFound)?;
        u32::try_from(n).map_err(|err| UserError::Backend(err.to_string()))
    }
}
