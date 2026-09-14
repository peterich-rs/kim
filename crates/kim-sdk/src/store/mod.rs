use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use tokio::sync::{mpsc, oneshot};

use crate::agent::AgentProfileRow;
use crate::command::{
    CommandReceipt, MessagePage, OutgoingPayload, PageCursor, SendMessageCommand, SendStatus,
};
use crate::error::{map_sqlx, SdkError};
use crate::sync::UnreadPolicy;
use crate::timeline::{PersonRef, ThreadView, TimelineSnapshot};

pub mod agent;
pub mod contacts;
pub mod cursors;
pub mod media;
pub mod messages;
pub mod migrate;
pub mod outbox;
pub mod prepare;
pub mod schema;
pub mod settings;
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
    Cancel {
        epoch: u64,
        account: String,
        client_id: String,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    DeleteThread {
        epoch: u64,
        account: String,
        dest: String,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    MarkSent {
        epoch: u64,
        account: String,
        client_id: String,
        message_id: i64,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    MarkFailed {
        epoch: u64,
        account: String,
        client_id: String,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    MarkRetry {
        epoch: u64,
        account: String,
        client_id: String,
        attempt: i32,
        next_attempt_at: i64,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    Requeue {
        epoch: u64,
        account: String,
        client_id: String,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    MarkRead {
        epoch: u64,
        account: String,
        dest: String,
        message_id: i64,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    DueNow {
        epoch: u64,
        account: String,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    ReplaceContacts {
        account: String,
        rows: Vec<PersonRef>,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    UpsertDeviceSettings {
        row: settings::DeviceSettings,
        mark_imported: bool,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    UpsertAgentProfile {
        account: String,
        row: AgentProfileRow,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    DeleteAgentProfile {
        account: String,
        profile_id: String,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    ImportAgentProfiles {
        account: String,
        rows: Vec<AgentProfileRow>,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    TouchMedia {
        url: String,
        reply: oneshot::Sender<Result<(), SdkError>>,
    },
    UpsertMedia {
        url: String,
        local_path: String,
        byte_size: i64,
        reply: oneshot::Sender<Result<Vec<String>, SdkError>>,
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

    pub(crate) async fn load_hot_window(
        &self,
        account: &str,
        dest: &str,
        limit: i32,
    ) -> Result<TimelineSnapshot, SdkError> {
        messages::load_hot_window(&self.pool, account, dest, limit).await
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

    pub(crate) async fn load_due(&self, account: &str) -> Result<Vec<outbox::OutboxRow>, SdkError> {
        outbox::load_due(&self.pool, account, now_ms()).await
    }

    pub(crate) async fn get_row(
        &self,
        account: &str,
        client_id: &str,
    ) -> Result<Option<outbox::OutboxRow>, SdkError> {
        outbox::get_row(&self.pool, account, client_id).await
    }

    pub(crate) async fn outbox_alive(
        &self,
        account: &str,
        client_id: &str,
    ) -> Result<bool, SdkError> {
        outbox::alive(&self.pool, account, client_id).await
    }

    pub(crate) async fn cancel(
        &self,
        epoch: u64,
        account: String,
        client_id: String,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::Cancel {
                epoch,
                account,
                client_id,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn delete_thread(
        &self,
        epoch: u64,
        account: String,
        dest: String,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::DeleteThread {
                epoch,
                account,
                dest,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn mark_sent(
        &self,
        epoch: u64,
        account: String,
        client_id: String,
        message_id: i64,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::MarkSent {
                epoch,
                account,
                client_id,
                message_id,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn mark_retry(
        &self,
        epoch: u64,
        account: String,
        client_id: String,
        attempt: i32,
        next_attempt_at: i64,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::MarkRetry {
                epoch,
                account,
                client_id,
                attempt,
                next_attempt_at,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn requeue(
        &self,
        epoch: u64,
        account: String,
        client_id: String,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::Requeue {
                epoch,
                account,
                client_id,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn mark_read_local(
        &self,
        epoch: u64,
        account: String,
        dest: String,
        message_id: i64,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::MarkRead {
                epoch,
                account,
                dest,
                message_id,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn due_now(&self, epoch: u64, account: String) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::DueNow {
                epoch,
                account,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn replace_contacts(
        &self,
        account: String,
        rows: Vec<PersonRef>,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::ReplaceContacts {
                account,
                rows,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn load_contacts(&self, account: &str) -> Result<Vec<PersonRef>, SdkError> {
        contacts::load_all(&self.pool, account).await
    }

    pub(crate) async fn load_device_settings(&self) -> Result<settings::DeviceSettings, SdkError> {
        settings::load_device(&self.pool).await
    }

    pub(crate) async fn prefs_imported(&self) -> Result<bool, SdkError> {
        settings::imported_prefs(&self.pool).await
    }

    pub(crate) async fn agent_profile_id_for_dest(
        &self,
        account: &str,
        dest: &str,
    ) -> Result<Option<String>, SdkError> {
        agent::profile_id_for_dest(&self.pool, account, dest).await
    }

    pub(crate) async fn agent_owned_dests(&self, account: &str) -> Result<Vec<String>, SdkError> {
        agent::owned_dests(&self.pool, account).await
    }

    pub(crate) async fn load_agent_profiles(
        &self,
        account: &str,
    ) -> Result<Vec<AgentProfileRow>, SdkError> {
        agent::load_all(&self.pool, account).await
    }

    pub(crate) async fn agent_profiles_imported(&self) -> Result<bool, SdkError> {
        agent::imported(&self.pool).await
    }

    pub(crate) async fn upsert_agent_profile(
        &self,
        account: String,
        row: AgentProfileRow,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::UpsertAgentProfile {
                account,
                row,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn delete_agent_profile(
        &self,
        account: String,
        profile_id: String,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::DeleteAgentProfile {
                account,
                profile_id,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn lookup_media(
        &self,
        url: &str,
    ) -> Result<Option<media::MediaCacheRow>, SdkError> {
        media::lookup(&self.pool, url).await
    }

    pub(crate) async fn touch_media(&self, url: String) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::TouchMedia { url, reply })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn upsert_media(
        &self,
        url: String,
        local_path: String,
        byte_size: i64,
    ) -> Result<Vec<String>, SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::UpsertMedia {
                url,
                local_path,
                byte_size,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn search_messages(
        &self,
        account: &str,
        query: &str,
        dest: Option<&str>,
    ) -> Result<Vec<(String, String, String, String, i64, i64)>, SdkError> {
        media::search_messages(&self.pool, account, query, dest, schema::SEARCH_CAP).await
    }

    pub(crate) async fn import_agent_profiles(
        &self,
        account: String,
        rows: Vec<AgentProfileRow>,
    ) -> Result<(), SdkError> {
        if self.agent_profiles_imported().await? {
            return Ok(());
        }
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::ImportAgentProfiles {
                account,
                rows,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn upsert_device_settings(
        &self,
        row: settings::DeviceSettings,
        mark_imported: bool,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::UpsertDeviceSettings {
                row,
                mark_imported,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn mark_failed(
        &self,
        epoch: u64,
        account: String,
        client_id: String,
    ) -> Result<(), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::MarkFailed {
                epoch,
                account,
                client_id,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
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
            WriteOp::Cancel {
                epoch: op_epoch,
                account,
                client_id,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    cancel_tx(&pool, &account, &client_id).await
                };
                let _ = reply.send(result);
            }
            WriteOp::DeleteThread {
                epoch: op_epoch,
                account,
                dest,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    delete_thread_tx(&pool, &account, &dest).await
                };
                let _ = reply.send(result);
            }
            WriteOp::MarkSent {
                epoch: op_epoch,
                account,
                client_id,
                message_id,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    mark_sent_tx(&pool, &account, &client_id, message_id).await
                };
                let _ = reply.send(result);
            }
            WriteOp::MarkFailed {
                epoch: op_epoch,
                account,
                client_id,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    mark_failed_tx(&pool, &account, &client_id).await
                };
                let _ = reply.send(result);
            }
            WriteOp::MarkRetry {
                epoch: op_epoch,
                account,
                client_id,
                attempt,
                next_attempt_at,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    mark_retry_tx(&pool, &account, &client_id, attempt, next_attempt_at).await
                };
                let _ = reply.send(result);
            }
            WriteOp::Requeue {
                epoch: op_epoch,
                account,
                client_id,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    requeue_tx(&pool, &account, &client_id).await
                };
                let _ = reply.send(result);
            }
            WriteOp::MarkRead {
                epoch: op_epoch,
                account,
                dest,
                message_id,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    mark_read_tx(&pool, &account, &dest, message_id).await
                };
                let _ = reply.send(result);
            }
            WriteOp::DueNow {
                epoch: op_epoch,
                account,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    due_now_tx(&pool, &account).await
                };
                let _ = reply.send(result);
            }
            WriteOp::ReplaceContacts {
                account,
                rows,
                reply,
            } => {
                let result = replace_contacts_tx(&pool, &account, &rows).await;
                let _ = reply.send(result);
            }
            WriteOp::UpsertDeviceSettings {
                row,
                mark_imported,
                reply,
            } => {
                let result = upsert_settings_tx(&pool, &row, mark_imported).await;
                let _ = reply.send(result);
            }
            WriteOp::UpsertAgentProfile {
                account,
                row,
                reply,
            } => {
                let result = upsert_agent_profile_tx(&pool, &account, &row).await;
                let _ = reply.send(result);
            }
            WriteOp::DeleteAgentProfile {
                account,
                profile_id,
                reply,
            } => {
                let result = delete_agent_profile_tx(&pool, &account, &profile_id).await;
                let _ = reply.send(result);
            }
            WriteOp::ImportAgentProfiles {
                account,
                rows,
                reply,
            } => {
                let result = import_agent_profiles_tx(&pool, &account, &rows).await;
                let _ = reply.send(result);
            }
            WriteOp::TouchMedia { url, reply } => {
                let result = touch_media_tx(&pool, &url).await;
                let _ = reply.send(result);
            }
            WriteOp::UpsertMedia {
                url,
                local_path,
                byte_size,
                reply,
            } => {
                let result = upsert_media_tx(&pool, &url, &local_path, byte_size).await;
                let _ = reply.send(result);
            }
        }
    }
}

async fn cancel_tx(pool: &SqlitePool, account: &str, client_id: &str) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = outbox::cancel(&mut conn, account, client_id).await;
    finish_conn(&mut conn, result).await
}

async fn replace_contacts_tx(
    pool: &SqlitePool,
    account: &str,
    rows: &[PersonRef],
) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = contacts::replace_all(&mut conn, account, rows, now_ms()).await;
    finish_conn(&mut conn, result).await
}

async fn upsert_agent_profile_tx(
    pool: &SqlitePool,
    account: &str,
    row: &AgentProfileRow,
) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = agent::upsert_profile(&mut conn, account, row, now_ms()).await;
    finish_conn(&mut conn, result).await
}

async fn delete_agent_profile_tx(
    pool: &SqlitePool,
    account: &str,
    profile_id: &str,
) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = agent::delete_profile(&mut conn, account, profile_id).await;
    finish_conn(&mut conn, result).await
}

async fn touch_media_tx(pool: &SqlitePool, url: &str) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = media::touch(&mut conn, url, now_ms()).await;
    finish_conn(&mut conn, result).await
}

async fn upsert_media_tx(
    pool: &SqlitePool,
    url: &str,
    local_path: &str,
    byte_size: i64,
) -> Result<Vec<String>, SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = media::upsert(&mut conn, url, local_path, byte_size, now_ms()).await;
    finish_conn(&mut conn, result).await
}

async fn import_agent_profiles_tx(
    pool: &SqlitePool,
    account: &str,
    rows: &[AgentProfileRow],
) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = async {
        for row in rows {
            agent::upsert_profile(&mut conn, account, row, now_ms()).await?;
        }
        agent::mark_imported(&mut conn).await?;
        Ok(())
    }
    .await;
    finish_conn(&mut conn, result).await
}

async fn upsert_settings_tx(
    pool: &SqlitePool,
    row: &settings::DeviceSettings,
    mark_imported: bool,
) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = async {
        settings::upsert_device(&mut conn, row).await?;
        if mark_imported {
            settings::mark_imported(&mut conn).await?;
        }
        Ok(())
    }
    .await;
    finish_conn(&mut conn, result).await
}

async fn delete_thread_tx(pool: &SqlitePool, account: &str, dest: &str) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = outbox::delete_thread(&mut conn, account, dest).await;
    finish_conn(&mut conn, result).await
}

async fn mark_sent_tx(
    pool: &SqlitePool,
    account: &str,
    client_id: &str,
    message_id: i64,
) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = outbox::mark_sent(&mut conn, account, client_id, message_id, now_ms()).await;
    finish_conn(&mut conn, result).await
}

async fn mark_failed_tx(pool: &SqlitePool, account: &str, client_id: &str) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = outbox::mark_failed(&mut conn, account, client_id, now_ms()).await;
    finish_conn(&mut conn, result).await
}

async fn mark_retry_tx(
    pool: &SqlitePool,
    account: &str,
    client_id: &str,
    attempt: i32,
    next_attempt_at: i64,
) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = outbox::mark_retry(
        &mut conn,
        account,
        client_id,
        attempt,
        next_attempt_at,
        now_ms(),
    )
    .await;
    finish_conn(&mut conn, result).await
}

async fn requeue_tx(pool: &SqlitePool, account: &str, client_id: &str) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = outbox::requeue(&mut conn, account, client_id, now_ms()).await;
    finish_conn(&mut conn, result).await
}

async fn due_now_tx(pool: &SqlitePool, account: &str) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = outbox::wake_pending(&mut conn, account, now_ms()).await;
    finish_conn(&mut conn, result).await
}

async fn mark_read_tx(
    pool: &SqlitePool,
    account: &str,
    dest: &str,
    message_id: i64,
) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = watermarks::advance(&mut conn, account, dest, message_id, now_ms()).await;
    finish_conn(&mut conn, result).await
}

async fn begin_immediate(conn: &mut sqlx::SqliteConnection) -> Result<(), SdkError> {
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn finish_conn<T>(
    conn: &mut sqlx::SqliteConnection,
    result: Result<T, SdkError>,
) -> Result<T, SdkError> {
    match result {
        Ok(v) => {
            sqlx::query("COMMIT")
                .execute(&mut *conn)
                .await
                .map_err(map_sqlx)?;
            Ok(v)
        }
        Err(e) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            Err(e)
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
    begin_immediate(&mut conn).await?;
    let result = async {
        messages::insert_own(
            &mut conn,
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
            &mut conn,
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
        threads::upsert_on_send(&mut conn, account, &cmd.dest, cmd.kind, &body, now).await?;
        messages::bump_timeline_version(&mut conn, account, &cmd.dest).await?;
        messages::prune(&mut conn, account, &cmd.dest).await?;
        Ok::<(), SdkError>(())
    }
    .await;
    finish_conn(&mut conn, result).await?;
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
    begin_immediate(&mut conn).await?;
    let result = async {
        let mut dests = Vec::new();
        for talk in talks {
            if let Some(out) = messages::apply_talk(&mut conn, account, talk, policy).await? {
                threads::apply_incoming(
                    &mut conn,
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
            messages::bump_timeline_version(&mut conn, account, &dest).await?;
            messages::prune(&mut conn, account, &dest).await?;
        }
        Ok::<(), SdkError>(())
    }
    .await;
    finish_conn(&mut conn, result).await
}

async fn persist_inbox_tx(
    pool: &SqlitePool,
    account: &str,
    items: &[kim_client::InboxItem],
) -> Result<Vec<ThreadView>, SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = async {
        let mut views = Vec::with_capacity(items.len());
        for item in items {
            let t = threads::persist_inbox_item(&mut conn, account, item).await?;
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
    finish_conn(&mut conn, result).await
}
