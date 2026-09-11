//! Account directory for Royal register / login and product profiles.

use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use kim_protocol::{PROFILE_KIND_BOT, PROFILE_KIND_USER};

use crate::directory::base36_upper;
use crate::idgen::IdGenerator;
use crate::social::SocialDirectory;

pub const NICKNAME_MAX_CHARS: usize = 32;
pub const BIO_MAX_CHARS: usize = 160;
pub const AVATAR_MAX_CHARS: usize = 512;
pub const SEARCH_LIMIT: usize = 20;
pub const BOT_MAX_PER_OWNER: u32 = 20;

#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("conflict")]
    Conflict,
    #[error("not found")]
    NotFound,
    #[error("invalid profile")]
    InvalidProfile,
    #[error("limit")]
    Limit,
    #[error("not bot owner")]
    NotBotOwner,
    #[error("{0}")]
    Backend(String),
}

impl From<crate::idgen::IdError> for UserError {
    fn from(e: crate::idgen::IdError) -> Self {
        UserError::Backend(e.to_string())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreateBot {
    pub owner: String,
    pub client_profile_id: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserPresence {
    pub exists: bool,
    pub kind: i32,
    pub owner_account: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserProfile {
    pub account: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
    /// `PROFILE_KIND_USER` (1) or `PROFILE_KIND_BOT` (2).
    pub kind: i32,
}

impl UserProfile {
    pub fn from_account(account: &str) -> Self {
        Self {
            account: account.to_string(),
            nickname: account.to_string(),
            avatar: String::new(),
            bio: String::new(),
            kind: PROFILE_KIND_USER,
        }
    }

    pub fn is_bot(&self) -> bool {
        self.kind == PROFILE_KIND_BOT
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
    async fn token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError>;
    async fn bump_token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError>;
    async fn set_password_and_bump_epoch(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<u32, UserError>;
    async fn create_bot(&self, app: &str, req: &CreateBot) -> Result<UserProfile, UserError>;
    async fn bot_owner(&self, app: &str, account: &str) -> Result<Option<String>, UserError>;
    async fn delete_bot(&self, app: &str, owner: &str, account: &str) -> Result<(), UserError>;
    async fn count_bots(&self, app: &str, owner: &str) -> Result<u32, UserError>;
    async fn lookup(&self, app: &str, account: &str) -> Result<Option<UserPresence>, UserError>;
}

#[derive(Clone)]
struct UserRecord {
    password_hash: Option<String>,
    nickname: String,
    avatar: String,
    bio: String,
    token_epoch: u32,
    kind: i32,
    owner_account: String,
    client_profile_id: String,
}

fn human_record(account: &str, password_hash: Option<String>) -> UserRecord {
    UserRecord {
        password_hash,
        nickname: account.to_string(),
        avatar: String::new(),
        bio: String::new(),
        token_epoch: 0,
        kind: PROFILE_KIND_USER,
        owner_account: String::new(),
        client_profile_id: String::new(),
    }
}

pub fn valid_client_profile_id(raw: &str) -> bool {
    let n = raw.len();
    (1..=64).contains(&n)
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn presence_from_record(rec: &UserRecord) -> UserPresence {
    UserPresence {
        exists: true,
        kind: rec.kind,
        owner_account: rec.owner_account.clone(),
    }
}

pub fn kind_from_db(raw: &str) -> i32 {
    if raw == "bot" {
        PROFILE_KIND_BOT
    } else {
        PROFILE_KIND_USER
    }
}

pub struct MemoryUserDirectory {
    inner: RwLock<HashMap<(String, String), UserRecord>>,
    social: Option<Arc<dyn SocialDirectory>>,
    idgen: Option<Arc<dyn IdGenerator>>,
}

impl MemoryUserDirectory {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            social: None,
            idgen: None,
        }
    }

    #[must_use]
    pub fn with_social(mut self, social: Arc<dyn SocialDirectory>) -> Self {
        self.social = Some(social);
        self
    }

    #[must_use]
    pub fn with_idgen(mut self, idgen: Arc<dyn IdGenerator>) -> Self {
        self.idgen = Some(idgen);
        self
    }

    fn read(&self) -> RwLockReadGuard<'_, HashMap<(String, String), UserRecord>> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashMap<(String, String), UserRecord>> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }

    fn find_bot_by_profile(
        inner: &HashMap<(String, String), UserRecord>,
        app: &str,
        owner: &str,
        client_profile_id: &str,
    ) -> Option<(String, UserRecord)> {
        inner.iter().find_map(|((row_app, account), rec)| {
            if row_app == app
                && rec.kind == PROFILE_KIND_BOT
                && rec.owner_account == owner
                && rec.client_profile_id == client_profile_id
            {
                Some((account.clone(), rec.clone()))
            } else {
                None
            }
        })
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
        kind: rec.kind,
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
            .or_insert_with(|| human_record(account, None));
        Ok(())
    }

    async fn create(&self, app: &str, account: &str, password_hash: &str) -> Result<(), UserError> {
        let mut inner = self.write();
        let key = (app.to_string(), account.to_string());
        if inner.contains_key(&key) {
            return Err(UserError::Conflict);
        }
        inner.insert(key, human_record(account, Some(password_hash.to_string())));
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
            if row_app != app
                || rec.kind == PROFILE_KIND_BOT
                || exclude.iter().any(|e| e == account)
            {
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

    async fn token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError> {
        Ok(self
            .read()
            .get(&(app.to_string(), account.to_string()))
            .map(|r| r.token_epoch)
            .unwrap_or(0))
    }

    async fn bump_token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError> {
        let mut inner = self.write();
        let rec = inner
            .get_mut(&(app.to_string(), account.to_string()))
            .ok_or(UserError::NotFound)?;
        rec.token_epoch = rec.token_epoch.saturating_add(1);
        Ok(rec.token_epoch)
    }

    async fn set_password_and_bump_epoch(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<u32, UserError> {
        let mut inner = self.write();
        let rec = inner
            .get_mut(&(app.to_string(), account.to_string()))
            .ok_or(UserError::NotFound)?;
        rec.password_hash = Some(password_hash.to_string());
        rec.token_epoch = rec.token_epoch.saturating_add(1);
        Ok(rec.token_epoch)
    }

    async fn create_bot(&self, app: &str, req: &CreateBot) -> Result<UserProfile, UserError> {
        let social = self
            .social
            .as_ref()
            .ok_or_else(|| UserError::Backend("create_bot requires social".into()))?;
        let idgen = self
            .idgen
            .as_ref()
            .ok_or_else(|| UserError::Backend("create_bot requires idgen".into()))?;
        if !valid_client_profile_id(&req.client_profile_id) {
            return Err(UserError::InvalidProfile);
        }
        if req.owner.is_empty() {
            return Err(UserError::InvalidProfile);
        }
        let existing = {
            let inner = self.read();
            Self::find_bot_by_profile(&inner, app, &req.owner, &req.client_profile_id)
        };
        if let Some((account, rec)) = existing {
            social
                .ensure_friends(app, &req.owner, &account)
                .await
                .map_err(|e| UserError::Backend(e.to_string()))?;
            return Ok(record_to_profile(&account, &rec));
        }
        let count = self.count_bots(app, &req.owner).await?;
        if count >= BOT_MAX_PER_OWNER {
            return Err(UserError::Limit);
        }
        let account = format!("b_{}", base36_upper(idgen.next_id()?));
        let nickname = if req.nickname.trim().is_empty() {
            account.clone()
        } else {
            validate_patch(&ProfilePatch {
                nickname: req.nickname.clone(),
                avatar: req.avatar.clone(),
                bio: req.bio.clone(),
            })?
            .nickname
        };
        let avatar = req.avatar.trim().to_string();
        let bio = req.bio.trim().to_string();
        if avatar.chars().count() > AVATAR_MAX_CHARS || bio.chars().count() > BIO_MAX_CHARS {
            return Err(UserError::InvalidProfile);
        }
        let raced = {
            let mut inner = self.write();
            if let Some((existing, rec)) =
                Self::find_bot_by_profile(&inner, app, &req.owner, &req.client_profile_id)
            {
                Some((existing, rec))
            } else {
                let key = (app.to_string(), account.clone());
                if inner.contains_key(&key) {
                    return Err(UserError::Conflict);
                }
                inner.insert(
                    key,
                    UserRecord {
                        password_hash: None,
                        nickname: nickname.clone(),
                        avatar: avatar.clone(),
                        bio: bio.clone(),
                        token_epoch: 0,
                        kind: PROFILE_KIND_BOT,
                        owner_account: req.owner.clone(),
                        client_profile_id: req.client_profile_id.clone(),
                    },
                );
                None
            }
        };
        if let Some((existing, rec)) = raced {
            social
                .ensure_friends(app, &req.owner, &existing)
                .await
                .map_err(|e| UserError::Backend(e.to_string()))?;
            return Ok(record_to_profile(&existing, &rec));
        }
        social
            .ensure_friends(app, &req.owner, &account)
            .await
            .map_err(|e| UserError::Backend(e.to_string()))?;
        Ok(UserProfile {
            account,
            nickname,
            avatar,
            bio,
            kind: PROFILE_KIND_BOT,
        })
    }

    async fn bot_owner(&self, app: &str, account: &str) -> Result<Option<String>, UserError> {
        Ok(self.lookup(app, account).await?.and_then(|p| {
            if p.kind == PROFILE_KIND_BOT && !p.owner_account.is_empty() {
                Some(p.owner_account)
            } else {
                None
            }
        }))
    }

    async fn delete_bot(&self, app: &str, owner: &str, account: &str) -> Result<(), UserError> {
        let rec = {
            let inner = self.read();
            inner
                .get(&(app.to_string(), account.to_string()))
                .cloned()
                .ok_or(UserError::NotFound)?
        };
        if rec.kind != PROFILE_KIND_BOT {
            return Err(UserError::NotFound);
        }
        if rec.owner_account != owner {
            return Err(UserError::NotBotOwner);
        }
        self.write().remove(&(app.to_string(), account.to_string()));
        if let Some(social) = &self.social {
            let _ = social.remove(app, owner, account).await;
        }
        Ok(())
    }

    async fn count_bots(&self, app: &str, owner: &str) -> Result<u32, UserError> {
        let n = self
            .read()
            .iter()
            .filter(|((row_app, _), rec)| {
                row_app.as_str() == app
                    && rec.kind == PROFILE_KIND_BOT
                    && rec.owner_account == owner
            })
            .count();
        u32::try_from(n).map_err(|e| UserError::Backend(e.to_string()))
    }

    async fn lookup(&self, app: &str, account: &str) -> Result<Option<UserPresence>, UserError> {
        Ok(self
            .read()
            .get(&(app.to_string(), account.to_string()))
            .map(presence_from_record))
    }
}

#[cfg(feature = "postgres")]
pub struct PostgresUserDirectory {
    pool: sqlx::PgPool,
    idgen: Option<Arc<dyn IdGenerator>>,
}

#[cfg(feature = "postgres")]
impl PostgresUserDirectory {
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self { pool, idgen: None }
    }

    pub fn from_pool_with_idgen(pool: sqlx::PgPool, idgen: Arc<dyn IdGenerator>) -> Self {
        Self {
            pool,
            idgen: Some(idgen),
        }
    }
}

#[cfg(feature = "postgres")]
fn pg_err(e: sqlx::Error) -> UserError {
    UserError::Backend(e.to_string())
}

#[cfg(feature = "postgres")]
fn row_profile(
    account: String,
    nickname: String,
    avatar: String,
    bio: String,
    kind: String,
) -> UserProfile {
    UserProfile {
        nickname: if nickname.is_empty() {
            account.clone()
        } else {
            nickname
        },
        account,
        avatar,
        bio,
        kind: kind_from_db(&kind),
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
        let row: Option<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT account, nickname, avatar, bio, kind FROM users
             WHERE app = $1 AND account = $2",
        )
        .bind(app)
        .bind(account)
        .fetch_optional(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(row.map(|(account, nickname, avatar, bio, kind)| {
            row_profile(account, nickname, avatar, bio, kind)
        }))
    }

    async fn update_profile(
        &self,
        app: &str,
        account: &str,
        patch: &ProfilePatch,
    ) -> Result<UserProfile, UserError> {
        let patch = validate_patch(patch)?;
        let row: Option<(String, String, String, String, String)> = sqlx::query_as(
            "UPDATE users SET nickname = $3, avatar = $4, bio = $5
             WHERE app = $1 AND account = $2
             RETURNING account, nickname, avatar, bio, kind",
        )
        .bind(app)
        .bind(account)
        .bind(&patch.nickname)
        .bind(&patch.avatar)
        .bind(&patch.bio)
        .fetch_optional(&self.pool)
        .await
        .map_err(pg_err)?;
        row.map(|(account, nickname, avatar, bio, kind)| {
            row_profile(account, nickname, avatar, bio, kind)
        })
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
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT account, nickname, avatar, bio, kind FROM users
             WHERE app = $1 AND account = ANY($2)",
        )
        .bind(app)
        .bind(accounts)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        let mut by_acc = HashMap::with_capacity(rows.len());
        for (account, nickname, avatar, bio, kind) in rows {
            by_acc.insert(
                account.clone(),
                row_profile(account, nickname, avatar, bio, kind),
            );
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
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT account, nickname, avatar, bio, kind FROM users
             WHERE app = $1
               AND kind = 'user'
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
            .map(|(account, nickname, avatar, bio, kind)| {
                row_profile(account, nickname, avatar, bio, kind)
            })
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
        u32::try_from(n).map_err(|e| UserError::Backend(e.to_string()))
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
        u32::try_from(n).map_err(|e| UserError::Backend(e.to_string()))
    }

    async fn create_bot(&self, app: &str, req: &CreateBot) -> Result<UserProfile, UserError> {
        let idgen = self
            .idgen
            .as_ref()
            .ok_or_else(|| UserError::Backend("create_bot requires idgen".into()))?;
        if !valid_client_profile_id(&req.client_profile_id) {
            return Err(UserError::InvalidProfile);
        }
        if req.owner.is_empty() {
            return Err(UserError::InvalidProfile);
        }
        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        let existing: Option<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT account, nickname, avatar, bio, kind FROM users
             WHERE app = $1 AND owner_account = $2 AND client_profile_id = $3 AND kind = 'bot'",
        )
        .bind(app)
        .bind(&req.owner)
        .bind(&req.client_profile_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(pg_err)?;
        if let Some((account, nickname, avatar, bio, kind)) = existing {
            let (a, b) = crate::social::ordered_pair(&req.owner, &account);
            sqlx::query(
                "INSERT INTO friendships (app, account_a, account_b)
                 VALUES ($1, $2, $3)
                 ON CONFLICT DO NOTHING",
            )
            .bind(app)
            .bind(a)
            .bind(b)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(row_profile(account, nickname, avatar, bio, kind));
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE app = $1 AND owner_account = $2 AND kind = 'bot'",
        )
        .bind(app)
        .bind(&req.owner)
        .fetch_one(&mut *tx)
        .await
        .map_err(pg_err)?;
        if count >= i64::from(BOT_MAX_PER_OWNER) {
            return Err(UserError::Limit);
        }
        let account = format!("b_{}", base36_upper(idgen.next_id()?));
        let nickname = if req.nickname.trim().is_empty() {
            account.clone()
        } else {
            validate_patch(&ProfilePatch {
                nickname: req.nickname.clone(),
                avatar: req.avatar.clone(),
                bio: req.bio.clone(),
            })?
            .nickname
        };
        let avatar = req.avatar.trim().to_string();
        let bio = req.bio.trim().to_string();
        if avatar.chars().count() > AVATAR_MAX_CHARS || bio.chars().count() > BIO_MAX_CHARS {
            return Err(UserError::InvalidProfile);
        }
        sqlx::query(
            "INSERT INTO users (app, account, nickname, avatar, bio, kind, owner_account, client_profile_id)
             VALUES ($1, $2, $3, $4, $5, 'bot', $6, $7)",
        )
        .bind(app)
        .bind(&account)
        .bind(&nickname)
        .bind(&avatar)
        .bind(&bio)
        .bind(&req.owner)
        .bind(&req.client_profile_id)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        let (a, b) = crate::social::ordered_pair(&req.owner, &account);
        sqlx::query(
            "INSERT INTO friendships (app, account_a, account_b)
             VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
        )
        .bind(app)
        .bind(a)
        .bind(b)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        tx.commit().await.map_err(pg_err)?;
        Ok(UserProfile {
            account,
            nickname,
            avatar,
            bio,
            kind: PROFILE_KIND_BOT,
        })
    }

    async fn bot_owner(&self, app: &str, account: &str) -> Result<Option<String>, UserError> {
        Ok(self.lookup(app, account).await?.and_then(|p| {
            if p.kind == PROFILE_KIND_BOT && !p.owner_account.is_empty() {
                Some(p.owner_account)
            } else {
                None
            }
        }))
    }

    async fn delete_bot(&self, app: &str, owner: &str, account: &str) -> Result<(), UserError> {
        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT kind, owner_account FROM users WHERE app = $1 AND account = $2")
                .bind(app)
                .bind(account)
                .fetch_optional(&mut *tx)
                .await
                .map_err(pg_err)?;
        let (kind, owner_account) = row.ok_or(UserError::NotFound)?;
        if kind != "bot" {
            return Err(UserError::NotFound);
        }
        if owner_account != owner {
            return Err(UserError::NotBotOwner);
        }
        let (a, b) = crate::social::ordered_pair(owner, account);
        sqlx::query("DELETE FROM friendships WHERE app = $1 AND account_a = $2 AND account_b = $3")
            .bind(app)
            .bind(a)
            .bind(b)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
        sqlx::query(
            "DELETE FROM friend_requests
             WHERE app = $1 AND (
               (from_account = $2 AND to_account = $3)
               OR (from_account = $3 AND to_account = $2)
             )",
        )
        .bind(app)
        .bind(owner)
        .bind(account)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        sqlx::query(
            "DELETE FROM blocks
             WHERE app = $1 AND (
               (account = $2 AND blocked = $3) OR (account = $3 AND blocked = $2)
             )",
        )
        .bind(app)
        .bind(owner)
        .bind(account)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        sqlx::query("DELETE FROM bot_turns WHERE app = $1 AND bot_account = $2")
            .bind(app)
            .bind(account)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
        sqlx::query(
            "DELETE FROM users WHERE app = $1 AND account = $2 AND kind = 'bot' AND owner_account = $3",
        )
        .bind(app)
        .bind(account)
        .bind(owner)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        tx.commit().await.map_err(pg_err)?;
        Ok(())
    }

    async fn count_bots(&self, app: &str, owner: &str) -> Result<u32, UserError> {
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE app = $1 AND owner_account = $2 AND kind = 'bot'",
        )
        .bind(app)
        .bind(owner)
        .fetch_one(&self.pool)
        .await
        .map_err(pg_err)?;
        u32::try_from(n).map_err(|e| UserError::Backend(e.to_string()))
    }

    async fn lookup(&self, app: &str, account: &str) -> Result<Option<UserPresence>, UserError> {
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT kind, owner_account FROM users WHERE app = $1 AND account = $2")
                .bind(app)
                .bind(account)
                .fetch_optional(&self.pool)
                .await
                .map_err(pg_err)?;
        Ok(row.map(|(kind, owner_account)| UserPresence {
            exists: true,
            kind: kind_from_db(&kind),
            owner_account,
        }))
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
        assert_eq!(dir.token_epoch("kim", "alice").await.unwrap(), 0);
        let p = dir.profile("kim", "alice").await.unwrap().unwrap();
        assert_eq!(p.nickname, "alice");
        assert_eq!(p.kind, PROFILE_KIND_USER);
        assert!(!p.is_bot());
    }

    #[tokio::test]
    async fn memory_profile_exposes_bot_kind() {
        let dir = MemoryUserDirectory::new();
        dir.write().insert(
            ("kim".into(), "bot_x".into()),
            UserRecord {
                password_hash: None,
                nickname: "助手".into(),
                avatar: String::new(),
                bio: String::new(),
                token_epoch: 0,
                kind: PROFILE_KIND_BOT,
                owner_account: "alice".into(),
                client_profile_id: "goose".into(),
            },
        );
        let p = dir.profile("kim", "bot_x").await.unwrap().unwrap();
        assert_eq!(p.kind, PROFILE_KIND_BOT);
        assert!(p.is_bot());
        assert_eq!(
            dir.read()
                .get(&("kim".into(), "bot_x".into()))
                .map(|r| r.owner_account.as_str()),
            Some("alice")
        );
        let hits = dir.search("kim", "bot", &[], 10).await.unwrap();
        assert!(hits.is_empty());
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

    #[tokio::test]
    async fn memory_set_password_and_bump_epoch_updates_both() {
        let dir = MemoryUserDirectory::new();
        dir.create("kim", "alice", "old-hash").await.unwrap();
        dir.set_password("kim", "alice", "mid-hash").await.unwrap();
        assert_eq!(dir.token_epoch("kim", "alice").await.unwrap(), 0);
        let ver = dir
            .set_password_and_bump_epoch("kim", "alice", "new-hash")
            .await
            .unwrap();
        assert_eq!(ver, 1);
        assert_eq!(
            dir.password_hash("kim", "alice").await.unwrap().as_deref(),
            Some("new-hash")
        );
        assert_eq!(dir.token_epoch("kim", "alice").await.unwrap(), 1);
        let missing = dir
            .set_password_and_bump_epoch("kim", "bob", "x")
            .await
            .unwrap_err();
        assert!(matches!(missing, UserError::NotFound));
    }

    #[tokio::test]
    async fn memory_create_bot_friends_and_idempotent() {
        use crate::idgen::SequenceIdGen;
        use crate::social::{MemorySocialDirectory, SocialDirectory};

        let social = Arc::new(MemorySocialDirectory::new());
        let dir = MemoryUserDirectory::new()
            .with_social(social.clone())
            .with_idgen(Arc::new(SequenceIdGen::new(1)));
        dir.upsert("kim", "alice").await.unwrap();
        let req = CreateBot {
            owner: "alice".into(),
            client_profile_id: "goose".into(),
            nickname: "助手".into(),
            avatar: String::new(),
            bio: String::new(),
        };
        let first = dir.create_bot("kim", &req).await.unwrap();
        assert_eq!(first.kind, PROFILE_KIND_BOT);
        assert!(first.account.starts_with("b_"));
        assert!(social
            .is_friend("kim", "alice", &first.account)
            .await
            .unwrap());
        let second = dir.create_bot("kim", &req).await.unwrap();
        assert_eq!(second.account, first.account);
        assert_eq!(dir.count_bots("kim", "alice").await.unwrap(), 1);
        let p = dir.lookup("kim", &first.account).await.unwrap().unwrap();
        assert_eq!(p.kind, PROFILE_KIND_BOT);
        assert_eq!(p.owner_account, "alice");
        assert_eq!(
            dir.bot_owner("kim", &first.account)
                .await
                .unwrap()
                .as_deref(),
            Some("alice")
        );
        assert!(dir.search("kim", "助手", &[], 10).await.unwrap().is_empty());
        assert!(matches!(
            MemoryUserDirectory::new()
                .create_bot("kim", &req)
                .await
                .unwrap_err(),
            UserError::Backend(_)
        ));
    }
}
