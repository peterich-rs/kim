//! Message persistence: write-fanout inbox rows + content + a per-account read index.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;

use crate::directory::GroupDirectory;
use crate::idgen::{IdError, IdGenerator};
use crate::users::UserDirectory;

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "redis")]
mod redis_ack;

#[cfg(feature = "postgres")]
pub use postgres::{connect_pool, PoolOpts, PostgresMessageStore};

pub const DIRECTION_RECV: i32 = 0;
pub const DIRECTION_SEND: i32 = 1;
pub const OFFLINE_SYNC_INDEX_COUNT: usize = 2000;
pub const MESSAGE_MAX_COUNT_PER_PAGE: usize = 200;
pub const INBOX_PAGE: usize = 50;
pub const INBOX_MAX: usize = 100;
pub const HISTORY_PAGE: usize = 50;
pub const HISTORY_MAX: usize = 100;
pub const ACK_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
pub(crate) const DAY_NANOS: i64 = 24 * 60 * 60 * 1_000_000_000;
const EXPIRES_NANOS: i64 = 15 * DAY_NANOS;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("idgen: {0}")]
    Id(#[from] IdError),
    #[error("royal http {status}: {msg}")]
    Http { status: u16, msg: String },
    #[error("{0}")]
    Backend(String),
}

pub struct InsertMessage {
    pub sender: String,
    pub dest: String,
    pub send_time: i64,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
    pub client_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fanout {
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
    pub sender: String,
    pub dest: String,
    pub kind: MessageKind,
    pub recipients: Vec<String>,
}

pub struct InsertResult {
    pub message_id: i64,
    pub send_time: i64,
    pub duplicate: bool,
    pub fanout: Fanout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MessageKind {
    User,
    Group,
}

pub(crate) fn unique_accounts(accounts: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for account in accounts {
        if seen.insert(account.clone()) {
            out.push(account);
        }
    }
    out
}

/// Only real index rows (no NULLs). Empty slice = ghost content.
pub(crate) fn fanout_from_index_rows(
    msg_type: i32,
    body: String,
    extra: String,
    rows: &[(String, String, i32, String)],
) -> Fanout {
    if rows.is_empty() {
        return Fanout {
            msg_type,
            body,
            extra,
            sender: String::new(),
            dest: String::new(),
            kind: MessageKind::User,
            recipients: Vec::new(),
        };
    }
    let recipients = unique_accounts(rows.iter().map(|r| r.0.clone()));
    if let Some((_, account_b, _, group_id)) = rows.iter().find(|r| !r.3.is_empty()) {
        return Fanout {
            msg_type,
            body,
            extra,
            sender: account_b.clone(),
            dest: group_id.clone(),
            kind: MessageKind::Group,
            recipients,
        };
    }
    let (sender, dest) = match rows.iter().find(|r| r.2 == DIRECTION_SEND) {
        Some((a, b, _, _)) => (a.clone(), b.clone()),
        None => {
            let (a, b, _, _) = &rows[0];
            (b.clone(), a.clone())
        }
    };
    Fanout {
        msg_type,
        body,
        extra,
        sender,
        dest,
        kind: MessageKind::User,
        recipients,
    }
}

pub(crate) fn fanout_from_write(
    kind: MessageKind,
    req: &InsertMessage,
    members: &[String],
) -> Fanout {
    let tuples: Vec<(String, String, i32, String)> = match kind {
        MessageKind::User => vec![
            (
                req.sender.clone(),
                req.dest.clone(),
                DIRECTION_SEND,
                String::new(),
            ),
            (
                req.dest.clone(),
                req.sender.clone(),
                DIRECTION_RECV,
                String::new(),
            ),
        ],
        MessageKind::Group => members
            .iter()
            .map(|m| {
                let dir = if m == &req.sender {
                    DIRECTION_SEND
                } else {
                    DIRECTION_RECV
                };
                (m.clone(), req.sender.clone(), dir, req.dest.clone())
            })
            .collect(),
    };
    fanout_from_index_rows(req.msg_type, req.body.clone(), req.extra.clone(), &tuples)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredMessage {
    pub message_id: i64,
    pub app: String,
    pub kind: MessageKind,
    pub sender: String,
    pub dest: String,
    pub send_time: i64,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageIndexRow {
    pub message_id: i64,
    pub direction: i32,
    pub send_time: i64,
    pub account_b: String,
    pub group: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageContentRow {
    pub message_id: i64,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboxEntry {
    pub dest: String,
    pub kind: MessageKind,
    pub last_message_id: i64,
    pub last_send_time: i64,
    pub last_body: String,
    pub last_sender: String,
    pub last_msg_type: i32,
    pub unread: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub message_id: i64,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
    pub sender: String,
    pub send_time: i64,
    pub direction: i32,
}

pub fn clamp_page(requested: i32, default: usize, max: usize) -> usize {
    if requested <= 0 {
        default
    } else {
        (requested as usize).min(max)
    }
}

#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn insert_user(&self, app: &str, req: &InsertMessage)
        -> Result<InsertResult, StoreError>;
    async fn insert_group(
        &self,
        app: &str,
        req: &InsertMessage,
        members: &[String],
    ) -> Result<InsertResult, StoreError>;
    async fn ack(&self, app: &str, account: &str, message_id: i64) -> Result<(), StoreError>;
    async fn offline_index(
        &self,
        app: &str,
        account: &str,
        message_id: i64,
    ) -> Result<Vec<MessageIndexRow>, StoreError>;
    async fn offline_content(
        &self,
        app: &str,
        account: &str,
        message_ids: &[i64],
    ) -> Result<Vec<MessageContentRow>, StoreError>;
    async fn inbox(
        &self,
        app: &str,
        account: &str,
        limit: i32,
    ) -> Result<Vec<InboxEntry>, StoreError>;
    async fn history(
        &self,
        app: &str,
        account: &str,
        dest: &str,
        kind: MessageKind,
        before_id: i64,
        limit: i32,
    ) -> Result<Vec<HistoryEntry>, StoreError>;
    async fn mark_read(
        &self,
        app: &str,
        account: &str,
        dest: &str,
        kind: MessageKind,
        message_id: i64,
    ) -> Result<(), StoreError>;
}

#[async_trait]
pub(crate) trait AckIndex: Send + Sync {
    async fn get(&self, account: &str) -> Result<i64, StoreError>;
    async fn set(&self, account: &str, message_id: i64) -> Result<(), StoreError>;
}

pub(crate) struct MemoryAckIndex {
    inner: RwLock<HashMap<String, AckEntry>>,
}

struct AckEntry {
    message_id: i64,
    expires_at: Instant,
}

impl MemoryAckIndex {
    pub(crate) fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, HashMap<String, AckEntry>> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashMap<String, AckEntry>> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }
}

#[async_trait]
impl AckIndex for MemoryAckIndex {
    async fn get(&self, account: &str) -> Result<i64, StoreError> {
        let now = Instant::now();
        let inner = self.read();
        Ok(inner
            .get(account)
            .filter(|e| e.expires_at > now)
            .map(|e| e.message_id)
            .unwrap_or(0))
    }

    async fn set(&self, account: &str, message_id: i64) -> Result<(), StoreError> {
        let mut inner = self.write();
        inner.insert(
            account.to_string(),
            AckEntry {
                message_id,
                expires_at: Instant::now() + ACK_TTL,
            },
        );
        Ok(())
    }
}

pub(crate) fn now_unix_nano() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    )
    .unwrap_or(i64::MAX)
}

pub(crate) fn clamp_start(start: i64, now: i64) -> i64 {
    let earliest = now.saturating_sub(EXPIRES_NANOS);
    if start == 0 || start < earliest {
        earliest
    } else {
        start
    }
}

/// In-process write-fanout log. No disk; process exit drops all.
pub struct MemoryMessageStore {
    idgen: Arc<dyn IdGenerator>,
    ack: Arc<dyn AckIndex>,
    inner: RwLock<Inner>,
}

#[derive(Default)]
struct Inner {
    contents: HashMap<i64, StoredMessage>,
    indexes: Vec<InboxRow>,
    idempotency: HashMap<(String, String, String), (i64, i64)>,
    /// (app, account, peer, group_id) -> last_read_id
    reads: HashMap<(String, String, String, String), i64>,
}

#[derive(Clone, Debug)]
struct InboxRow {
    app: String,
    account_a: String,
    account_b: String,
    direction: i32,
    message_id: i64,
    group_id: String,
    send_time: i64,
}

impl MemoryMessageStore {
    pub fn new(idgen: Arc<dyn IdGenerator>) -> Self {
        Self::with_ack(idgen, Arc::new(MemoryAckIndex::new()))
    }

    fn with_ack(idgen: Arc<dyn IdGenerator>, ack: Arc<dyn AckIndex>) -> Self {
        Self {
            idgen,
            ack,
            inner: RwLock::new(Inner::default()),
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }

    fn fanout_from_memory(inner: &Inner, message_id: i64) -> Result<Fanout, StoreError> {
        let content = inner
            .contents
            .get(&message_id)
            .ok_or_else(|| StoreError::Backend("fanout missing".into()))?;
        let rows: Vec<(String, String, i32, String)> = inner
            .indexes
            .iter()
            .filter(|r| r.message_id == message_id)
            .map(|r| {
                (
                    r.account_a.clone(),
                    r.account_b.clone(),
                    r.direction,
                    r.group_id.clone(),
                )
            })
            .collect();
        Ok(fanout_from_index_rows(
            content.msg_type,
            content.body.clone(),
            content.extra.clone(),
            &rows,
        ))
    }

    fn insert(
        &self,
        app: &str,
        kind: MessageKind,
        req: &InsertMessage,
        members: &[String],
    ) -> Result<InsertResult, StoreError> {
        if !req.client_id.is_empty() {
            let key = (app.to_string(), req.sender.clone(), req.client_id.clone());
            let inner = self.read();
            if let Some(&(message_id, send_time)) = inner.idempotency.get(&key) {
                return Ok(InsertResult {
                    message_id,
                    send_time,
                    duplicate: true,
                    fanout: Self::fanout_from_memory(&inner, message_id)?,
                });
            }
        }
        let message_id = self.idgen.next_id()?;
        let content = StoredMessage {
            message_id,
            app: app.to_string(),
            kind,
            sender: req.sender.clone(),
            dest: req.dest.clone(),
            send_time: req.send_time,
            msg_type: req.msg_type,
            body: req.body.clone(),
            extra: req.extra.clone(),
        };
        let indexes = match kind {
            MessageKind::User => vec![
                InboxRow {
                    app: app.to_string(),
                    account_a: req.sender.clone(),
                    account_b: req.dest.clone(),
                    direction: DIRECTION_SEND,
                    message_id,
                    group_id: String::new(),
                    send_time: req.send_time,
                },
                InboxRow {
                    app: app.to_string(),
                    account_a: req.dest.clone(),
                    account_b: req.sender.clone(),
                    direction: DIRECTION_RECV,
                    message_id,
                    group_id: String::new(),
                    send_time: req.send_time,
                },
            ],
            MessageKind::Group => members
                .iter()
                .map(|m| InboxRow {
                    app: app.to_string(),
                    account_a: m.clone(),
                    account_b: req.sender.clone(),
                    direction: if m == &req.sender {
                        DIRECTION_SEND
                    } else {
                        DIRECTION_RECV
                    },
                    message_id,
                    group_id: req.dest.clone(),
                    send_time: req.send_time,
                })
                .collect(),
        };
        for _ in &indexes {
            let _ = self.idgen.next_id()?;
        }
        let mut inner = self.write();
        if !req.client_id.is_empty() {
            let key = (app.to_string(), req.sender.clone(), req.client_id.clone());
            if let Some(&(id, send_time)) = inner.idempotency.get(&key) {
                return Ok(InsertResult {
                    message_id: id,
                    send_time,
                    duplicate: true,
                    fanout: Self::fanout_from_memory(&inner, id)?,
                });
            }
            inner.idempotency.insert(key, (message_id, req.send_time));
        }
        inner.contents.insert(message_id, content);
        inner.indexes.extend(indexes);
        Ok(InsertResult {
            message_id,
            send_time: req.send_time,
            duplicate: false,
            fanout: fanout_from_write(kind, req, members),
        })
    }

    #[cfg(test)]
    pub fn recorded(&self) -> Vec<StoredMessage> {
        let inner = self.read();
        let mut rec: Vec<_> = inner.contents.values().cloned().collect();
        rec.sort_by_key(|m| m.message_id);
        rec
    }

    #[cfg(test)]
    fn recorded_indexes(&self) -> Vec<InboxRow> {
        self.read().indexes.clone()
    }

    async fn sent_time(&self, account: &str, message_id: i64) -> Result<i64, StoreError> {
        let mut id = message_id;
        if id == 0 {
            id = self.ack.get(account).await?;
        }
        let now = now_unix_nano();
        let start = if id > 0 {
            let inner = self.read();
            inner
                .contents
                .get(&id)
                .map(|c| c.send_time)
                .unwrap_or_else(|| now.saturating_sub(DAY_NANOS))
        } else {
            0
        };
        Ok(clamp_start(start, now))
    }
}

#[async_trait]
impl MessageStore for MemoryMessageStore {
    async fn insert_user(
        &self,
        app: &str,
        req: &InsertMessage,
    ) -> Result<InsertResult, StoreError> {
        self.insert(app, MessageKind::User, req, &[])
    }

    async fn insert_group(
        &self,
        app: &str,
        req: &InsertMessage,
        members: &[String],
    ) -> Result<InsertResult, StoreError> {
        self.insert(app, MessageKind::Group, req, members)
    }

    async fn ack(&self, _app: &str, account: &str, message_id: i64) -> Result<(), StoreError> {
        if message_id == 0 {
            return Ok(());
        }
        self.ack.set(account, message_id).await
    }

    async fn offline_index(
        &self,
        app: &str,
        account: &str,
        message_id: i64,
    ) -> Result<Vec<MessageIndexRow>, StoreError> {
        let start = self.sent_time(account, message_id).await?;
        let mut rows: Vec<MessageIndexRow> = {
            let inner = self.read();
            inner
                .indexes
                .iter()
                .filter(|r| {
                    r.app == app
                        && r.account_a == account
                        && r.direction == DIRECTION_RECV
                        && r.send_time > start
                })
                .map(|r| MessageIndexRow {
                    message_id: r.message_id,
                    direction: r.direction,
                    send_time: r.send_time,
                    account_b: r.account_b.clone(),
                    group: r.group_id.clone(),
                })
                .collect()
        };
        rows.sort_by_key(|r| r.send_time);
        rows.truncate(OFFLINE_SYNC_INDEX_COUNT);
        if message_id > 0 {
            self.ack.set(account, message_id).await?;
        }
        Ok(rows)
    }

    async fn offline_content(
        &self,
        app: &str,
        account: &str,
        message_ids: &[i64],
    ) -> Result<Vec<MessageContentRow>, StoreError> {
        let inner = self.read();
        Ok(message_ids
            .iter()
            .filter_map(|id| {
                let c = inner.contents.get(id)?;
                if c.app != app {
                    return None;
                }
                let visible = inner
                    .indexes
                    .iter()
                    .any(|r| r.message_id == *id && r.app == app && r.account_a == account);
                if !visible {
                    return None;
                }
                Some(MessageContentRow {
                    message_id: c.message_id,
                    msg_type: c.msg_type,
                    body: c.body.clone(),
                    extra: c.extra.clone(),
                })
            })
            .collect())
    }

    async fn inbox(
        &self,
        app: &str,
        account: &str,
        limit: i32,
    ) -> Result<Vec<InboxEntry>, StoreError> {
        let cap = clamp_page(limit, INBOX_PAGE, INBOX_MAX);
        let inner = self.read();
        let mut latest: HashMap<(MessageKind, String), InboxEntry> = HashMap::new();
        for row in inner
            .indexes
            .iter()
            .filter(|r| r.app == app && r.account_a == account)
        {
            let (kind, dest) = if row.group_id.is_empty() {
                (MessageKind::User, row.account_b.clone())
            } else {
                (MessageKind::Group, row.group_id.clone())
            };
            let sender = if row.direction == DIRECTION_SEND {
                account.to_string()
            } else {
                row.account_b.clone()
            };
            let (peer, group_id) = match kind {
                MessageKind::User => (dest.as_str(), ""),
                MessageKind::Group => ("", dest.as_str()),
            };
            let last_read = inner
                .reads
                .get(&(
                    app.to_string(),
                    account.to_string(),
                    peer.to_string(),
                    group_id.to_string(),
                ))
                .copied()
                .unwrap_or(0);
            let unread_inc =
                i32::from(row.direction == DIRECTION_RECV && row.message_id > last_read);
            let content = inner.contents.get(&row.message_id);
            let entry = latest
                .entry((kind, dest.clone()))
                .or_insert_with(|| InboxEntry {
                    dest: dest.clone(),
                    kind,
                    last_message_id: 0,
                    last_send_time: 0,
                    last_body: String::new(),
                    last_sender: String::new(),
                    last_msg_type: 0,
                    unread: 0,
                });
            entry.unread = entry.unread.saturating_add(unread_inc);
            let newer = row.send_time > entry.last_send_time
                || (row.send_time == entry.last_send_time
                    && row.message_id > entry.last_message_id);
            if newer {
                entry.last_message_id = row.message_id;
                entry.last_send_time = row.send_time;
                entry.last_sender = sender;
                if let Some(c) = content {
                    entry.last_body = c.body.clone();
                    entry.last_msg_type = c.msg_type;
                }
            }
        }
        let mut items: Vec<InboxEntry> = latest.into_values().collect();
        items.sort_by(|a, b| {
            b.last_send_time
                .cmp(&a.last_send_time)
                .then(b.last_message_id.cmp(&a.last_message_id))
        });
        items.truncate(cap);
        Ok(items)
    }

    async fn history(
        &self,
        app: &str,
        account: &str,
        dest: &str,
        kind: MessageKind,
        before_id: i64,
        limit: i32,
    ) -> Result<Vec<HistoryEntry>, StoreError> {
        let cap = clamp_page(limit, HISTORY_PAGE, HISTORY_MAX);
        let inner = self.read();
        let mut rows: Vec<HistoryEntry> = inner
            .indexes
            .iter()
            .filter(|r| {
                r.app == app
                    && r.account_a == account
                    && match kind {
                        MessageKind::User => r.group_id.is_empty() && r.account_b == dest,
                        MessageKind::Group => r.group_id == dest,
                    }
                    && (before_id <= 0 || r.message_id < before_id)
            })
            .filter_map(|r| {
                inner.contents.get(&r.message_id).map(|c| HistoryEntry {
                    message_id: r.message_id,
                    msg_type: c.msg_type,
                    body: c.body.clone(),
                    extra: c.extra.clone(),
                    sender: if r.direction == DIRECTION_SEND {
                        account.to_string()
                    } else {
                        r.account_b.clone()
                    },
                    send_time: r.send_time,
                    direction: r.direction,
                })
            })
            .collect();
        rows.sort_by_key(|b| std::cmp::Reverse(b.message_id));
        rows.truncate(cap);
        Ok(rows)
    }

    async fn mark_read(
        &self,
        app: &str,
        account: &str,
        dest: &str,
        kind: MessageKind,
        message_id: i64,
    ) -> Result<(), StoreError> {
        if dest.is_empty() || message_id <= 0 {
            return Ok(());
        }
        let (peer, group_id) = match kind {
            MessageKind::User => (dest, ""),
            MessageKind::Group => ("", dest),
        };
        let key = (
            app.to_string(),
            account.to_string(),
            peer.to_string(),
            group_id.to_string(),
        );
        let mut inner = self.write();
        let slot = inner.reads.entry(key).or_insert(0);
        if message_id > *slot {
            *slot = message_id;
        }
        Ok(())
    }
}

/// Postgres backends for Royal. Migrates once via `connect_pool`.
/// ACK stays crate-private; Redis/Memory is chosen here.
pub struct PgBackends {
    pub store: Arc<dyn MessageStore>,
    pub groups: Arc<dyn GroupDirectory>,
    pub users: Arc<dyn UserDirectory>,
    pub social: Arc<dyn crate::social::SocialDirectory>,
}

/// Open the message store. Empty URLs use Memory. Non-empty `database_url`
/// without `--features postgres` is an error (same contract as session Redis).
pub async fn open_message_store(
    database_url: Option<&str>,
    redis_url: Option<&str>,
    idgen: Arc<dyn IdGenerator>,
    pool: PoolConfig,
) -> Result<Arc<dyn MessageStore>, StoreError> {
    let ack = open_ack_index(redis_url).await?;
    match database_url {
        None | Some("") => Ok(Arc::new(MemoryMessageStore::with_ack(idgen, ack))),
        Some(url) => open_postgres_store(url, idgen, ack, pool).await,
    }
}

pub async fn open_pg_backends(
    database_url: &str,
    redis_url: Option<&str>,
    idgen: Arc<dyn IdGenerator>,
    pool: PoolConfig,
) -> Result<PgBackends, StoreError> {
    open_pg_backends_inner(database_url, redis_url, idgen, pool).await
}

#[derive(Clone, Copy)]
pub struct PoolConfig {
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 5,
            acquire_timeout: Duration::from_secs(3),
            idle_timeout: Duration::from_secs(60),
        }
    }
}

async fn open_ack_index(redis_url: Option<&str>) -> Result<Arc<dyn AckIndex>, StoreError> {
    match redis_url {
        None | Some("") => Ok(Arc::new(MemoryAckIndex::new())),
        Some(url) => open_redis_ack(url).await,
    }
}

#[cfg(feature = "redis")]
async fn open_redis_ack(url: &str) -> Result<Arc<dyn AckIndex>, StoreError> {
    let ack = redis_ack::RedisAckIndex::open(url).await?;
    Ok(Arc::new(ack))
}

#[cfg(not(feature = "redis"))]
async fn open_redis_ack(_url: &str) -> Result<Arc<dyn AckIndex>, StoreError> {
    Err(StoreError::Backend("rebuild with --features redis".into()))
}

#[cfg(feature = "postgres")]
async fn open_postgres_store(
    url: &str,
    idgen: Arc<dyn IdGenerator>,
    ack: Arc<dyn AckIndex>,
    pool: PoolConfig,
) -> Result<Arc<dyn MessageStore>, StoreError> {
    let store = PostgresMessageStore::connect(
        url,
        idgen,
        ack,
        PoolOpts {
            max_connections: pool.max_connections,
            acquire_timeout: pool.acquire_timeout,
            idle_timeout: pool.idle_timeout,
        },
    )
    .await?;
    Ok(Arc::new(store))
}

#[cfg(feature = "postgres")]
async fn open_pg_backends_inner(
    database_url: &str,
    redis_url: Option<&str>,
    idgen: Arc<dyn IdGenerator>,
    pool: PoolConfig,
) -> Result<PgBackends, StoreError> {
    let pg = connect_pool(
        database_url,
        PoolOpts {
            max_connections: pool.max_connections,
            acquire_timeout: pool.acquire_timeout,
            idle_timeout: pool.idle_timeout,
        },
    )
    .await?;
    let ack = open_ack_index(redis_url).await?;
    let store: Arc<dyn MessageStore> = Arc::new(PostgresMessageStore::from_pool(
        pg.clone(),
        idgen.clone(),
        ack,
    ));
    let groups: Arc<dyn GroupDirectory> = Arc::new(
        crate::directory::PostgresGroupDirectory::from_pool(pg.clone(), idgen),
    );
    let users: Arc<dyn UserDirectory> =
        Arc::new(crate::users::PostgresUserDirectory::from_pool(pg.clone()));
    let social: Arc<dyn crate::social::SocialDirectory> =
        Arc::new(crate::social::PostgresSocialDirectory::from_pool(pg));
    Ok(PgBackends {
        store,
        groups,
        users,
        social,
    })
}

#[cfg(not(feature = "postgres"))]
async fn open_pg_backends_inner(
    _database_url: &str,
    _redis_url: Option<&str>,
    _idgen: Arc<dyn IdGenerator>,
    _pool: PoolConfig,
) -> Result<PgBackends, StoreError> {
    Err(StoreError::Backend(
        "rebuild with --features postgres".into(),
    ))
}

#[cfg(not(feature = "postgres"))]
async fn open_postgres_store(
    _url: &str,
    _idgen: Arc<dyn IdGenerator>,
    _ack: Arc<dyn AckIndex>,
    _pool: PoolConfig,
) -> Result<Arc<dyn MessageStore>, StoreError> {
    Err(StoreError::Backend(
        "rebuild with --features postgres".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idgen::SequenceIdGen;

    fn sample(sender: &str, dest: &str, send_time: i64, body: &str) -> InsertMessage {
        InsertMessage {
            sender: sender.into(),
            dest: dest.into(),
            send_time,
            msg_type: 1,
            body: body.into(),
            extra: String::new(),
            client_id: String::new(),
        }
    }

    #[tokio::test]
    async fn insert_user_client_id_is_idempotent() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let mut req = sample("alice", "bob", 50, "hi");
        req.client_id = "c1".into();
        let a = store.insert_user("kim", &req).await.unwrap();
        let b = store.insert_user("kim", &req).await.unwrap();
        assert_eq!(a.message_id, b.message_id);
        assert_eq!(a.send_time, b.send_time);
        assert!(!a.duplicate);
        assert!(b.duplicate);
        assert_eq!(store.recorded().len(), 1);
        assert_eq!(a.fanout.body, "hi");
        assert_eq!(a.fanout.dest, "bob");
        assert!(a.fanout.recipients.contains(&"alice".into()));
        assert!(a.fanout.recipients.contains(&"bob".into()));
        let mut changed = sample("alice", "carol", 50, "CHANGED");
        changed.client_id = "c1".into();
        let c = store.insert_user("kim", &changed).await.unwrap();
        assert!(c.duplicate);
        assert_eq!(c.fanout.body, "hi");
        assert_eq!(c.fanout.dest, "bob");
        assert_eq!(c.message_id, a.message_id);
        assert!(c.fanout.recipients.contains(&"alice".into()));
        assert!(c.fanout.recipients.contains(&"bob".into()));
    }

    #[tokio::test]
    async fn insert_group_duplicate_keeps_first_recipients() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let mut req = sample("alice", "g1", 60, "hey");
        req.client_id = "c1".into();
        let first = store
            .insert_group("kim", &req, &["alice".into(), "bob".into(), "carol".into()])
            .await
            .unwrap();
        let second = store
            .insert_group("kim", &req, &["alice".into(), "dave".into()])
            .await
            .unwrap();
        assert!(second.duplicate);
        assert_eq!(first.fanout.recipients, second.fanout.recipients);
        assert_eq!(
            second.fanout.recipients,
            vec!["alice".to_string(), "bob".into(), "carol".into()]
        );
    }

    #[tokio::test]
    async fn self_chat_recipients_dedup_to_one() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let got = store
            .insert_user("kim", &sample("alice", "alice", 50, "note"))
            .await
            .unwrap();
        assert_eq!(got.fanout.recipients.len(), 1);
        assert_eq!(got.fanout.recipients[0], "alice");
        assert_eq!(got.fanout.kind, MessageKind::User);
        assert_eq!(got.fanout.dest, "alice");
    }

    #[tokio::test]
    async fn insert_user_writes_two_index_rows() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let got = store
            .insert_user("kim", &sample("alice", "bob", 50, "hi"))
            .await
            .unwrap();
        assert!(got.message_id > 10_000);
        let rec = store.recorded();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].kind, MessageKind::User);
        let idx = store.recorded_indexes();
        assert_eq!(idx.len(), 2);
        let send = idx.iter().find(|r| r.direction == DIRECTION_SEND).unwrap();
        let recv = idx.iter().find(|r| r.direction == DIRECTION_RECV).unwrap();
        assert_eq!(send.account_a, "alice");
        assert_eq!(send.account_b, "bob");
        assert_eq!(recv.account_a, "bob");
        assert_eq!(recv.account_b, "alice");
        assert!(send.group_id.is_empty());
    }

    #[tokio::test]
    async fn insert_group_fans_out_one_row_per_member() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        store
            .insert_group(
                "kim",
                &sample("alice", "g1", 60, "hey"),
                &["alice".into(), "bob".into(), "carol".into()],
            )
            .await
            .unwrap();
        let rec = store.recorded();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].kind, MessageKind::Group);
        let idx = store.recorded_indexes();
        assert_eq!(idx.len(), 3);
        let alice = idx.iter().find(|r| r.account_a == "alice").unwrap();
        assert_eq!(alice.direction, DIRECTION_SEND);
        let bob = idx.iter().find(|r| r.account_a == "bob").unwrap();
        assert_eq!(bob.direction, DIRECTION_RECV);
        assert_eq!(bob.account_b, "alice");
        assert_eq!(bob.group_id, "g1");
    }

    #[tokio::test]
    async fn empty_members_writes_content_without_index() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let first = store
            .insert_group("kim", &sample("alice", "gone", 1, "x"), &[])
            .await
            .unwrap();
        assert_eq!(store.recorded().len(), 1);
        assert!(store.recorded_indexes().is_empty());
        assert!(first.fanout.recipients.is_empty());
        assert_eq!(first.fanout.kind, MessageKind::User);
        assert!(first.fanout.dest.is_empty());
    }

    #[tokio::test]
    async fn empty_members_duplicate_is_empty_recipients_not_error() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let mut req = sample("alice", "gone", 1, "x");
        req.client_id = "c1".into();
        let first = store.insert_group("kim", &req, &[]).await.unwrap();
        let second = store.insert_group("kim", &req, &[]).await.unwrap();
        assert!(!first.duplicate);
        assert!(second.duplicate);
        assert!(second.fanout.recipients.is_empty());
        assert_eq!(second.fanout.kind, MessageKind::User);
        assert!(second.fanout.dest.is_empty());
        assert_eq!(first.message_id, second.message_id);
    }

    #[tokio::test]
    async fn ack_zero_is_noop_and_ack_hides_earlier_inbox() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let a = store
            .insert_user("kim", &sample("alice", "bob", now_unix_nano(), "one"))
            .await
            .unwrap();
        let b = store
            .insert_user("kim", &sample("alice", "bob", now_unix_nano(), "two"))
            .await
            .unwrap();
        store.ack("kim", "bob", 0).await.unwrap();
        let before = store.offline_index("kim", "bob", 0).await.unwrap();
        assert_eq!(before.len(), 2);

        store.ack("kim", "bob", b.message_id).await.unwrap();
        let after = store.offline_index("kim", "bob", 0).await.unwrap();
        assert!(after.iter().all(|r| r.message_id != a.message_id));
        assert!(after.iter().all(|r| r.message_id != b.message_id));
    }

    #[tokio::test]
    async fn cold_start_clips_to_fifteen_day_window() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let old = now_unix_nano().saturating_sub(EXPIRES_NANOS + DAY_NANOS);
        store
            .insert_user("kim", &sample("alice", "bob", old, "stale"))
            .await
            .unwrap();
        let recent = now_unix_nano();
        let fresh = store
            .insert_user("kim", &sample("alice", "bob", recent, "fresh"))
            .await
            .unwrap();
        let idx = store.offline_index("kim", "bob", 0).await.unwrap();
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].message_id, fresh.message_id);
    }

    #[tokio::test]
    async fn offline_content_preserves_request_order_and_skips_missing() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let a = store
            .insert_user("kim", &sample("alice", "bob", 1, "a"))
            .await
            .unwrap();
        let b = store
            .insert_user("kim", &sample("alice", "bob", 2, "b"))
            .await
            .unwrap();
        let rows = store
            .offline_content("kim", "bob", &[b.message_id, 0, a.message_id])
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].body, "b");
        assert_eq!(rows[1].body, "a");
    }

    #[tokio::test]
    async fn offline_content_skips_ids_without_index_for_account() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let a = store
            .insert_user("kim", &sample("alice", "bob", 1, "secret"))
            .await
            .unwrap();
        let rows = store
            .offline_content("kim", "carol", &[a.message_id])
            .await
            .unwrap();
        assert!(rows.is_empty());
        let other_app = store
            .offline_content("kim-gray", "bob", &[a.message_id])
            .await
            .unwrap();
        assert!(other_app.is_empty());
        let owner = store
            .offline_content("kim", "alice", &[a.message_id])
            .await
            .unwrap();
        assert_eq!(owner.len(), 1);
        assert_eq!(owner[0].body, "secret");
    }

    #[tokio::test]
    async fn offline_content_self_chat_hits_either_index_row() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        let a = store
            .insert_user("kim", &sample("alice", "alice", 1, "note"))
            .await
            .unwrap();
        let rows = store
            .offline_content("kim", "alice", &[a.message_id])
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].body, "note");
    }

    #[tokio::test]
    async fn inbox_history_and_read_cursor() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        store
            .insert_user("kim", &sample("alice", "bob", 10, "hi"))
            .await
            .unwrap();
        let second = store
            .insert_user("kim", &sample("bob", "alice", 20, "yo"))
            .await
            .unwrap();
        store
            .insert_group(
                "kim",
                &sample("alice", "g1", 30, "hey"),
                &["alice".into(), "bob".into()],
            )
            .await
            .unwrap();

        let inbox = store.inbox("kim", "alice", 10).await.unwrap();
        assert_eq!(inbox.len(), 2);
        assert_eq!(inbox[0].dest, "g1");
        assert_eq!(inbox[0].kind, MessageKind::Group);
        assert_eq!(inbox[1].dest, "bob");
        assert_eq!(inbox[1].unread, 1);

        store
            .mark_read("kim", "alice", "bob", MessageKind::User, second.message_id)
            .await
            .unwrap();
        let inbox = store.inbox("kim", "alice", 10).await.unwrap();
        let dm = inbox.iter().find(|e| e.dest == "bob").unwrap();
        assert_eq!(dm.unread, 0);

        let hist = store
            .history("kim", "alice", "bob", MessageKind::User, 0, 10)
            .await
            .unwrap();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].body, "yo");
        assert_eq!(hist[0].sender, "bob");
        let page = store
            .history(
                "kim",
                "alice",
                "bob",
                MessageKind::User,
                hist[0].message_id,
                10,
            )
            .await
            .unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].body, "hi");
    }

    #[tokio::test]
    async fn open_empty_is_memory() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = open_message_store(None, None, idgen, PoolConfig::default())
            .await
            .unwrap();
        store
            .insert_user("kim", &sample("a", "b", 1, "x"))
            .await
            .unwrap();
    }

    #[cfg(not(feature = "postgres"))]
    #[tokio::test]
    async fn nonempty_database_url_requires_feature() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        match open_message_store(
            Some("postgres://127.0.0.1/kim"),
            None,
            idgen,
            PoolConfig::default(),
        )
        .await
        {
            Err(e) => assert_eq!(e.to_string(), "rebuild with --features postgres"),
            Ok(_) => panic!("expected feature error"),
        }
    }

    #[cfg(not(feature = "redis"))]
    #[tokio::test]
    async fn nonempty_redis_url_requires_feature() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        match open_message_store(
            None,
            Some("redis://127.0.0.1:6379"),
            idgen,
            PoolConfig::default(),
        )
        .await
        {
            Err(e) => assert_eq!(e.to_string(), "rebuild with --features redis"),
            Ok(_) => panic!("expected feature error"),
        }
    }
}
