use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use tokio::sync::{mpsc, oneshot};

use crate::agent::{AgentProfileRow, DeviceOverlayRow, ProviderAccountRow};
use crate::command::{CommandReceipt, OutgoingPayload, PageCursor, SendMessageCommand, SendStatus};
use crate::error::{map_sqlx, SdkError};
use crate::sync::UnreadPolicy;
use crate::timeline::{PersonRef, ThreadView, TimelineSnapshot};

pub mod agent;
pub(crate) mod changes;
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

use changes::{ChangeLog, CommitEffect};

const WRITE_CAP: usize = 128;

pub(crate) struct Store {
    pub pool: SqlitePool,
    writes: mpsc::Sender<WriteOp>,
    changes: Arc<ChangeLog>,
}

enum WriteOp {
    Enqueue {
        epoch: u64,
        account: String,
        cmd: SendMessageCommand,
        reply: oneshot::Sender<Result<(CommandReceipt, u64), SdkError>>,
    },
    PersistTalks {
        epoch: u64,
        account: String,
        talks: Vec<kim_client::IncomingTalk>,
        policy: UnreadPolicy,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    PersistInbox {
        epoch: u64,
        account: String,
        items: Vec<kim_client::InboxItem>,
        reply: oneshot::Sender<Result<(Vec<ThreadView>, u64), SdkError>>,
    },
    Cancel {
        epoch: u64,
        account: String,
        client_id: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    DeleteThread {
        epoch: u64,
        account: String,
        dest: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    MarkSent {
        epoch: u64,
        account: String,
        client_id: String,
        message_id: i64,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    MarkFailed {
        epoch: u64,
        account: String,
        client_id: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    MarkRetry {
        epoch: u64,
        account: String,
        client_id: String,
        attempt: i32,
        next_attempt_at: i64,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    Requeue {
        epoch: u64,
        account: String,
        client_id: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    MarkRead {
        epoch: u64,
        account: String,
        dest: String,
        message_id: i64,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    DueNow {
        epoch: u64,
        account: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    ReplaceContacts {
        epoch: u64,
        account: String,
        rows: Vec<PersonRef>,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    UpsertContact {
        epoch: u64,
        account: String,
        peer: String,
        relation: Option<String>,
        nickname: String,
        avatar: String,
        bio: Option<String>,
        kind: Option<i32>,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    DeleteContact {
        epoch: u64,
        account: String,
        peer: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    UpsertDeviceSettings {
        row: settings::DeviceSettings,
        mark_imported: bool,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    EnsureThread {
        account: String,
        dest: String,
        kind: i32,
        reply: oneshot::Sender<Result<(ThreadView, u64), SdkError>>,
    },
    UpsertAgentProfile {
        account: String,
        row: AgentProfileRow,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    DeleteAgentProfile {
        account: String,
        profile_id: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    ImportAgentProfiles {
        account: String,
        rows: Vec<AgentProfileRow>,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    RekeyAgentProfiles {
        from: String,
        to: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    UpsertProviderAccount {
        account: String,
        row: ProviderAccountRow,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    DeleteProviderAccount {
        account: String,
        id: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    UpsertDeviceOverlay {
        account: String,
        row: DeviceOverlayRow,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    UpsertAgentFlags {
        flags_json: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    TouchMedia {
        url: String,
        reply: oneshot::Sender<Result<((), u64), SdkError>>,
    },
    UpsertMedia {
        url: String,
        local_path: String,
        byte_size: i64,
        reply: oneshot::Sender<Result<(Vec<String>, u64), SdkError>>,
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
        let changes = Arc::new(ChangeLog::new());
        let worker_changes = changes.clone();
        tokio::spawn(async move {
            write_worker(worker_pool, epoch, worker_changes, rx).await;
        });
        Ok(Arc::new(Self {
            pool,
            writes: tx,
            changes,
        }))
    }

    pub(crate) fn changes(&self) -> Arc<ChangeLog> {
        self.changes.clone()
    }

    pub(crate) async fn enqueue(
        &self,
        epoch: u64,
        account: String,
        cmd: SendMessageCommand,
    ) -> Result<(CommandReceipt, u64), SdkError> {
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

    pub(crate) async fn load_timeline_window(
        &self,
        account: &str,
        dest: &str,
        limit: i32,
        older_bound: Option<&(i64, String)>,
    ) -> Result<TimelineSnapshot, SdkError> {
        messages::load_timeline_window(&self.pool, account, dest, limit, older_bound).await
    }

    pub(crate) async fn load_page(
        &self,
        account: &str,
        cursor: &PageCursor,
    ) -> Result<(Vec<crate::timeline::MessageView>, bool), SdkError> {
        messages::load_page(&self.pool, account, cursor).await
    }

    pub(crate) async fn count_sent(&self, account: &str, dest: &str) -> Result<i64, SdkError> {
        messages::count_sent(&self.pool, account, dest).await
    }

    pub(crate) async fn local_message_tip(
        &self,
        account: &str,
        dest: &str,
    ) -> Result<i64, SdkError> {
        messages::max_message_id(&self.pool, account, dest).await
    }

    pub(crate) async fn thread_server_tip(
        &self,
        account: &str,
        dest: &str,
    ) -> Result<Option<(i32, i64)>, SdkError> {
        threads::server_tip(&self.pool, account, dest).await
    }

    pub(crate) async fn persist_talks(
        &self,
        epoch: u64,
        account: String,
        talks: Vec<kim_client::IncomingTalk>,
        policy: UnreadPolicy,
    ) -> Result<((), u64), SdkError> {
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

    pub(crate) async fn ensure_thread(
        &self,
        account: String,
        dest: String,
        kind: i32,
    ) -> Result<(ThreadView, u64), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::EnsureThread {
                account,
                dest,
                kind,
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
    ) -> Result<(Vec<ThreadView>, u64), SdkError> {
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
    ) -> Result<((), u64), SdkError> {
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
    ) -> Result<((), u64), SdkError> {
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
    ) -> Result<((), u64), SdkError> {
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
    ) -> Result<((), u64), SdkError> {
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
    ) -> Result<((), u64), SdkError> {
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
    ) -> Result<((), u64), SdkError> {
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

    pub(crate) async fn due_now(&self, epoch: u64, account: String) -> Result<((), u64), SdkError> {
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
        epoch: u64,
        account: String,
        rows: Vec<PersonRef>,
    ) -> Result<((), u64), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::ReplaceContacts {
                epoch,
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn upsert_contact(
        &self,
        epoch: u64,
        account: String,
        peer: String,
        relation: Option<String>,
        nickname: String,
        avatar: String,
        bio: Option<String>,
        kind: Option<i32>,
    ) -> Result<((), u64), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::UpsertContact {
                epoch,
                account,
                peer,
                relation,
                nickname,
                avatar,
                bio,
                kind,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn delete_contact(
        &self,
        epoch: u64,
        account: String,
        peer: String,
    ) -> Result<((), u64), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::DeleteContact {
                epoch,
                account,
                peer,
                reply,
            })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
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
    ) -> Result<((), u64), SdkError> {
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
    ) -> Result<((), u64), SdkError> {
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

    pub(crate) async fn touch_media(&self, url: String) -> Result<((), u64), SdkError> {
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
    ) -> Result<(Vec<String>, u64), SdkError> {
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
    ) -> Result<((), u64), SdkError> {
        if self.agent_profiles_imported().await? {
            return Ok(((), 0));
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

    pub(crate) async fn load_provider_accounts(
        &self,
        account: &str,
    ) -> Result<Vec<ProviderAccountRow>, SdkError> {
        agent::load_provider_accounts(&self.pool, account).await
    }

    pub(crate) async fn load_provider_accounts_all(
        &self,
        account: &str,
    ) -> Result<Vec<ProviderAccountRow>, SdkError> {
        agent::load_provider_accounts_all(&self.pool, account).await
    }

    pub(crate) async fn upsert_provider_account(
        &self,
        account: String,
        row: ProviderAccountRow,
    ) -> Result<((), u64), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::UpsertProviderAccount {
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

    pub(crate) async fn delete_provider_account(
        &self,
        account: String,
        id: String,
    ) -> Result<((), u64), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::DeleteProviderAccount { account, id, reply })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn load_device_overlay(
        &self,
        account: &str,
        profile_id: &str,
    ) -> Result<Option<DeviceOverlayRow>, SdkError> {
        agent::load_overlay(&self.pool, account, profile_id).await
    }

    pub(crate) async fn upsert_device_overlay(
        &self,
        account: String,
        row: DeviceOverlayRow,
    ) -> Result<((), u64), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::UpsertDeviceOverlay {
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

    pub(crate) async fn load_agent_flags(&self) -> Result<String, SdkError> {
        settings::load_agent_flags(&self.pool).await
    }

    pub(crate) async fn upsert_agent_flags(
        &self,
        flags_json: String,
    ) -> Result<((), u64), SdkError> {
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::UpsertAgentFlags { flags_json, reply })
            .map_err(|_| SdkError::Busy {
                queue: "store".into(),
            })?;
        rx.await.map_err(|_| SdkError::Internal {
            message: "store worker dropped".into(),
        })?
    }

    pub(crate) async fn rekey_agent_profiles(
        &self,
        from: String,
        to: String,
    ) -> Result<((), u64), SdkError> {
        if from == to {
            return Ok(((), 0));
        }
        let (reply, rx) = oneshot::channel();
        self.writes
            .try_send(WriteOp::RekeyAgentProfiles { from, to, reply })
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
    ) -> Result<((), u64), SdkError> {
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
    ) -> Result<((), u64), SdkError> {
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

async fn write_worker(
    pool: SqlitePool,
    epoch: Arc<AtomicU64>,
    changes: Arc<ChangeLog>,
    mut rx: mpsc::Receiver<WriteOp>,
) {
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
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
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
            }
            WriteOp::ReplaceContacts {
                epoch: op_epoch,
                account,
                rows,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    replace_contacts_tx(&pool, &account, &rows).await
                };
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
            }
            WriteOp::UpsertContact {
                epoch: op_epoch,
                account,
                peer,
                relation,
                nickname,
                avatar,
                bio,
                kind,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    upsert_contact_tx(
                        &pool,
                        &account,
                        &peer,
                        relation.as_deref(),
                        &nickname,
                        &avatar,
                        bio.as_deref(),
                        kind,
                    )
                    .await
                };
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
            }
            WriteOp::DeleteContact {
                epoch: op_epoch,
                account,
                peer,
                reply,
            } => {
                let current = epoch.load(Ordering::SeqCst);
                let result = if op_epoch != current {
                    Err(SdkError::StaleEpoch {
                        expected: op_epoch,
                        actual: current,
                    })
                } else {
                    delete_contact_tx(&pool, &account, &peer).await
                };
                let _ = reply.send(record_result(&changes, &account, op_epoch, result));
            }
            WriteOp::UpsertDeviceSettings {
                row,
                mark_imported,
                reply,
            } => {
                let result = upsert_settings_tx(&pool, &row, mark_imported).await;
                let _ = reply.send(record_result(&changes, "", 0, result));
            }
            WriteOp::EnsureThread {
                account,
                dest,
                kind,
                reply,
            } => {
                let result = ensure_thread_tx(&pool, &account, &dest, kind).await;
                let _ = reply.send(record_result(&changes, &account, 0, result));
            }
            WriteOp::UpsertAgentProfile {
                account,
                row,
                reply,
            } => {
                let result = upsert_agent_profile_tx(&pool, &account, &row).await;
                let _ = reply.send(record_result(&changes, &account, 0, result));
            }
            WriteOp::DeleteAgentProfile {
                account,
                profile_id,
                reply,
            } => {
                let result = delete_agent_profile_tx(&pool, &account, &profile_id).await;
                let _ = reply.send(record_result(&changes, &account, 0, result));
            }
            WriteOp::ImportAgentProfiles {
                account,
                rows,
                reply,
            } => {
                let result = import_agent_profiles_tx(&pool, &account, &rows).await;
                let _ = reply.send(record_result(&changes, &account, 0, result));
            }
            WriteOp::RekeyAgentProfiles { from, to, reply } => {
                let result = rekey_agent_profiles_tx(&pool, &from, &to).await;
                let _ = reply.send(record_result(&changes, "", 0, result));
            }
            WriteOp::UpsertProviderAccount {
                account,
                row,
                reply,
            } => {
                let result = upsert_provider_account_tx(&pool, &account, &row).await;
                let _ = reply.send(record_result(&changes, &account, 0, result));
            }
            WriteOp::DeleteProviderAccount { account, id, reply } => {
                let result = delete_provider_account_tx(&pool, &account, &id).await;
                let _ = reply.send(record_result(&changes, &account, 0, result));
            }
            WriteOp::UpsertDeviceOverlay {
                account,
                row,
                reply,
            } => {
                let result = upsert_overlay_tx(&pool, &account, &row).await;
                let _ = reply.send(record_result(&changes, &account, 0, result));
            }
            WriteOp::UpsertAgentFlags { flags_json, reply } => {
                let result = upsert_agent_flags_tx(&pool, &flags_json).await;
                let _ = reply.send(record_result(&changes, "", 0, result));
            }
            WriteOp::TouchMedia { url, reply } => {
                let result = touch_media_tx(&pool, &url).await;
                let _ = reply.send(record_result(&changes, "", 0, result));
            }
            WriteOp::UpsertMedia {
                url,
                local_path,
                byte_size,
                reply,
            } => {
                let result = upsert_media_tx(&pool, &url, &local_path, byte_size).await;
                let _ = reply.send(record_result(&changes, "", 0, result));
            }
        }
    }
}

fn record_result<T>(
    changes: &ChangeLog,
    account: &str,
    epoch: u64,
    result: Result<(T, CommitEffect), SdkError>,
) -> Result<(T, u64), SdkError> {
    result.map(|(value, effect)| {
        let sequence = if effect.is_empty() {
            0
        } else {
            changes.record(account.to_owned(), epoch, effect)
        };
        (value, sequence)
    })
}

fn timeline_and_inbox(dest: impl Into<String>) -> CommitEffect {
    let mut effect = CommitEffect::timeline(dest);
    effect.merge(CommitEffect::inbox());
    effect
}

async fn cancel_tx(
    pool: &SqlitePool,
    account: &str,
    client_id: &str,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = async {
        let dest = outbox::dest_for_client_id(&mut conn, account, client_id).await?;
        outbox::cancel(&mut conn, account, client_id).await?;
        Ok::<_, SdkError>((
            (),
            dest.map(timeline_and_inbox)
                .unwrap_or_else(CommitEffect::empty),
        ))
    }
    .await;
    finish_conn(&mut conn, result).await
}

async fn replace_contacts_tx(
    pool: &SqlitePool,
    account: &str,
    rows: &[PersonRef],
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = contacts::replace_all(&mut conn, account, rows, now_ms()).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::contacts_replaced()))
}

#[allow(clippy::too_many_arguments)]
async fn upsert_contact_tx(
    pool: &SqlitePool,
    account: &str,
    peer: &str,
    relation: Option<&str>,
    nickname: &str,
    avatar: &str,
    bio: Option<&str>,
    kind: Option<i32>,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = contacts::upsert_one(
        &mut conn,
        account,
        peer,
        relation,
        nickname,
        avatar,
        bio,
        kind,
        now_ms(),
    )
    .await;
    finish_conn(&mut conn, result).await.map(|changed| {
        (
            (),
            if changed {
                CommitEffect::contacts()
            } else {
                CommitEffect::empty()
            },
        )
    })
}

async fn ensure_thread_tx(
    pool: &SqlitePool,
    account: &str,
    dest: &str,
    kind: i32,
) -> Result<(ThreadView, CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let existed = threads::find(&mut conn, account, dest).await?.is_some();
    let stored = threads::ensure(&mut conn, account, dest, kind).await;
    finish_conn(&mut conn, stored).await.map(|stored| {
        let view = ThreadView {
            id: stored.id,
            kind: stored.kind,
            title: stored.title,
            avatar: stored.avatar,
            last_body: stored.last_body,
            last_at: stored.last_at,
            unread: stored.unread,
        };
        let effect = if existed {
            CommitEffect::empty()
        } else {
            CommitEffect::inbox()
        };
        (view, effect)
    })
}

async fn delete_contact_tx(
    pool: &SqlitePool,
    account: &str,
    peer: &str,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = contacts::delete_peer(&mut conn, account, peer).await;
    finish_conn(&mut conn, result).await.map(|changed| {
        (
            (),
            if changed {
                CommitEffect::contacts()
            } else {
                CommitEffect::empty()
            },
        )
    })
}

async fn upsert_agent_profile_tx(
    pool: &SqlitePool,
    account: &str,
    row: &AgentProfileRow,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = agent::upsert_profile(&mut conn, account, row, now_ms()).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn delete_agent_profile_tx(
    pool: &SqlitePool,
    account: &str,
    profile_id: &str,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = agent::delete_profile(&mut conn, account, profile_id, now_ms()).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn touch_media_tx(pool: &SqlitePool, url: &str) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = media::touch(&mut conn, url, now_ms()).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn upsert_media_tx(
    pool: &SqlitePool,
    url: &str,
    local_path: &str,
    byte_size: i64,
) -> Result<(Vec<String>, CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = media::upsert(&mut conn, url, local_path, byte_size, now_ms()).await;
    finish_conn(&mut conn, result)
        .await
        .map(|evicted| (evicted, CommitEffect::empty()))
}

async fn import_agent_profiles_tx(
    pool: &SqlitePool,
    account: &str,
    rows: &[AgentProfileRow],
) -> Result<((), CommitEffect), SdkError> {
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
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn upsert_provider_account_tx(
    pool: &SqlitePool,
    account: &str,
    row: &ProviderAccountRow,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = agent::upsert_provider_account(&mut conn, account, row, now_ms()).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn delete_provider_account_tx(
    pool: &SqlitePool,
    account: &str,
    id: &str,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = agent::delete_provider_account(&mut conn, account, id, now_ms()).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn upsert_overlay_tx(
    pool: &SqlitePool,
    account: &str,
    row: &DeviceOverlayRow,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = agent::upsert_overlay(&mut conn, account, row).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn upsert_agent_flags_tx(
    pool: &SqlitePool,
    flags_json: &str,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = settings::upsert_agent_flags(&mut conn, flags_json).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn rekey_agent_profiles_tx(
    pool: &SqlitePool,
    from: &str,
    to: &str,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = agent::rekey(&mut conn, from, to).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn upsert_settings_tx(
    pool: &SqlitePool,
    row: &settings::DeviceSettings,
    mark_imported: bool,
) -> Result<((), CommitEffect), SdkError> {
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
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn delete_thread_tx(
    pool: &SqlitePool,
    account: &str,
    dest: &str,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = outbox::delete_thread(&mut conn, account, dest).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), timeline_and_inbox(dest)))
}

async fn mark_sent_tx(
    pool: &SqlitePool,
    account: &str,
    client_id: &str,
    message_id: i64,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = async {
        let dest = outbox::dest_for_client_id(&mut conn, account, client_id).await?;
        outbox::mark_sent(&mut conn, account, client_id, message_id, now_ms()).await?;
        Ok::<_, SdkError>((
            (),
            dest.map(timeline_and_inbox)
                .unwrap_or_else(CommitEffect::empty),
        ))
    }
    .await;
    finish_conn(&mut conn, result).await
}

async fn mark_failed_tx(
    pool: &SqlitePool,
    account: &str,
    client_id: &str,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = async {
        let dest = outbox::dest_for_client_id(&mut conn, account, client_id).await?;
        outbox::mark_failed(&mut conn, account, client_id, now_ms()).await?;
        Ok::<_, SdkError>((
            (),
            dest.map(CommitEffect::timeline)
                .unwrap_or_else(CommitEffect::empty),
        ))
    }
    .await;
    finish_conn(&mut conn, result).await
}

async fn mark_retry_tx(
    pool: &SqlitePool,
    account: &str,
    client_id: &str,
    attempt: i32,
    next_attempt_at: i64,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = async {
        let dest = outbox::dest_for_client_id(&mut conn, account, client_id).await?;
        outbox::mark_retry(
            &mut conn,
            account,
            client_id,
            attempt,
            next_attempt_at,
            now_ms(),
        )
        .await?;
        Ok::<_, SdkError>((
            (),
            dest.map(CommitEffect::timeline)
                .unwrap_or_else(CommitEffect::empty),
        ))
    }
    .await;
    finish_conn(&mut conn, result).await
}

async fn requeue_tx(
    pool: &SqlitePool,
    account: &str,
    client_id: &str,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = async {
        let dest = outbox::dest_for_client_id(&mut conn, account, client_id).await?;
        outbox::requeue(&mut conn, account, client_id, now_ms()).await?;
        Ok::<_, SdkError>((
            (),
            dest.map(CommitEffect::timeline)
                .unwrap_or_else(CommitEffect::empty),
        ))
    }
    .await;
    finish_conn(&mut conn, result).await
}

async fn due_now_tx(pool: &SqlitePool, account: &str) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = outbox::wake_pending(&mut conn, account, now_ms()).await;
    finish_conn(&mut conn, result)
        .await
        .map(|_| ((), CommitEffect::empty()))
}

async fn mark_read_tx(
    pool: &SqlitePool,
    account: &str,
    dest: &str,
    message_id: i64,
) -> Result<((), CommitEffect), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = async {
        let (unread, last_read) = watermarks::peek(&mut conn, account, dest).await?;
        if unread == 0 && message_id <= last_read {
            return Ok(CommitEffect::empty());
        }
        watermarks::advance(&mut conn, account, dest, message_id, now_ms()).await?;
        Ok(timeline_and_inbox(dest))
    }
    .await;
    finish_conn(&mut conn, result)
        .await
        .map(|effect| ((), effect))
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
) -> Result<(CommandReceipt, CommitEffect), SdkError> {
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
    let dest = cmd.dest;
    Ok((
        CommandReceipt {
            request_id,
            client_id,
            dest: dest.clone(),
            accepted_at: now,
            send_status: SendStatus::Pending,
        },
        timeline_and_inbox(dest),
    ))
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
) -> Result<((), CommitEffect), SdkError> {
    if talks.is_empty() {
        return Ok(((), CommitEffect::empty()));
    }
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    begin_immediate(&mut conn).await?;
    let result = async {
        let mut dests = Vec::new();
        for talk in talks {
            if let Some(out) = messages::apply_talk(&mut conn, account, talk, policy).await? {
                if !out.needs_publish() {
                    continue;
                }
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
        let mut effect = CommitEffect::empty();
        for dest in dests {
            messages::bump_timeline_version(&mut conn, account, &dest).await?;
            messages::prune(&mut conn, account, &dest).await?;
            effect.merge(CommitEffect::timeline(dest));
        }
        if !effect.is_empty() {
            effect.merge(CommitEffect::inbox());
        }
        Ok::<_, SdkError>(((), effect))
    }
    .await;
    finish_conn(&mut conn, result).await
}

async fn persist_inbox_tx(
    pool: &SqlitePool,
    account: &str,
    items: &[kim_client::InboxItem],
) -> Result<(Vec<ThreadView>, CommitEffect), SdkError> {
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
        Ok::<_, SdkError>((views, CommitEffect::inbox()))
    }
    .await;
    finish_conn(&mut conn, result).await
}

#[cfg(test)]
mod tests {
    use super::changes::ChangedQuery;
    use super::*;

    #[tokio::test]
    async fn enqueue_records_commit_effect_and_sequence() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("kim-cache.db");
        migrate_path(path.clone()).await.expect("migrate");
        let epoch = Arc::new(AtomicU64::new(7));
        let store = Store::open(path, epoch).await.expect("open");
        let changes = store.changes();
        let waiter_changes = changes.clone();
        let waiter = tokio::spawn(async move { waiter_changes.take().await });
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(10)).await;

        let (_receipt, sequence) = store
            .enqueue(
                7,
                "alice".into(),
                SendMessageCommand {
                    dest: "bob".into(),
                    kind: kim_protocol::INBOX_KIND_USER,
                    payload: OutgoingPayload::Text {
                        body: "hello".into(),
                    },
                    client_id: Some("11111111-1111-4111-8111-111111111111".into()),
                    batch_id: None,
                },
            )
            .await
            .expect("enqueue");
        let notice = tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("notice")
            .expect("join");

        assert_eq!(sequence, 1);
        assert_eq!(notice.sequence, sequence);
        assert!(notice
            .queries
            .contains(&ChangedQuery::Timeline { dest: "bob".into() }));
        assert!(notice.queries.contains(&ChangedQuery::Inbox));
    }
}
