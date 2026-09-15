use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use kim_protocol::pkt::{
    AgentProviderAccount as PbAccount, AgentSpecRecord as PbRecord, AgentSpecSyncResp,
    AgentSpecUpsertReq, AgentSpecUpsertResp, Status,
};
use kim_router::{Context, RouterError};
use tracing::warn;

use crate::users::UserError;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentSpecRecord {
    pub profile_id: String,
    pub nickname: String,
    pub server_account: String,
    pub spec: Vec<u8>,
    pub key_ciphertext: Vec<u8>,
    pub updated_at: i64,
    pub deleted_at: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentProviderAccount {
    pub id: String,
    pub vendor_id: String,
    pub base_url: String,
    pub key_ref: String,
    pub display_name: String,
    pub models: Vec<String>,
    pub updated_at: i64,
    pub deleted_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentSpecError {
    #[error("invalid spec")]
    Invalid,
    #[error("not bot owner")]
    NotOwner,
    #[error("{0}")]
    Backend(String),
}

impl From<UserError> for AgentSpecError {
    fn from(err: UserError) -> Self {
        match err {
            UserError::NotBotOwner => AgentSpecError::NotOwner,
            other => AgentSpecError::Backend(other.to_string()),
        }
    }
}

#[async_trait]
pub trait AgentSpecStore: Send + Sync {
    async fn list(&self, app: &str, owner: &str) -> Result<Vec<AgentSpecRecord>, AgentSpecError>;
    async fn upsert(
        &self,
        app: &str,
        owner: &str,
        rec: AgentSpecRecord,
    ) -> Result<AgentSpecRecord, AgentSpecError>;
    async fn list_accounts(
        &self,
        app: &str,
        owner: &str,
    ) -> Result<Vec<AgentProviderAccount>, AgentSpecError>;
    async fn upsert_account(
        &self,
        app: &str,
        owner: &str,
        rec: AgentProviderAccount,
    ) -> Result<AgentProviderAccount, AgentSpecError>;
}

type OwnerItemKey = (String, String, String);
type SharedOwnerMap<T> = Arc<RwLock<HashMap<OwnerItemKey, T>>>;

#[derive(Clone, Default)]
pub struct MemoryAgentSpecStore {
    inner: SharedOwnerMap<AgentSpecRecord>,
    accounts: SharedOwnerMap<AgentProviderAccount>,
}

impl MemoryAgentSpecStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AgentSpecStore for MemoryAgentSpecStore {
    async fn list(&self, app: &str, owner: &str) -> Result<Vec<AgentSpecRecord>, AgentSpecError> {
        let guard = self
            .inner
            .read()
            .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
        let mut out: Vec<AgentSpecRecord> = guard
            .iter()
            .filter(|((a, o, _), _)| a == app && o == owner)
            .map(|(_, r)| r.clone())
            .collect();
        out.sort_by(|a, b| a.profile_id.cmp(&b.profile_id));
        Ok(out)
    }

    async fn upsert(
        &self,
        app: &str,
        owner: &str,
        rec: AgentSpecRecord,
    ) -> Result<AgentSpecRecord, AgentSpecError> {
        if rec.profile_id.is_empty() || rec.spec.is_empty() {
            return Err(AgentSpecError::Invalid);
        }
        let key = (app.to_string(), owner.to_string(), rec.profile_id.clone());
        let mut guard = self
            .inner
            .write()
            .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
        if let Some(stored) = guard.get(&key) {
            if rec.updated_at < stored.updated_at {
                return Ok(stored.clone());
            }
        }
        guard.insert(key, rec.clone());
        Ok(rec)
    }

    async fn list_accounts(
        &self,
        app: &str,
        owner: &str,
    ) -> Result<Vec<AgentProviderAccount>, AgentSpecError> {
        let guard = self
            .accounts
            .read()
            .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
        let mut out: Vec<AgentProviderAccount> = guard
            .iter()
            .filter(|((a, o, _), _)| a == app && o == owner)
            .map(|(_, r)| r.clone())
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    async fn upsert_account(
        &self,
        app: &str,
        owner: &str,
        rec: AgentProviderAccount,
    ) -> Result<AgentProviderAccount, AgentSpecError> {
        if rec.id.is_empty() {
            return Err(AgentSpecError::Invalid);
        }
        let key = (app.to_string(), owner.to_string(), rec.id.clone());
        let mut guard = self
            .accounts
            .write()
            .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
        if let Some(stored) = guard.get(&key) {
            if rec.updated_at < stored.updated_at {
                return Ok(stored.clone());
            }
        }
        guard.insert(key, rec.clone());
        Ok(rec)
    }
}

fn to_pb(r: &AgentSpecRecord) -> PbRecord {
    PbRecord {
        profile_id: r.profile_id.clone(),
        nickname: r.nickname.clone(),
        server_account: r.server_account.clone(),
        spec: r.spec.clone(),
        key_ciphertext: r.key_ciphertext.clone(),
        updated_at: r.updated_at,
        deleted_at: r.deleted_at,
    }
}

fn to_pb_account(a: &AgentProviderAccount) -> PbAccount {
    PbAccount {
        id: a.id.clone(),
        vendor_id: a.vendor_id.clone(),
        base_url: a.base_url.clone(),
        key_ref: a.key_ref.clone(),
        display_name: a.display_name.clone(),
        models: a.models.clone(),
        updated_at: a.updated_at,
        deleted_at: a.deleted_at,
    }
}

fn from_pb_account(a: PbAccount) -> AgentProviderAccount {
    AgentProviderAccount {
        id: a.id,
        vendor_id: a.vendor_id,
        base_url: a.base_url,
        key_ref: a.key_ref,
        display_name: a.display_name,
        models: a.models,
        updated_at: a.updated_at,
        deleted_at: a.deleted_at,
    }
}

fn from_pb(r: PbRecord) -> AgentSpecRecord {
    AgentSpecRecord {
        profile_id: r.profile_id,
        nickname: r.nickname,
        server_account: r.server_account,
        spec: r.spec,
        key_ciphertext: r.key_ciphertext,
        updated_at: r.updated_at,
        deleted_at: r.deleted_at,
    }
}

fn status_of(err: &AgentSpecError) -> Status {
    match err {
        AgentSpecError::Invalid => Status::InvalidPacketBody,
        AgentSpecError::NotOwner => Status::NotBotOwner,
        AgentSpecError::Backend(_) => Status::SystemException,
    }
}

fn owner_or_deny(ctx: &Context) -> Result<String, Status> {
    let dest = ctx.header().dest.as_str();
    if !dest.is_empty() && dest != ctx.session().account {
        return Err(Status::NotBotOwner);
    }
    Ok(ctx.session().account.clone())
}

pub async fn do_agent_spec_sync(
    ctx: Context,
    store: &dyn AgentSpecStore,
) -> Result<(), RouterError> {
    let owner = match owner_or_deny(&ctx) {
        Ok(o) => o,
        Err(status) => {
            ctx.resp_bytes(status, bytes::Bytes::new()).await?;
            return Ok(());
        }
    };
    let records = match store.list(&ctx.session().app, &owner).await {
        Ok(rows) => rows,
        Err(err) => {
            warn!(%err, "agent spec sync failed");
            ctx.resp_with_error(status_of(&err), &err).await?;
            return Ok(());
        }
    };
    let accounts = match store.list_accounts(&ctx.session().app, &owner).await {
        Ok(rows) => rows,
        Err(err) => {
            warn!(%err, "agent account sync failed");
            ctx.resp_with_error(status_of(&err), &err).await?;
            return Ok(());
        }
    };
    ctx.resp(
        Status::Success,
        Some(&AgentSpecSyncResp {
            records: records.iter().map(to_pb).collect(),
            accounts: accounts.iter().map(to_pb_account).collect(),
        }),
    )
    .await?;
    Ok(())
}

pub async fn do_agent_spec_upsert(
    ctx: Context,
    store: &dyn AgentSpecStore,
) -> Result<(), RouterError> {
    let owner = match owner_or_deny(&ctx) {
        Ok(o) => o,
        Err(status) => {
            ctx.resp_bytes(status, bytes::Bytes::new()).await?;
            return Ok(());
        }
    };
    let req = match ctx.read_body::<AgentSpecUpsertReq>() {
        Ok(r) => r,
        Err(err) => {
            ctx.resp_with_error(Status::InvalidPacketBody, &err).await?;
            return Ok(());
        }
    };
    if req.record.is_none() && req.account.is_none() {
        ctx.resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await?;
        return Ok(());
    }
    if let Some(acc) = req.account {
        let rec = from_pb_account(acc);
        if rec.id.is_empty() {
            ctx.resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
                .await?;
            return Ok(());
        }
        if let Err(err) = store.upsert_account(&ctx.session().app, &owner, rec).await {
            warn!(%err, "agent account upsert failed");
            ctx.resp_with_error(status_of(&err), &err).await?;
            return Ok(());
        }
    }
    let Some(pb) = req.record else {
        ctx.resp(Status::Success, Some(&AgentSpecUpsertResp { record: None }))
            .await?;
        return Ok(());
    };
    let rec = from_pb(pb);
    if rec.profile_id.is_empty() || rec.spec.is_empty() {
        ctx.resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await?;
        return Ok(());
    }
    match store.upsert(&ctx.session().app, &owner, rec).await {
        Ok(stored) => {
            ctx.resp(
                Status::Success,
                Some(&AgentSpecUpsertResp {
                    record: Some(to_pb(&stored)),
                }),
            )
            .await?;
        }
        Err(err) => {
            warn!(%err, "agent spec upsert failed");
            ctx.resp_with_error(status_of(&err), &err).await?;
        }
    }
    Ok(())
}

pub async fn open_agent_spec_store(
    database_url: Option<&str>,
) -> Result<Arc<dyn AgentSpecStore>, AgentSpecError> {
    match database_url {
        None | Some("") => Ok(Arc::new(MemoryAgentSpecStore::new())),
        Some(url) => open_postgres_agent_specs(url).await,
    }
}

#[cfg(feature = "postgres")]
async fn open_postgres_agent_specs(url: &str) -> Result<Arc<dyn AgentSpecStore>, AgentSpecError> {
    let pool = crate::store::connect_pool(url, crate::store::PoolOpts::default())
        .await
        .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
    Ok(Arc::new(PostgresAgentSpecStore { pool }))
}

#[cfg(not(feature = "postgres"))]
async fn open_postgres_agent_specs(_url: &str) -> Result<Arc<dyn AgentSpecStore>, AgentSpecError> {
    Err(AgentSpecError::Backend(
        "rebuild with --features postgres".into(),
    ))
}

#[cfg(feature = "postgres")]
struct PostgresAgentSpecStore {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresAgentSpecStore {
    async fn get_one(
        &self,
        app: &str,
        owner: &str,
        profile_id: &str,
    ) -> Result<Option<AgentSpecRecord>, AgentSpecError> {
        let rows = self.list(app, owner).await?;
        Ok(rows.into_iter().find(|r| r.profile_id == profile_id))
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl AgentSpecStore for PostgresAgentSpecStore {
    async fn list(&self, app: &str, owner: &str) -> Result<Vec<AgentSpecRecord>, AgentSpecError> {
        let rows: Vec<(
            String,
            String,
            String,
            Vec<u8>,
            Option<Vec<u8>>,
            i64,
            Option<i64>,
        )> = sqlx::query_as(
            r"
            SELECT profile_id, nickname, server_account, spec, key_ciphertext, updated_at, deleted_at
            FROM agent_specs WHERE app = $1 AND owner = $2
            ORDER BY profile_id
            ",
        )
        .bind(app)
        .bind(owner)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(
                |(profile_id, nickname, server_account, spec, key, updated_at, deleted_at)| {
                    AgentSpecRecord {
                        profile_id,
                        nickname,
                        server_account,
                        spec,
                        key_ciphertext: key.unwrap_or_default(),
                        updated_at,
                        deleted_at: deleted_at.unwrap_or(0),
                    }
                },
            )
            .collect())
    }

    async fn upsert(
        &self,
        app: &str,
        owner: &str,
        rec: AgentSpecRecord,
    ) -> Result<AgentSpecRecord, AgentSpecError> {
        if rec.profile_id.is_empty() || rec.spec.is_empty() {
            return Err(AgentSpecError::Invalid);
        }
        let stored: Option<(i64,)> = sqlx::query_as(
            "SELECT updated_at FROM agent_specs WHERE app = $1 AND owner = $2 AND profile_id = $3",
        )
        .bind(app)
        .bind(owner)
        .bind(&rec.profile_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
        if let Some((updated_at,)) = stored {
            if rec.updated_at < updated_at {
                return self
                    .get_one(app, owner, &rec.profile_id)
                    .await?
                    .ok_or_else(|| AgentSpecError::Backend("missing after lww".into()));
            }
        }
        let deleted = if rec.deleted_at > 0 {
            Some(rec.deleted_at)
        } else {
            None
        };
        let key_ct = if rec.key_ciphertext.is_empty() {
            None
        } else {
            Some(rec.key_ciphertext.clone())
        };
        sqlx::query(
            r"
            INSERT INTO agent_specs (
              app, owner, profile_id, nickname, server_account, spec, key_ciphertext, updated_at, deleted_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (app, owner, profile_id) DO UPDATE SET
              nickname = excluded.nickname,
              server_account = excluded.server_account,
              spec = excluded.spec,
              key_ciphertext = excluded.key_ciphertext,
              updated_at = excluded.updated_at,
              deleted_at = excluded.deleted_at
            ",
        )
        .bind(app)
        .bind(owner)
        .bind(&rec.profile_id)
        .bind(&rec.nickname)
        .bind(&rec.server_account)
        .bind(&rec.spec)
        .bind(key_ct)
        .bind(rec.updated_at)
        .bind(deleted)
        .execute(&self.pool)
        .await
        .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
        Ok(rec)
    }

    async fn list_accounts(
        &self,
        app: &str,
        owner: &str,
    ) -> Result<Vec<AgentProviderAccount>, AgentSpecError> {
        let rows: Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            i64,
            Option<i64>,
        )> = sqlx::query_as(
            r"
            SELECT id, vendor_id, base_url, key_ref, display_name, models, updated_at, deleted_at
            FROM agent_provider_accounts WHERE app = $1 AND owner = $2
            ORDER BY id
            ",
        )
        .bind(app)
        .bind(owner)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    vendor_id,
                    base_url,
                    key_ref,
                    display_name,
                    models,
                    updated_at,
                    deleted_at,
                )| {
                    AgentProviderAccount {
                        id,
                        vendor_id,
                        base_url,
                        key_ref,
                        display_name,
                        models: serde_json::from_str(&models).unwrap_or_default(),
                        updated_at,
                        deleted_at: deleted_at.unwrap_or(0),
                    }
                },
            )
            .collect())
    }

    async fn upsert_account(
        &self,
        app: &str,
        owner: &str,
        rec: AgentProviderAccount,
    ) -> Result<AgentProviderAccount, AgentSpecError> {
        if rec.id.is_empty() {
            return Err(AgentSpecError::Invalid);
        }
        let stored: Option<(i64,)> = sqlx::query_as(
            "SELECT updated_at FROM agent_provider_accounts WHERE app = $1 AND owner = $2 AND id = $3",
        )
        .bind(app)
        .bind(owner)
        .bind(&rec.id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
        if let Some((updated_at,)) = stored {
            if rec.updated_at < updated_at {
                let rows = self.list_accounts(app, owner).await?;
                return rows
                    .into_iter()
                    .find(|a| a.id == rec.id)
                    .ok_or_else(|| AgentSpecError::Backend("missing after lww".into()));
            }
        }
        let models = serde_json::to_string(&rec.models).unwrap_or_else(|_| "[]".into());
        let deleted = if rec.deleted_at > 0 {
            Some(rec.deleted_at)
        } else {
            None
        };
        sqlx::query(
            r"
            INSERT INTO agent_provider_accounts (
              app, owner, id, vendor_id, base_url, key_ref, display_name, models, updated_at, deleted_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (app, owner, id) DO UPDATE SET
              vendor_id = excluded.vendor_id,
              base_url = excluded.base_url,
              key_ref = excluded.key_ref,
              display_name = excluded.display_name,
              models = excluded.models,
              updated_at = excluded.updated_at,
              deleted_at = excluded.deleted_at
            ",
        )
        .bind(app)
        .bind(owner)
        .bind(&rec.id)
        .bind(&rec.vendor_id)
        .bind(&rec.base_url)
        .bind(&rec.key_ref)
        .bind(&rec.display_name)
        .bind(models)
        .bind(rec.updated_at)
        .bind(deleted)
        .execute(&self.pool)
        .await
        .map_err(|e| AgentSpecError::Backend(e.to_string()))?;
        Ok(rec)
    }
}
