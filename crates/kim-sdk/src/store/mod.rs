use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Connection, SqlitePool};
use tokio::sync::{mpsc, oneshot};

use crate::command::{
    CommandReceipt, MessagePage, OutgoingPayload, PageCursor, SendMessageCommand, SendStatus,
};
use crate::error::{map_sqlx, SdkError};
use crate::sync::UnreadPolicy;
use crate::timeline::ThreadView;

pub mod cursors;
pub mod messages;
pub mod migrate;
pub mod outbox;
pub mod schema;
pub mod threads;
pub mod watermarks;

const WRITE_CAP: usize = 128;

pub(crate) struct Store {
    pub pool: SqlitePool,
    writes: mpsc::Sender<WriteOp>,
}

enum WriteOp {
    Enqueue {
        epoch: u64,
        account: String,
        cmd: SendMessageCommand,
        reply: oneshot::Sender<Result<CommandReceipt, SdkError>>,
    },
    PersistTalks {
        epoch: u64,
        account: String,
        talks: Vec<kim_client::IncomingTalk>,
        policy: UnreadPolicy,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    PersistInbox {
        epoch: u64,
        account: String,
        items: Vec<kim_client::InboxItem>,
        reply: oneshot::Sender<Result<Vec<ThreadView>, SdkError>>,
    },
}

pub(crate) fn connect_options(path: &PathBuf) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_millis(3000))
        .foreign_keys(true)
}

pub(crate) async fn migrate_path(path: PathBuf) -> Result<(), SdkError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_options(&path))
        .await
        .map_err(map_sqlx)?;
    migrate::migrate(&pool).await?;
    pool.close().await;
    Ok(())
}

impl Store {
    pub(crate) async fn open(path: PathBuf, epoch: Arc<AtomicU64>) -> Result<Arc<Self>, SdkError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(connect_options(&path))
            .await
            .map_err(map_sqlx)?;
        let (tx, rx) = mpsc::channel(WRITE_CAP);
        let worker_pool = pool.clone();
        tokio::spawn(async move {
            write_worker(worker_pool, epoch, rx).await;
        });
        Ok(Arc::new(Self { pool, writes: tx }))
    }

    pub(crate) async fn enqueue(
        &self,
        epoch: u64,
        account: String,
        cmd: SendMessageCommand,
    ) -> Result<CommandReceipt, SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::Enqueue {
                epoch,
                account,
                cmd,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn load_older(
        &self,
        account: &str,
        cursor: PageCursor,
    ) -> Result<MessagePage, SdkError> {
        let dest = cursor.dest.clone();
        let (messages, has_more) = messages::load_page(&self.pool, account, &cursor).await?;
        Ok(MessagePage {
            dest,
            messages,
            has_more,
        })
    }

    pub(crate) async fn persist_talks(
        &self,
        epoch: u64,
        account: String,
        talks: Vec<kim_client::IncomingTalk>,
        policy: UnreadPolicy,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::PersistTalks {
                epoch,
                account,
                talks,
                policy,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn persist_inbox(
        &self,
        epoch: u64,
        account: String,
        items: Vec<kim_client::InboxItem>,
    ) -> Result<Vec<ThreadView>, SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::PersistInbox {
                epoch,
                account,
                items,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn load_threads(&self, account: &str) -> Result<Vec<ThreadView>, SdkError> {
        threads::load_all(&self.pool, account).await
    }
}

async fn write_worker(pool: SqlitePool, epoch: Arc<AtomicU64>, mut rx: mpsc::Receiver<WriteOp>) {
    while let Some(op) = rx.recv().await {
        match op {
            WriteOp::Enqueue {
                epoch: op_epoch,
                account,
                cmd,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    persist_enqueue(&pool, &account, cmd).await
                };
                let _ = reply.send(result);
            }
            WriteOp::PersistTalks {
                epoch: op_epoch,
                account,
                talks,
                policy,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    persist_talks_tx(&pool, &account, &talks, policy).await
                };
                let _ = reply.send(result);
            }
            WriteOp::PersistInbox {
                epoch: op_epoch,
                account,
                items,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    persist_inbox_tx(&pool, &account, &items).await
                };
                let _ = reply.send(result);
            }
        }
    }
}

async fn persist_enqueue(
    pool: &SqlitePool,
    account: &str,
    cmd: SendMessageCommand,
) -> Result<CommandReceipt, SdkError> {
    if cmd.dest.is_empty() {
        return Err(SdkError::InvalidArgument {
            message: "dest is required".into(),
        });
    }
    if cmd.dest == account {
        return Err(SdkError::CannotChatSelf);
    }
    let client_id = match cmd.client_id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => uuid::Uuid::new_v4().to_string(),
    };
    let request_id = uuid::Uuid::new_v4().to_string();
    let now = now_ms();
    let batch_id = cmd.batch_id.clone().unwrap_or_default();
    let (body, extra, local_path, mime, width, height, byte_size) = match &cmd.payload {
        OutgoingPayload::Text { body } => (
            body.clone(),
            String::new(),
            String::new(),
            String::new(),
            0,
            0,
            0,
        ),
        OutgoingPayload::Image { media } => (
            media.path.clone(),
            String::new(),
            media.path.clone(),
            media.mime.clone(),
            media.width,
            media.height,
            media.byte_size,
        ),
        OutgoingPayload::Video { url, extra } => (
            url.clone(),
            extra.clone(),
            String::new(),
            String::new(),
            0,
            0,
            0,
        ),
    };
    let payload_type = cmd.payload.payload_type();
    let kind_i32 = payload_type;
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    let mut tx = conn.begin().await.map_err(map_sqlx)?;
    let result = async {
        messages::insert_own(
            &mut tx,
            messages::OwnInsert {
                account,
                dest: &cmd.dest,
                key: &client_id,
                sender: account,
                body: &body,
                at: now,
                kind: kind_i32,
                width,
                height,
                batch_id: &batch_id,
                status: SendStatus::Pending,
                local_path: &local_path,
                thread_kind: cmd.kind,
            },
        )
        .await?;
        outbox::insert(
            &mut tx,
            outbox::OutboxInsert {
                account,
                client_id: &client_id,
                dest: &cmd.dest,
                kind: cmd.kind,
                payload_type,
                body: &body,
                extra: &extra,
                local_path: &local_path,
                mime: &mime,
                width,
                height,
                byte_size,
                batch_id: &batch_id,
                status: SendStatus::Pending,
                now,
            },
        )
        .await?;
        threads::upsert_on_send(&mut tx, account, &cmd.dest, cmd.kind, &body, now).await?;
        messages::prune(&mut tx, account, &cmd.dest).await?;
        Ok::<(), SdkError>(())
    }
    .await;
    match result {
        Ok(()) => tx.commit().await.map_err(map_sqlx)?,
        Err(e) => {
            let _ = tx.rollback().await;
            return Err(e);
        }
    }
    Ok(CommandReceipt {
        request_id,
        client_id,
        dest: cmd.dest,
        accepted_at: now,
        send_status: SendStatus::Pending,
    })
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

pub(crate) fn send_time_ms(send_time: i64) -> i64 {
    if send_time <= 0 {
        return now_ms();
    }
    if send_time > 10_000_000_000_000_000 {
        send_time / 1_000_000
    } else if send_time > 100_000_000_000_000 {
        send_time / 1_000
    } else if send_time > 100_000_000_000 {
        send_time
    } else {
        send_time.saturating_mul(1000)
    }
}

async fn persist_talks_tx(
    pool: &SqlitePool,
    account: &str,
    talks: &[kim_client::IncomingTalk],
    policy: UnreadPolicy,
) -> Result<(), SdkError> {
    if talks.is_empty() {
        return Ok(());
    }
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    let mut tx = conn.begin().await.map_err(map_sqlx)?;
    let result = async {
        let mut dests = Vec::new();
        for talk in talks {
            if let Some(out) = messages::apply_talk(&mut tx, account, talk, policy).await? {
                threads::apply_incoming(
                    &mut tx,
                    account,
                    &out.dest,
                    &out.msg.body,
                    out.msg.at,
                    out.unread_delta,
                    out.msg.thread_kind,
                )
                .await?;
                dests.push(out.dest);
            }
        }
        dests.sort();
        dests.dedup();
        for dest in dests {
            messages::prune(&mut tx, account, &dest).await?;
        }
        Ok::<(), SdkError>(())
    }
    .await;
    match result {
        Ok(()) => tx.commit().await.map_err(map_sqlx)?,
        Err(e) => {
            let _ = tx.rollback().await;
            return Err(e);
        }
    }
    Ok(())
}

async fn persist_inbox_tx(
    pool: &SqlitePool,
    account: &str,
    items: &[kim_client::InboxItem],
) -> Result<Vec<ThreadView>, SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    let mut tx = conn.begin().await.map_err(map_sqlx)?;
    let result = async {
        let mut views = Vec::with_capacity(items.len());
        for item in items {
            let t = threads::persist_inbox_item(&mut tx, account, item).await?;
            views.push(ThreadView {
                id: t.id,
                kind: t.kind,
                title: t.title,
                avatar: t.avatar,
                last_body: t.last_body,
                last_at: t.last_at,
                unread: t.unread,
            });
        }
        Ok::<Vec<ThreadView>, SdkError>(views)
    }
    .await;
    match result {
        Ok(views) => {
            tx.commit().await.map_err(map_sqlx)?;
            Ok(views)
        }
        Err(e) => {
            let _ = tx.rollback().await;
            Err(e)
        }
    }
}
