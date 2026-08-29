use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};

use crate::idgen::IdGenerator;

use super::{
    clamp_start, now_unix_nano, AckIndex, InsertMessage, InsertResult, MessageContentRow,
    MessageIndexRow, MessageStore, StoreError, DAY_NANOS, DIRECTION_RECV, DIRECTION_SEND,
    OFFLINE_SYNC_INDEX_COUNT,
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
        tx.commit().await.map_err(pg_err)?;
        Ok(InsertResult { message_id })
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
        _app: &str,
        message_ids: &[i64],
    ) -> Result<Vec<MessageContentRow>, StoreError> {
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<(i64, i16, String, String)> = sqlx::query_as(
            "SELECT id, msg_type, body, extra FROM message_content WHERE id = ANY($1)",
        )
        .bind(message_ids)
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
            .offline_content(&app, &[inserted.message_id])
            .await
            .unwrap();
        assert_eq!(body[0].body, "hello");
        store.ack(&app, "bob", inserted.message_id).await.unwrap();
        let after = store.offline_index(&app, "bob", 0).await.unwrap();
        assert!(after.is_empty());
    }
}
