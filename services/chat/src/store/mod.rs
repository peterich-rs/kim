//! Message persistence: write-fanout inbox rows + content + a per-account read index.

use std::collections::HashMap;
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
pub const ACK_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
pub(crate) const DAY_NANOS: i64 = 24 * 60 * 60 * 1_000_000_000;
const EXPIRES_NANOS: i64 = 15 * DAY_NANOS;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("idgen: {0}")]
    Id(#[from] IdError),
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
}

pub struct InsertResult {
    pub message_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKind {
    User,
    Group,
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
        message_ids: &[i64],
    ) -> Result<Vec<MessageContentRow>, StoreError>;
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

    fn insert(
        &self,
        app: &str,
        kind: MessageKind,
        req: &InsertMessage,
        members: &[String],
    ) -> Result<InsertResult, StoreError> {
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
        inner.contents.insert(message_id, content);
        inner.indexes.extend(indexes);
        Ok(InsertResult { message_id })
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
        _app: &str,
        message_ids: &[i64],
    ) -> Result<Vec<MessageContentRow>, StoreError> {
        let inner = self.read();
        Ok(message_ids
            .iter()
            .filter_map(|id| {
                inner.contents.get(id).map(|c| MessageContentRow {
                    message_id: c.message_id,
                    msg_type: c.msg_type,
                    body: c.body.clone(),
                    extra: c.extra.clone(),
                })
            })
            .collect())
    }
}

/// Postgres backends for Royal. Migrates once via `connect_pool`.
/// ACK stays crate-private; Redis/Memory is chosen here.
pub struct PgBackends {
    pub store: Arc<dyn MessageStore>,
    pub groups: Arc<dyn GroupDirectory>,
    pub users: Arc<dyn UserDirectory>,
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
        Arc::new(crate::users::PostgresUserDirectory::from_pool(pg));
    Ok(PgBackends {
        store,
        groups,
        users,
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
        }
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
        store
            .insert_group("kim", &sample("alice", "gone", 1, "x"), &[])
            .await
            .unwrap();
        assert_eq!(store.recorded().len(), 1);
        assert!(store.recorded_indexes().is_empty());
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
            .offline_content("kim", &[b.message_id, 0, a.message_id])
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].body, "b");
        assert_eq!(rows[1].body, "a");
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
