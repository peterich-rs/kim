//! Deep module for local message lifecycle: store, outbox, unread, persist-then-ack.

mod agent;
mod command;
mod error;
mod ids;
mod media;
mod proto;
mod session;
mod store;
mod timeline;

pub mod outbox;
pub mod sync;

use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;

pub use agent::{AgentPort, NoopAgent};
pub use command::{
    CommandReceipt, MessagePage, OutgoingPayload, PageCursor, ReadMarker, SendMessageCommand,
    SendStatus, StartSession, TimelineQuery,
};
pub use error::SdkError;
pub use ids::{
    incoming_message_key, is_client_key, prefer_key, AccountId, ClientMessageId, DestId,
    SessionEpoch,
};
pub use media::{image_mime_ok, MediaRef, MAX_IMAGE_BYTES};
pub use proto::ProtocolClient;
pub use timeline::{
    LinkStateView, MessageView, SessionSnapshot, SessionUpdate, ThreadView, TimelineDelta,
    TimelineSnapshot, TimelineUpdate,
};

use crate::session::lock;
use crate::store::Store;

struct Inner {
    epoch: Arc<AtomicU64>,
    cancel: Mutex<CancellationToken>,
    session: Mutex<Option<StartSession>>,
    store: Mutex<Option<Arc<Store>>>,
    #[allow(dead_code)]
    agent: Arc<dyn AgentPort>,
}

pub struct KimSdk {
    inner: Arc<Inner>,
}

impl KimSdk {
    /// Protocol client, does not open SQLite.
    #[must_use]
    pub fn protocol_only() -> Arc<Self> {
        Arc::new(Self {
            inner: Arc::new(Inner {
                epoch: Arc::new(AtomicU64::new(0)),
                cancel: Mutex::new(CancellationToken::new()),
                session: Mutex::new(None),
                store: Mutex::new(None),
                agent: Arc::new(NoopAgent),
            }),
        })
    }

    /// Flag-on / `cargo test -p kim-sdk` fixture. protocol_only + attach_store.
    pub async fn open(db_path: String) -> Result<Arc<Self>, SdkError> {
        let sdk = Self::protocol_only();
        sdk.attach_store(db_path).await?;
        Ok(sdk)
    }

    /// Open `kim-cache.db`. Migration runs on a blocking thread.
    pub async fn attach_store(&self, db_path: String) -> Result<(), SdkError> {
        if db_path.is_empty() {
            return Err(SdkError::InvalidArgument {
                message: "db path is required".into(),
            });
        }
        {
            let existing = lock(&self.inner.store);
            if existing.is_some() {
                return Err(SdkError::InvalidArgument {
                    message: "store already attached".into(),
                });
            }
        }
        let path = PathBuf::from(db_path);
        let migrate_path = path.clone();
        tokio::task::spawn_blocking(move || migrate_blocking(migrate_path))
            .await
            .map_err(|e| SdkError::Internal {
                message: format!("join: {e}"),
            })??;
        let epoch = self.inner.epoch.clone();
        let store = Store::open(path, epoch).await?;
        *lock(&self.inner.store) = Some(store);
        Ok(())
    }

    pub async fn start_session(&self, s: StartSession) -> Result<(), SdkError> {
        if s.account.is_empty() {
            return Err(SdkError::InvalidArgument {
                message: "account is required".into(),
            });
        }
        self.replace_session(s);
        Ok(())
    }

    pub async fn stop_session(&self) -> Result<(), SdkError> {
        let _ = self.bump_epoch();
        *lock(&self.inner.session) = None;
        Ok(())
    }

    pub async fn switch_account(&self, account: String, token: String) -> Result<(), SdkError> {
        if account.is_empty() {
            return Err(SdkError::InvalidArgument {
                message: "account is required".into(),
            });
        }
        let _ = self.bump_epoch();
        self.update_account(account, token)
    }

    pub async fn enqueue_message(
        &self,
        cmd: SendMessageCommand,
    ) -> Result<CommandReceipt, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        store.enqueue(epoch, session.account, cmd).await
    }

    pub async fn load_older(&self, cursor: PageCursor) -> Result<MessagePage, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        store.load_older(&session.account, cursor).await
    }

    pub fn notify_radio_up(&self) -> Result<(), SdkError> {
        Ok(())
    }

    pub fn notify_foreground(&self) -> Result<(), SdkError> {
        Ok(())
    }

    fn store(&self) -> Result<Arc<Store>, SdkError> {
        lock(&self.inner.store)
            .clone()
            .ok_or(SdkError::InvalidArgument {
                message: "store not attached".into(),
            })
    }
}

fn migrate_blocking(path: PathBuf) -> Result<(), SdkError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| SdkError::Internal {
            message: format!("runtime: {e}"),
        })?;
    rt.block_on(store::migrate_path(path))
}
