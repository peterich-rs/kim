use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use kim_router::{SessionError, SessionStorage};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use tokio::time::timeout;

use crate::idgen::IdGenerator;

use super::{
    clamp_page, clamp_start, fanout_from_index_rows, fanout_from_write, now_unix_nano,
    recv_accounts, AckIndex, DeliveryTarget, Fanout, HistoryEntry, InboxEntry, InsertMessage,
    InsertResult, MessageContentRow, MessageIndexRow, MessageKind, MessageStore, StoreError,
    DAY_NANOS, DIRECTION_RECV, DIRECTION_SEND, EXPIRES_NANOS, HISTORY_MAX, HISTORY_PAGE, INBOX_MAX,
    INBOX_PAGE, LIST_LOCATIONS_BUDGET, MESSAGE_MAX_COUNT_PER_PAGE, OFFLINE_SYNC_INDEX_COUNT,
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
    sessions: Option<Arc<dyn SessionStorage>>,
    pending_receipt: bool,
}

impl PostgresMessageStore {
    pub(crate) fn from_pool(
        pool: PgPool,
        idgen: Arc<dyn IdGenerator>,
        ack: Arc<dyn AckIndex>,
        sessions: Option<Arc<dyn SessionStorage>>,
        pending_receipt: bool,
    ) -> Self {
        Self {
            pool,
            idgen,
            ack,
            sessions,
            pending_receipt,
        }
    }

    pub(crate) async fn connect(
        url: &str,
        idgen: Arc<dyn IdGenerator>,
        ack: Arc<dyn AckIndex>,
        opts: PoolOpts,
    ) -> Result<Self, StoreError> {
        let pool = connect_pool(url, opts).await?;
        Ok(Self::from_pool(
            pool,
            idgen,
            ack,
            None,
            super::pending_receipt_enabled(),
        ))
    }

    async fn insert_fanout(
        &self,
        app: &str,
        req: &InsertMessage,
        members: Option<&[String]>,
    ) -> Result<InsertResult, StoreError> {
        if !self.pending_receipt {
            return self.insert_fanout_legacy(app, req, members).await;
        }
        for _ in 0..32 {
            match self.insert_fanout_pending(app, req, members).await? {
                FanoutAttempt::Done(r) => return Ok(r),
                FanoutAttempt::Retry => continue,
            }
        }
        Err(StoreError::Backend("idempotency retry exhausted".into()))
    }

    async fn insert_fanout_legacy(
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

    async fn insert_fanout_pending(
        &self,
        app: &str,
        req: &InsertMessage,
        members: Option<&[String]>,
    ) -> Result<FanoutAttempt, StoreError> {
        let message_id = self.idgen.next_id()?;
        let msg_type = i16::try_from(req.msg_type)
            .map_err(|_| StoreError::Backend("msg_type does not fit smallint".into()))?;
        let recv = recv_accounts(req, members);
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await.map_err(pg_err)?;
        lock_recv_accounts(&mut tx, app, &recv).await?;

        let existing = if req.client_id.is_empty() {
            None
        } else {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT message_id, send_time FROM message_idempotency
                 WHERE app = $1 AND sender = $2 AND client_id = $3",
            )
            .bind(app)
            .bind(&req.sender)
            .bind(&req.client_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(pg_err)?
        };

        let targets = match self.resolve_targets(&recv, req).await {
            Ok(t) => t,
            Err(err) => {
                tx.rollback().await.map_err(pg_err)?;
                return Err(err);
            }
        };

        if let Some((id, send_time)) = existing {
            insert_receipts(&mut tx, app, id, &targets).await?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(FanoutAttempt::Done(InsertResult {
                message_id: id,
                send_time,
                duplicate: true,
                fanout: load_fanout(&self.pool, app, id).await?,
            }));
        }

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
                return Ok(FanoutAttempt::Retry);
            }
        }

        insert_receipts(&mut tx, app, message_id, &targets).await?;
        tx.commit().await.map_err(pg_err)?;
        let kind = if members.is_some() {
            MessageKind::Group
        } else {
            MessageKind::User
        };
        Ok(FanoutAttempt::Done(InsertResult {
            message_id,
            send_time: req.send_time,
            duplicate: false,
            fanout: fanout_from_write(kind, req, members.unwrap_or(&[])),
        }))
    }

    async fn resolve_targets(
        &self,
        recv: &[String],
        req: &InsertMessage,
    ) -> Result<Vec<DeliveryTarget>, StoreError> {
        let Some(sessions) = self.sessions.as_ref() else {
            let recv_set: std::collections::HashSet<&str> =
                recv.iter().map(String::as_str).collect();
            return Ok(req
                .online_targets
                .iter()
                .filter(|t| !t.target_id.is_empty() && recv_set.contains(t.account.as_str()))
                .cloned()
                .collect());
        };
        let sessions = sessions.clone();
        let recv = recv.to_vec();
        match timeout(LIST_LOCATIONS_BUDGET, async move {
            let mut out = Vec::new();
            for account in &recv {
                let locs = match sessions.get_locations(std::slice::from_ref(account)).await {
                    Ok(v) => v,
                    Err(SessionError::NotFound) => continue,
                    Err(err) => return Err(StoreError::Backend(err.to_string())),
                };
                for loc in locs {
                    let jti = if loc.jti.is_empty() {
                        match sessions.get(&loc.channel_id).await {
                            Ok(s) => s.jti,
                            Err(_) => String::new(),
                        }
                    } else {
                        loc.jti
                    };
                    if !jti.is_empty() {
                        out.push(DeliveryTarget {
                            account: account.clone(),
                            target_id: jti,
                        });
                    }
                }
            }
            Ok(out)
        })
        .await
        {
            Ok(r) => r,
            Err(_) => {
                tracing::error!("insert list_locations timed out");
                Err(StoreError::Backend("list_locations timeout".into()))
            }
        }
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

enum FanoutAttempt {
    Done(InsertResult),
    Retry,
}

async fn lock_recv_accounts(
    tx: &mut Transaction<'_, Postgres>,
    app: &str,
    recv: &[String],
) -> Result<(), StoreError> {
    let mut accounts = recv.to_vec();
    accounts.sort();
    accounts.dedup();
    for account in accounts {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1::text), hashtext($2::text))")
            .bind(app)
            .bind(&account)
            .execute(&mut **tx)
            .await
            .map_err(pg_err)?;
    }
    Ok(())
}

async fn insert_receipts(
    tx: &mut Transaction<'_, Postgres>,
    app: &str,
    message_id: i64,
    targets: &[DeliveryTarget],
) -> Result<(), StoreError> {
    for t in targets {
        if t.target_id.is_empty() {
            continue;
        }
        sqlx::query(
            "INSERT INTO pending_delivery (app, account, target_id, message_id, expires_at)
             VALUES ($1, $2, $3, $4, now() + interval '15 days')
             ON CONFLICT DO NOTHING",
        )
        .bind(app)
        .bind(&t.account)
        .bind(&t.target_id)
        .bind(message_id)
        .execute(&mut **tx)
        .await
        .map_err(pg_err)?;
    }
    Ok(())
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

    async fn ack(
        &self,
        app: &str,
        account: &str,
        target_id: &str,
        message_ids: &[i64],
    ) -> Result<(), StoreError> {
        if !self.pending_receipt || target_id.is_empty() {
            let id = message_ids.first().copied().unwrap_or(0);
            if id == 0 {
                return Ok(());
            }
            return self.ack.set(account, id).await;
        }
        if target_id.is_empty() || message_ids.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "UPDATE pending_delivery
                SET acked_at = now()
              WHERE app = $1 AND account = $2 AND target_id = $3
                AND message_id = ANY($4::bigint[])
                AND acked_at IS NULL",
        )
        .bind(app)
        .bind(account)
        .bind(target_id)
        .bind(message_ids)
        .execute(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(())
    }

    async fn offline_index(
        &self,
        app: &str,
        account: &str,
        target_id: &str,
        message_id: i64,
        resume: bool,
    ) -> Result<(Vec<MessageIndexRow>, bool), StoreError> {
        if !self.pending_receipt || target_id.is_empty() {
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
            let out = rows
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
                .collect();
            return Ok((out, false));
        }
        if target_id.is_empty() {
            return Ok((Vec::new(), false));
        }
        if !resume && message_id != 0 {
            return Ok((Vec::new(), false));
        }
        let fetch = if resume {
            i64::try_from(MESSAGE_MAX_COUNT_PER_PAGE + 1).unwrap_or(201)
        } else {
            i64::try_from(MESSAGE_MAX_COUNT_PER_PAGE).unwrap_or(200)
        };
        let rows: Vec<(i64, i16, i64, String, String)> = sqlx::query_as(
            "SELECT i.message_id, i.direction, i.send_time, i.account_b, i.group_id
               FROM pending_delivery pd
               JOIN message_index i
                 ON i.app = pd.app AND i.account_a = pd.account AND i.message_id = pd.message_id
              WHERE pd.app = $1 AND pd.account = $2 AND pd.target_id = $3
                AND pd.acked_at IS NULL AND pd.expires_at > now()
                AND i.direction = $4
              ORDER BY pd.created_at ASC, pd.message_id ASC
              LIMIT $5",
        )
        .bind(app)
        .bind(account)
        .bind(target_id)
        .bind(DIRECTION_RECV as i16)
        .bind(fetch)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        let has_more = resume && rows.len() > MESSAGE_MAX_COUNT_PER_PAGE;
        let rows: Vec<MessageIndexRow> = rows
            .into_iter()
            .take(MESSAGE_MAX_COUNT_PER_PAGE)
            .map(
                |(message_id, direction, send_time, account_b, group)| MessageIndexRow {
                    message_id,
                    direction: i32::from(direction),
                    send_time,
                    account_b,
                    group,
                },
            )
            .collect();
        Ok((rows, has_more))
    }

    async fn backfill_delivery(
        &self,
        app: &str,
        account: &str,
        target_id: &str,
    ) -> Result<(), StoreError> {
        if !self.pending_receipt || target_id.is_empty() {
            return Ok(());
        }
        let cutoff = now_unix_nano().saturating_sub(EXPIRES_NANOS);
        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1::text), hashtext($2::text))")
            .bind(app)
            .bind(account)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
        let result = sqlx::query(
            "INSERT INTO pending_delivery (app, account, target_id, message_id, expires_at)
             SELECT i.app, i.account_a, $3, i.message_id, now() + interval '15 days'
               FROM message_index i
               JOIN message_content c ON c.id = i.message_id
              WHERE i.app = $1 AND i.account_a = $2
                AND i.direction = 0
                AND c.send_time >= $4
             ON CONFLICT DO NOTHING",
        )
        .bind(app)
        .bind(account)
        .bind(target_id)
        .bind(cutoff)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        let n = result.rows_affected();
        if n > 10_000 {
            tracing::warn!(account, rows = n, "backfill_rows");
        } else {
            tracing::info!(account, backfill_rows = n, "backfill");
        }
        tx.commit().await.map_err(pg_err)?;
        Ok(())
    }

    async fn gc_expired_deliveries(&self, limit: i64) -> Result<u64, StoreError> {
        let result = sqlx::query(
            "DELETE FROM pending_delivery
              WHERE ctid IN (
                SELECT ctid FROM pending_delivery
                 WHERE expires_at < now()
                 ORDER BY expires_at
                 LIMIT $1
              )",
        )
        .bind(limit)
        .execute(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(result.rows_affected())
    }

    async fn pending_delivery_stats(&self) -> Result<(i64, i64), StoreError> {
        let row: (i64, Option<f64>) = sqlx::query_as(
            "SELECT COUNT(*)::bigint,
                    EXTRACT(EPOCH FROM (now() - MIN(created_at)))
               FROM pending_delivery
              WHERE acked_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok((row.0, row.1.unwrap_or(0.0) as i64))
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
    use crate::store::{DeliveryTarget, MemoryAckIndex};
    use async_trait::async_trait;
    use kim_router::{SessionError, SessionStorage};

    fn sample(sender: &str, dest: &str, send_time: i64, body: &str) -> InsertMessage {
        InsertMessage {
            sender: sender.into(),
            dest: dest.into(),
            send_time,
            msg_type: 1,
            body: body.into(),
            extra: String::new(),
            client_id: String::new(),
            online_targets: Vec::new(),
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
        let idx = store
            .offline_index(&app, "bob", "", 0, false)
            .await
            .unwrap()
            .0;
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].message_id, inserted.message_id);
        assert_eq!(idx[0].account_b, "alice");
        let body = store
            .offline_content(&app, "bob", &[inserted.message_id])
            .await
            .unwrap();
        assert_eq!(body[0].body, "hello");
        store
            .ack(&app, "bob", "", &[inserted.message_id])
            .await
            .unwrap();
        let after = store
            .offline_index(&app, "bob", "", 0, false)
            .await
            .unwrap()
            .0;
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

    async fn connect_pending(
        sessions: Option<Arc<dyn SessionStorage>>,
    ) -> Option<(PostgresMessageStore, sqlx::PgPool)> {
        let url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.is_empty())?;
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let ack: Arc<dyn AckIndex> = Arc::new(MemoryAckIndex::new());
        let pool = match connect_pool(
            &url,
            PoolOpts {
                max_connections: 2,
                acquire_timeout: Duration::from_secs(3),
                idle_timeout: Duration::from_secs(60),
            },
        )
        .await
        {
            Ok(p) => p,
            Err(err) => {
                eprintln!("skip postgres: {err}");
                return None;
            }
        };
        Some((
            PostgresMessageStore::from_pool(pool.clone(), idgen, ack, sessions, true),
            pool,
        ))
    }

    struct BoomSessions;

    #[async_trait]
    impl SessionStorage for BoomSessions {
        async fn add(&self, _: &kim_protocol::pkt::Session) -> Result<(), SessionError> {
            Ok(())
        }
        async fn delete(&self, _: &str, _: &str) -> Result<(), SessionError> {
            Ok(())
        }
        async fn get(&self, _: &str) -> Result<kim_protocol::pkt::Session, SessionError> {
            Err(SessionError::Other("boom".into()))
        }
        async fn get_locations(
            &self,
            _: &[String],
        ) -> Result<Vec<kim_router::Location>, SessionError> {
            Err(SessionError::Other("boom".into()))
        }
        async fn get_location(
            &self,
            _: &str,
            _: &str,
        ) -> Result<kim_router::Location, SessionError> {
            Err(SessionError::Other("boom".into()))
        }
    }

    #[tokio::test]
    async fn postgres_ack_then_backfill_does_not_resurrect() {
        let Some((store, _)) = connect_pending(None).await else {
            return;
        };
        let app = format!("kim_pg_bf_{}", now_unix_nano());
        let mut req = sample("alice", "bob", now_unix_nano(), "hi");
        req.online_targets = vec![DeliveryTarget {
            account: "bob".into(),
            target_id: "j1".into(),
        }];
        let inserted = store.insert_user(&app, &req).await.unwrap();
        store
            .ack(&app, "bob", "j1", &[inserted.message_id])
            .await
            .unwrap();
        store.backfill_delivery(&app, "bob", "j1").await.unwrap();
        let idx = store
            .offline_index(&app, "bob", "j1", 0, true)
            .await
            .unwrap()
            .0;
        assert!(idx.iter().all(|r| r.message_id != inserted.message_id));
    }

    #[tokio::test]
    async fn postgres_list_locations_error_rolls_back_content() {
        let Some((store, pool)) = connect_pending(Some(Arc::new(BoomSessions))).await else {
            return;
        };
        let app = format!("kim_pg_boom_{}", now_unix_nano());
        let err = store
            .insert_user(&app, &sample("alice", "bob", now_unix_nano(), "x"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("boom"));
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*)::bigint FROM message_content WHERE app = $1")
                .bind(&app)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn postgres_duplicate_after_login_upserts_new_jti() {
        let sessions = Arc::new(kim_session::MemorySessionStore::new());
        sessions
            .add(&kim_protocol::pkt::Session {
                channel_id: "bob-1".into(),
                gate_id: "g".into(),
                account: "bob".into(),
                jti: "j1".into(),
                ..kim_protocol::pkt::Session::default()
            })
            .await
            .unwrap();
        let Some((store, _)) = connect_pending(Some(sessions.clone())).await else {
            return;
        };
        let app = format!("kim_pg_login_{}", now_unix_nano());
        let mut req = sample("alice", "bob", now_unix_nano(), "hi");
        req.client_id = "c1".into();
        let first = store.insert_user(&app, &req).await.unwrap();
        assert!(!first.duplicate);
        sessions
            .add(&kim_protocol::pkt::Session {
                channel_id: "bob-2".into(),
                gate_id: "g".into(),
                account: "bob".into(),
                jti: "j2".into(),
                ..kim_protocol::pkt::Session::default()
            })
            .await
            .unwrap();
        let second = store.insert_user(&app, &req).await.unwrap();
        assert!(second.duplicate);
        assert_eq!(first.message_id, second.message_id);
        let j2 = store
            .offline_index(&app, "bob", "j2", 0, true)
            .await
            .unwrap()
            .0;
        assert!(j2.iter().any(|r| r.message_id == first.message_id));
    }
}
