//! Account directory for Royal register / login and product profiles.

use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;

pub const NICKNAME_MAX_CHARS: usize = 32;
pub const BIO_MAX_CHARS: usize = 160;
pub const AVATAR_MAX_CHARS: usize = 512;
pub const SEARCH_LIMIT: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("conflict")]
    Conflict,
    #[error("not found")]
    NotFound,
    #[error("invalid profile")]
    InvalidProfile,
    #[error("{0}")]
    Backend(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserProfile {
    pub account: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
}

impl UserProfile {
    pub fn from_account(account: &str) -> Self {
        Self {
            account: account.to_string(),
            nickname: account.to_string(),
            avatar: String::new(),
            bio: String::new(),
        }
    }

    pub fn display_name(&self) -> &str {
        if self.nickname.is_empty() {
            &self.account
        } else {
            &self.nickname
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfilePatch {
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
}

#[async_trait]
pub trait UserDirectory: Send + Sync {
    async fn upsert(&self, app: &str, account: &str) -> Result<(), UserError>;
    async fn create(&self, app: &str, account: &str, password_hash: &str) -> Result<(), UserError>;
    async fn password_hash(&self, app: &str, account: &str) -> Result<Option<String>, UserError>;
    async fn exists(&self, app: &str, account: &str) -> Result<bool, UserError>;
    async fn profile(&self, app: &str, account: &str) -> Result<Option<UserProfile>, UserError>;
    async fn update_profile(
        &self,
        app: &str,
        account: &str,
        patch: &ProfilePatch,
    ) -> Result<UserProfile, UserError>;
    async fn profiles(&self, app: &str, accounts: &[String])
        -> Result<Vec<UserProfile>, UserError>;
    async fn search(
        &self,
        app: &str,
        query: &str,
        exclude: &[String],
        limit: usize,
    ) -> Result<Vec<UserProfile>, UserError>;
    async fn set_password(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<(), UserError>;
}

#[derive(Clone)]
struct UserRecord {
    password_hash: Option<String>,
    nickname: String,
    avatar: String,
    bio: String,
}

pub struct MemoryUserDirectory {
    inner: RwLock<HashMap<(String, String), UserRecord>>,
}

impl MemoryUserDirectory {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, HashMap<(String, String), UserRecord>> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashMap<(String, String), UserRecord>> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for MemoryUserDirectory {
    fn default() -> Self {
        Self::new()
    }
}

fn record_to_profile(account: &str, rec: &UserRecord) -> UserProfile {
    UserProfile {
        account: account.to_string(),
        nickname: if rec.nickname.is_empty() {
            account.to_string()
        } else {
            rec.nickname.clone()
        },
        avatar: rec.avatar.clone(),
        bio: rec.bio.clone(),
    }
}

pub fn validate_patch(patch: &ProfilePatch) -> Result<ProfilePatch, UserError> {
    let nickname = patch.nickname.trim();
    if nickname.is_empty() || nickname.chars().count() > NICKNAME_MAX_CHARS {
        return Err(UserError::InvalidProfile);
    }
    if patch.avatar.chars().count() > AVATAR_MAX_CHARS {
        return Err(UserError::InvalidProfile);
    }
    if patch.bio.chars().count() > BIO_MAX_CHARS {
        return Err(UserError::InvalidProfile);
    }
    Ok(ProfilePatch {
        nickname: nickname.to_string(),
        avatar: patch.avatar.trim().to_string(),
        bio: patch.bio.trim().to_string(),
    })
}

#[async_trait]
impl UserDirectory for MemoryUserDirectory {
    async fn upsert(&self, app: &str, account: &str) -> Result<(), UserError> {
        self.write()
            .entry((app.to_string(), account.to_string()))
            .or_insert_with(|| UserRecord {
                password_hash: None,
                nickname: account.to_string(),
                avatar: String::new(),
                bio: String::new(),
            });
        Ok(())
    }

    async fn create(&self, app: &str, account: &str, password_hash: &str) -> Result<(), UserError> {
        let mut inner = self.write();
        let key = (app.to_string(), account.to_string());
        if inner.contains_key(&key) {
            return Err(UserError::Conflict);
        }
        inner.insert(
            key,
            UserRecord {
                password_hash: Some(password_hash.to_string()),
                nickname: account.to_string(),
                avatar: String::new(),
                bio: String::new(),
            },
        );
        Ok(())
    }

    async fn password_hash(&self, app: &str, account: &str) -> Result<Option<String>, UserError> {
        Ok(self
            .read()
            .get(&(app.to_string(), account.to_string()))
            .and_then(|r| r.password_hash.clone()))
    }

    async fn exists(&self, app: &str, account: &str) -> Result<bool, UserError> {
        Ok(self
            .read()
            .contains_key(&(app.to_string(), account.to_string())))
    }

    async fn profile(&self, app: &str, account: &str) -> Result<Option<UserProfile>, UserError> {
        Ok(self
            .read()
            .get(&(app.to_string(), account.to_string()))
            .map(|r| record_to_profile(account, r)))
    }

    async fn update_profile(
        &self,
        app: &str,
        account: &str,
        patch: &ProfilePatch,
    ) -> Result<UserProfile, UserError> {
        let patch = validate_patch(patch)?;
        let mut inner = self.write();
        let rec = inner
            .get_mut(&(app.to_string(), account.to_string()))
            .ok_or(UserError::NotFound)?;
        rec.nickname = patch.nickname;
        rec.avatar = patch.avatar;
        rec.bio = patch.bio;
        Ok(record_to_profile(account, rec))
    }

    async fn profiles(
        &self,
        app: &str,
        accounts: &[String],
    ) -> Result<Vec<UserProfile>, UserError> {
        let inner = self.read();
        Ok(accounts
            .iter()
            .filter_map(|acc| {
                inner
                    .get(&(app.to_string(), acc.clone()))
                    .map(|r| record_to_profile(acc, r))
            })
            .collect())
    }

    async fn search(
        &self,
        app: &str,
        query: &str,
        exclude: &[String],
        limit: usize,
    ) -> Result<Vec<UserProfile>, UserError> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let q_lower = q.to_ascii_lowercase();
        let cap = if limit == 0 { SEARCH_LIMIT } else { limit };
        let inner = self.read();
        let mut out = Vec::new();
        for ((row_app, account), rec) in inner.iter() {
            if row_app != app || exclude.iter().any(|e| e == account) {
                continue;
            }
            let nick = if rec.nickname.is_empty() {
                account.as_str()
            } else {
                rec.nickname.as_str()
            };
            let acc_hit = account.eq_ignore_ascii_case(q);
            let nick_hit = nick.to_ascii_lowercase().starts_with(&q_lower);
            if acc_hit || nick_hit {
                out.push(record_to_profile(account, rec));
            }
            if out.len() >= cap {
                break;
            }
        }
        out.sort_by(|a, b| a.account.cmp(&b.account));
        Ok(out)
    }

    async fn set_password(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<(), UserError> {
        let mut inner = self.write();
        let rec = inner
            .get_mut(&(app.to_string(), account.to_string()))
            .ok_or(UserError::NotFound)?;
        rec.password_hash = Some(password_hash.to_string());
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
fn pg_err(e: sqlx::Error) -> UserError {
    UserError::Backend(e.to_string())
}

#[cfg(feature = "postgres")]
fn row_profile(account: String, nickname: String, avatar: String, bio: String) -> UserProfile {
    UserProfile {
        nickname: if nickname.is_empty() {
            account.clone()
        } else {
            nickname
        },
        account,
        avatar,
        bio,
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl UserDirectory for PostgresUserDirectory {
    async fn upsert(&self, app: &str, account: &str) -> Result<(), UserError> {
        sqlx::query(
            "INSERT INTO users (app, account, nickname) VALUES ($1, $2, $2)
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

    async fn profile(&self, app: &str, account: &str) -> Result<Option<UserProfile>, UserError> {
        let row: Option<(String, String, String, String)> = sqlx::query_as(
            "SELECT account, nickname, avatar, bio FROM users
             WHERE app = $1 AND account = $2",
        )
        .bind(app)
        .bind(account)
        .fetch_optional(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(row.map(|(account, nickname, avatar, bio)| row_profile(account, nickname, avatar, bio)))
    }

    async fn update_profile(
        &self,
        app: &str,
        account: &str,
        patch: &ProfilePatch,
    ) -> Result<UserProfile, UserError> {
        let patch = validate_patch(patch)?;
        let row: Option<(String, String, String, String)> = sqlx::query_as(
            "UPDATE users SET nickname = $3, avatar = $4, bio = $5
             WHERE app = $1 AND account = $2
             RETURNING account, nickname, avatar, bio",
        )
        .bind(app)
        .bind(account)
        .bind(&patch.nickname)
        .bind(&patch.avatar)
        .bind(&patch.bio)
        .fetch_optional(&self.pool)
        .await
        .map_err(pg_err)?;
        row.map(|(account, nickname, avatar, bio)| row_profile(account, nickname, avatar, bio))
            .ok_or(UserError::NotFound)
    }

    async fn profiles(
        &self,
        app: &str,
        accounts: &[String],
    ) -> Result<Vec<UserProfile>, UserError> {
        if accounts.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT account, nickname, avatar, bio FROM users
             WHERE app = $1 AND account = ANY($2)",
        )
        .bind(app)
        .bind(accounts)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        let mut by_acc = HashMap::with_capacity(rows.len());
        for (account, nickname, avatar, bio) in rows {
            by_acc.insert(account.clone(), row_profile(account, nickname, avatar, bio));
        }
        Ok(accounts.iter().filter_map(|a| by_acc.remove(a)).collect())
    }

    async fn search(
        &self,
        app: &str,
        query: &str,
        exclude: &[String],
        limit: usize,
    ) -> Result<Vec<UserProfile>, UserError> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let cap = i64::try_from(if limit == 0 { SEARCH_LIMIT } else { limit }).unwrap_or(20);
        let prefix = format!("{}%", q.to_lowercase());
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT account, nickname, avatar, bio FROM users
             WHERE app = $1
               AND NOT (account = ANY($4))
               AND (lower(account) = lower($2) OR lower(nickname) LIKE $3)
             ORDER BY account ASC
             LIMIT $5",
        )
        .bind(app)
        .bind(q)
        .bind(&prefix)
        .bind(exclude)
        .bind(cap)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(rows
            .into_iter()
            .map(|(account, nickname, avatar, bio)| row_profile(account, nickname, avatar, bio))
            .collect())
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
        let p = dir.profile("kim", "alice").await.unwrap().unwrap();
        assert_eq!(p.nickname, "alice");
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

    #[tokio::test]
    async fn memory_update_and_search() {
        let dir = MemoryUserDirectory::new();
        dir.upsert("kim", "alice").await.unwrap();
        dir.upsert("kim", "albert").await.unwrap();
        dir.update_profile(
            "kim",
            "alice",
            &ProfilePatch {
                nickname: "Ali".into(),
                avatar: "http://a".into(),
                bio: "hi".into(),
            },
        )
        .await
        .unwrap();
        let hits = dir.search("kim", "al", &[], 10).await.unwrap();
        assert!(hits.iter().any(|p| p.account == "alice"));
        let none = dir
            .search("kim", "al", &["alice".into(), "albert".into()], 10)
            .await
            .unwrap();
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn memory_rejects_empty_nickname() {
        let dir = MemoryUserDirectory::new();
        dir.upsert("kim", "alice").await.unwrap();
        let err = dir
            .update_profile(
                "kim",
                "alice",
                &ProfilePatch {
                    nickname: "  ".into(),
                    avatar: String::new(),
                    bio: String::new(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, UserError::InvalidProfile));
    }
}
