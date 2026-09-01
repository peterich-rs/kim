use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};

use crate::idgen::IdGenerator;

use super::{
    clamp_page, clamp_start, fanout_from_index_rows, fanout_from_write, now_unix_nano, AckIndex,
    Fanout, HistoryEntry, InboxEntry, InsertMessage, InsertResult, MessageContentRow,
    MessageIndexRow, MessageKind, MessageStore, StoreError, DAY_NANOS, DIRECTION_RECV,
    DIRECTION_SEND, HISTORY_MAX, HISTORY_PAGE, INBOX_MAX, INBOX_PAGE, OFFLINE_SYNC_INDEX_COUNT,
};

#[derive(Clone, Copy)]
pub struct PoolOpts {
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
}

pub async fn connect_pool(url: &str, opts: PoolOpts) -> Result<PgPool, StoreError> {
    let pool = PgPoolOptions::new()
        .max_connections(opts.max_connections)
        .acquire_timeout(opts.acquire_timeout)
        .idle_timeout(opts.idle_timeout)
        .connect(url)
        .await
        .map_err(pg_err)?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| StoreError::Backend(e.to_string()))?;
    Ok(pool)
}

pub struct PostgresMessageStore {
    pool: PgPool,
    idgen: Arc<dyn IdGenerator>,
    ack: Arc<dyn AckIndex>,
}

impl PostgresMessageStore {
    pub(crate) fn from_pool(
        pool: PgPool,
        idgen: Arc<dyn IdGenerator>,
        ack: Arc<dyn AckIndex>,
    ) -> Self {
        Self { pool, idgen, ack }
    }

    pub(crate) async fn connect(
        url: &str,
        idgen: Arc<dyn IdGenerator>,
        ack: Arc<dyn AckIndex>,
        opts: PoolOpts,
    ) -> Result<Self, StoreError> {
        let pool = connect_pool(url, opts).await?;
        Ok(Self::from_pool(pool, idgen, ack))
    }

    async fn insert_fanout(
        &self,
        app: &str,
        req: &InsertMessage,
        members: Option<&[String]>,
    ) -> Result<InsertResult, StoreError> {
        if !req.client_id.is_empty() {
            let existing: Option<(i64, i64)> = sqlx::query_as(
                "SELECT message_id, send_time FROM message_idempotency
                 WHERE app = $1 AND sender = $2 AND client_id = $3",
            )
            .bind(app)
            .bind(&req.sender)
            .bind(&req.client_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(pg_err)?;
            if let Some((message_id, send_time)) = existing {
                return Ok(InsertResult {
                    message_id,
                    send_time,
                    duplicate: true,
                    fanout: load_fanout(&self.pool, app, message_id).await?,
                });
            }
        }
        let message_id = self.idgen.next_id()?;
        let msg_type = i16::try_from(req.msg_type)
            .map_err(|_| StoreError::Backend("msg_type does not fit smallint".into()))?;
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await.map_err(pg_err)?;
        sqlx::query(
            "INSERT INTO message_content (id, app, msg_type, body, extra, send_time)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(message_id)
        .bind(app)
        .bind(msg_type)
        .bind(&req.body)
        .bind(&req.extra)
        .bind(req.send_time)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;

        let rows: Vec<(String, String, i16, String)> = match members {
            None => vec![
                (
                    req.sender.clone(),
                    req.dest.clone(),
                    DIRECTION_SEND as i16,
                    String::new(),
                ),
                (
                    req.dest.clone(),
                    req.sender.clone(),
                    DIRECTION_RECV as i16,
                    String::new(),
                ),
            ],
            Some(list) => list
                .iter()
                .map(|m| {
                    let dir = if m == &req.sender {
                        DIRECTION_SEND
                    } else {
                        DIRECTION_RECV
                    };
                    (m.clone(), req.sender.clone(), dir as i16, req.dest.clone())
                })
                .collect(),
        };
        for (account_a, account_b, direction, group_id) in rows {
            let idx_id = self.idgen.next_id()?;
            sqlx::query(
                "INSERT INTO message_index
                    (id, app, account_a, account_b, direction, message_id, group_id, send_time)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(idx_id)
            .bind(app)
            .bind(&account_a)
            .bind(&account_b)
            .bind(direction)
            .bind(message_id)
            .bind(&group_id)
            .bind(req.send_time)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
        }
        if !req.client_id.is_empty() {
            let claimed = sqlx::query(
                "INSERT INTO message_idempotency (app, sender, client_id, message_id, send_time)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (app, sender, client_id) DO NOTHING",
            )
            .bind(app)
            .bind(&req.sender)
            .bind(&req.client_id)
            .bind(message_id)
            .bind(req.send_time)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
            if claimed.rows_affected() == 0 {
                tx.rollback().await.map_err(pg_err)?;
                let existing: Option<(i64, i64)> = sqlx::query_as(
                    "SELECT message_id, send_time FROM message_idempotency
                     WHERE app = $1 AND sender = $2 AND client_id = $3",
                )
                .bind(app)
                .bind(&req.sender)
                .bind(&req.client_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(pg_err)?;
                if let Some((id, send_time)) = existing {
                    return Ok(InsertResult {
                        message_id: id,
                        send_time,
                        duplicate: true,
                        fanout: load_fanout(&self.pool, app, id).await?,
                    });
                }
                return Err(StoreError::Backend(
                    "idempotency conflict without row".into(),
                ));
            }
        }
        tx.commit().await.map_err(pg_err)?;
        let kind = if members.is_some() {
            MessageKind::Group
        } else {
            MessageKind::User
        };
        Ok(InsertResult {
            message_id,
            send_time: req.send_time,
            duplicate: false,
            fanout: fanout_from_write(kind, req, members.unwrap_or(&[])),
        })
    }

    async fn sent_time(&self, account: &str, message_id: i64) -> Result<i64, StoreError> {
        let mut id = message_id;
        if id == 0 {
            id = self.ack.get(account).await?;
        }
        let now = now_unix_nano();
        let mut start = 0;
        if id > 0 {
            let row: Option<(i64,)> =
                sqlx::query_as("SELECT send_time FROM message_content WHERE id = $1")
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(pg_err)?;
            start = row
                .map(|r| r.0)
                .unwrap_or_else(|| now.saturating_sub(DAY_NANOS));
        }
        Ok(clamp_start(start, now))
    }
}

fn pg_err(e: sqlx::Error) -> StoreError {
    StoreError::Backend(e.to_string())
}

type FanoutSqlRow = (
    i16,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<i16>,
    Option<String>,
);

async fn load_fanout(pool: &PgPool, app: &str, message_id: i64) -> Result<Fanout, StoreError> {
    let rows: Vec<FanoutSqlRow> = sqlx::query_as(
        "SELECT c.msg_type, c.body, c.extra,
                i.account_a, i.account_b, i.direction, i.group_id
         FROM message_content c
         LEFT JOIN message_index i
           ON i.message_id = c.id AND i.app = c.app
         WHERE c.id = $1 AND c.app = $2
         ORDER BY i.id",
    )
    .bind(message_id)
    .bind(app)
    .fetch_all(pool)
    .await
    .map_err(pg_err)?;

    let Some(first) = rows.first() else {
        return Err(StoreError::Backend("fanout missing".into()));
    };
    let msg_type = i32::from(first.0);
    let body = first.1.clone();
    let extra = first.2.clone();
    let index_rows: Vec<(String, String, i32, String)> = rows
        .into_iter()
        .filter_map(|(_, _, _, a, b, dir, gid)| {
            Some((a?, b?, i32::from(dir?), gid.unwrap_or_default()))
        })
        .collect();
    Ok(fanout_from_index_rows(msg_type, body, extra, &index_rows))
}

#[async_trait]
impl MessageStore for PostgresMessageStore {
    async fn insert_user(
        &self,
        app: &str,
        req: &InsertMessage,
    ) -> Result<InsertResult, StoreError> {
        self.insert_fanout(app, req, None).await
    }

    async fn insert_group(
        &self,
        app: &str,
        req: &InsertMessage,
        members: &[String],
    ) -> Result<InsertResult, StoreError> {
        self.insert_fanout(app, req, Some(members)).await
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
        let limit = i64::try_from(OFFLINE_SYNC_INDEX_COUNT).unwrap_or(i64::MAX);
        let rows: Vec<(i64, i16, i64, String, String)> = sqlx::query_as(
            "SELECT message_id, direction, send_time, account_b, group_id
             FROM message_index
             WHERE app = $1 AND account_a = $2 AND send_time > $3 AND direction = $4
             ORDER BY send_time ASC
             LIMIT $5",
        )
        .bind(app)
        .bind(account)
        .bind(start)
        .bind(DIRECTION_RECV as i16)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        if message_id > 0 {
            self.ack.set(account, message_id).await?;
        }
        Ok(rows
            .into_iter()
            .map(
                |(message_id, direction, send_time, account_b, group)| MessageIndexRow {
                    message_id,
                    direction: i32::from(direction),
                    send_time,
                    account_b,
                    group,
                },
            )
            .collect())
    }

    async fn offline_content(
        &self,
        app: &str,
        account: &str,
        message_ids: &[i64],
    ) -> Result<Vec<MessageContentRow>, StoreError> {
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<(i64, i16, String, String)> = sqlx::query_as(
            "SELECT c.id, c.msg_type, c.body, c.extra
             FROM message_content c
             WHERE c.id = ANY($1)
               AND c.app = $2
               AND EXISTS (
                 SELECT 1 FROM message_index i
                 WHERE i.message_id = c.id
                   AND i.app = $2
                   AND i.account_a = $3
               )",
        )
        .bind(message_ids)
        .bind(app)
        .bind(account)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        let mut by_id = HashMap::with_capacity(rows.len());
        for (id, msg_type, body, extra) in rows {
            by_id.insert(
                id,
                MessageContentRow {
                    message_id: id,
                    msg_type: i32::from(msg_type),
                    body,
                    extra,
                },
            );
        }
        Ok(message_ids
            .iter()
            .filter_map(|id| by_id.remove(id))
            .collect())
    }

    async fn inbox(
        &self,
        app: &str,
        account: &str,
        limit: i32,
    ) -> Result<Vec<InboxEntry>, StoreError> {
        let cap = i64::try_from(clamp_page(limit, INBOX_PAGE, INBOX_MAX)).unwrap_or(50);
        let rows: Vec<(String, i32, i64, i64, i32)> = sqlx::query_as(
            "SELECT
                CASE WHEN i.group_id = '' THEN i.account_b ELSE i.group_id END AS dest,
                CASE WHEN i.group_id = '' THEN 0 ELSE 1 END AS kind,
                MAX(i.message_id) AS last_id,
                MAX(i.send_time) AS last_at,
                COUNT(*) FILTER (
                    WHERE i.direction = $3 AND i.message_id > COALESCE(r.last_read_id, 0)
                )::int AS unread
             FROM message_index i
             LEFT JOIN conversation_reads r
               ON r.app = i.app AND r.account = i.account_a
              AND (
                    (i.group_id = '' AND r.peer = i.account_b AND r.group_id = '')
                    OR (i.group_id <> '' AND r.peer = '' AND r.group_id = i.group_id)
                  )
             WHERE i.app = $1 AND i.account_a = $2
             GROUP BY 1, 2, r.last_read_id
             ORDER BY last_at DESC, last_id DESC
             LIMIT $4",
        )
        .bind(app)
        .bind(account)
        .bind(DIRECTION_RECV as i16)
        .bind(cap)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<i64> = rows.iter().map(|r| r.2).collect();
        let contents: Vec<(i64, i16, String, String)> = sqlx::query_as(
            "SELECT id, msg_type, body, extra FROM message_content WHERE id = ANY($1)",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        let mut by_id = HashMap::with_capacity(contents.len());
        for (id, msg_type, body, extra) in contents {
            by_id.insert(id, (i32::from(msg_type), body, extra));
        }
        let senders: Vec<(i64, i16, String)> = sqlx::query_as(
            "SELECT message_id, direction, account_b FROM message_index
             WHERE app = $1 AND account_a = $2 AND message_id = ANY($3)",
        )
        .bind(app)
        .bind(account)
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        let mut sender_by_id = HashMap::new();
        for (id, direction, account_b) in senders {
            let sender = if i32::from(direction) == DIRECTION_SEND {
                account.to_string()
            } else {
                account_b
            };
            sender_by_id.entry(id).or_insert(sender);
        }
        Ok(rows
            .into_iter()
            .map(|(dest, kind, last_id, last_at, unread)| {
                let (msg_type, body) = by_id
                    .get(&last_id)
                    .map(|(t, b, _)| (*t, b.clone()))
                    .unwrap_or((0, String::new()));
                InboxEntry {
                    dest,
                    kind: if kind == 1 {
                        MessageKind::Group
                    } else {
                        MessageKind::User
                    },
                    last_message_id: last_id,
                    last_send_time: last_at,
                    last_body: body,
                    last_sender: sender_by_id.remove(&last_id).unwrap_or_default(),
                    last_msg_type: msg_type,
                    unread,
                }
            })
            .collect())
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
        let cap = i64::try_from(clamp_page(limit, HISTORY_PAGE, HISTORY_MAX)).unwrap_or(50);
        let is_group = kind == MessageKind::Group;
        let rows: Vec<(i64, i16, i64, String, i16, String, String)> = sqlx::query_as(
            "SELECT i.message_id, i.direction, i.send_time, i.account_b,
                    c.msg_type, c.body, c.extra
             FROM message_index i
             JOIN message_content c ON c.id = i.message_id
             WHERE i.app = $1 AND i.account_a = $2
               AND (
                    ($3 AND i.group_id = $4)
                    OR (NOT $3 AND i.group_id = '' AND i.account_b = $4)
               )
               AND ($5 <= 0 OR i.message_id < $5)
             ORDER BY i.message_id DESC
             LIMIT $6",
        )
        .bind(app)
        .bind(account)
        .bind(is_group)
        .bind(dest)
        .bind(before_id)
        .bind(cap)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(rows
            .into_iter()
            .map(
                |(message_id, direction, send_time, account_b, msg_type, body, extra)| {
                    let direction = i32::from(direction);
                    HistoryEntry {
                        message_id,
                        msg_type: i32::from(msg_type),
                        body,
                        extra,
                        sender: if direction == DIRECTION_SEND {
                            account.to_string()
                        } else {
                            account_b
                        },
                        send_time,
                        direction,
                    }
                },
            )
            .collect())
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
        sqlx::query(
            "INSERT INTO conversation_reads (app, account, peer, group_id, last_read_id, updated_at)
             VALUES ($1, $2, $3, $4, $5, now())
             ON CONFLICT (app, account, peer, group_id)
             DO UPDATE SET
                last_read_id = GREATEST(conversation_reads.last_read_id, EXCLUDED.last_read_id),
                updated_at = now()",
        )
        .bind(app)
        .bind(account)
        .bind(peer)
        .bind(group_id)
        .bind(message_id)
        .execute(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idgen::SequenceIdGen;
    use crate::store::MemoryAckIndex;

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

    async fn connect() -> Option<PostgresMessageStore> {
        let url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.is_empty())?;
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let ack: Arc<dyn AckIndex> = Arc::new(MemoryAckIndex::new());
        match PostgresMessageStore::connect(
            &url,
            idgen,
            ack,
            PoolOpts {
                max_connections: 2,
                acquire_timeout: Duration::from_secs(3),
                idle_timeout: Duration::from_secs(60),
            },
        )
        .await
        {
            Ok(s) => Some(s),
            Err(err) => {
                eprintln!("skip postgres: {err}");
                None
            }
        }
    }

    #[tokio::test]
    async fn postgres_write_fanout_and_offline() {
        let Some(store) = connect().await else {
            return;
        };
        let app = format!("kim_pg_{}", now_unix_nano());
        let now = now_unix_nano();
        let inserted = store
            .insert_user(&app, &sample("alice", "bob", now, "hello"))
            .await
            .unwrap();
        let idx = store.offline_index(&app, "bob", 0).await.unwrap();
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].message_id, inserted.message_id);
        assert_eq!(idx[0].account_b, "alice");
        let body = store
            .offline_content(&app, "bob", &[inserted.message_id])
            .await
            .unwrap();
        assert_eq!(body[0].body, "hello");
        store.ack(&app, "bob", inserted.message_id).await.unwrap();
        let after = store.offline_index(&app, "bob", 0).await.unwrap();
        assert!(after.is_empty());
    }

    #[tokio::test]
    async fn postgres_duplicate_reloads_fanout() {
        let Some(store) = connect().await else {
            return;
        };
        let app = format!("kim_pg_dup_{}", now_unix_nano());
        let now = now_unix_nano();
        let mut first = sample("alice", "bob", now, "hello");
        first.client_id = "c1".into();
        let a = store.insert_user(&app, &first).await.unwrap();
        assert!(!a.duplicate);
        assert_eq!(a.fanout.body, "hello");
        assert_eq!(a.fanout.dest, "bob");
        let mut changed = sample("alice", "carol", now, "CHANGED");
        changed.client_id = "c1".into();
        let b = store.insert_user(&app, &changed).await.unwrap();
        assert!(b.duplicate);
        assert_eq!(b.message_id, a.message_id);
        assert_eq!(b.fanout.body, "hello");
        assert_eq!(b.fanout.dest, "bob");
        assert!(b.fanout.recipients.contains(&"alice".into()));
        assert!(b.fanout.recipients.contains(&"bob".into()));
    }

    #[tokio::test]
    async fn postgres_concurrent_same_client_id_returns_one_winner() {
        let Some(store) = connect().await else {
            return;
        };
        let store = Arc::new(store);
        let app = format!("kim_pg_race_{}", now_unix_nano());
        let now = now_unix_nano();
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let a = {
            let store = store.clone();
            let app = app.clone();
            let barrier = barrier.clone();
            tokio::spawn(async move {
                let mut req = sample("alice", "bob", now, "A");
                req.client_id = "c1".into();
                barrier.wait().await;
                store.insert_user(&app, &req).await
            })
        };
        let b = {
            let store = store.clone();
            let app = app.clone();
            tokio::spawn(async move {
                let mut req = sample("alice", "bob", now, "B");
                req.client_id = "c1".into();
                barrier.wait().await;
                store.insert_user(&app, &req).await
            })
        };
        let first = a.await.unwrap().unwrap();
        let second = b.await.unwrap().unwrap();
        let (winner, loser) = if first.duplicate {
            (second, first)
        } else {
            (first, second)
        };
        assert!(!winner.duplicate);
        assert!(loser.duplicate);
        assert_eq!(winner.message_id, loser.message_id);
        assert!(winner.fanout.body == "A" || winner.fanout.body == "B");
        assert_eq!(loser.fanout.body, winner.fanout.body);
    }
}
