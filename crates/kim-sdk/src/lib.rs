//! Deep module for local message lifecycle: store, outbox, unread, persist-then-ack.

mod agent;
mod command;
mod error;
mod ids;
mod media;
mod metrics;
mod proto;
mod session;
mod store;
mod timeline;

pub mod outbox;
pub mod sync;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, watch};
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
pub use media::{image_mime_ok, validate_media_path, MediaRef, MediaUploader, MAX_IMAGE_BYTES};
pub use metrics::SdkMetrics;
pub use proto::ProtocolClient;
pub use sync::UnreadPolicy;
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
    supervisor: Mutex<Option<Arc<kim_client::SessionSupervisor>>>,
    protocol: Mutex<Option<Arc<dyn ProtocolClient>>>,
    uploader: Mutex<Option<Arc<MediaUploader>>>,
    session_subs: Mutex<Vec<mpsc::Sender<SessionUpdate>>>,
    timelines: Mutex<HashMap<String, watch::Sender<TimelineUpdate>>>,
    session_snapshot: watch::Sender<SessionSnapshot>,
    agent: Mutex<Arc<dyn AgentPort>>,
    metrics: SdkMetrics,
}

#[derive(Clone)]
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
                supervisor: Mutex::new(None),
                protocol: Mutex::new(None),
                uploader: Mutex::new(None),
                session_subs: Mutex::new(Vec::new()),
                timelines: Mutex::new(HashMap::new()),
                session_snapshot: watch::channel(SessionSnapshot::default()).0,
                agent: Mutex::new(Arc::new(NoopAgent)),
                metrics: SdkMetrics::default(),
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
        self.stop_supervisor();
        self.replace_session(s.clone());
        let cfg = kim_client::ClientConfig::new(s.url, s.token)
            .with_user_agent(s.user_agent)
            .with_device(kim_client::device_for_target_os(std::env::consts::OS).to_string());
        let mut sup = kim_client::SessionSupervisor::new(cfg);
        if self.store_attached() {
            sup = sup.with_persist(Arc::new(SdkPersistHook { sdk: self.clone() }));
        }
        *lock(&self.inner.supervisor) = Some(Arc::new(sup));
        Ok(())
    }

    pub async fn stop_session(&self) -> Result<(), SdkError> {
        let _ = self.bump_epoch();
        self.stop_supervisor();
        *lock(&self.inner.session) = None;
        Ok(())
    }

    pub async fn switch_account(&self, account: String, token: String) -> Result<(), SdkError> {
        if account.is_empty() {
            return Err(SdkError::InvalidArgument {
                message: "account is required".into(),
            });
        }
        let snap = self.session_snapshot()?;
        self.start_session(StartSession {
            url: snap.url,
            token,
            user_agent: snap.user_agent,
            account,
        })
        .await
    }

    pub fn supervisor(&self) -> Result<Arc<kim_client::SessionSupervisor>, SdkError> {
        lock(&self.inner.supervisor)
            .clone()
            .ok_or(SdkError::NotConnected)
    }

    pub fn store_attached(&self) -> bool {
        lock(&self.inner.store).is_some()
    }

    fn stop_supervisor(&self) {
        if let Some(sup) = lock(&self.inner.supervisor).take() {
            sup.stop();
        }
    }

    pub async fn enqueue_message(
        &self,
        cmd: SendMessageCommand,
    ) -> Result<CommandReceipt, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let receipt = store.enqueue(epoch, session.account, cmd).await?;
        self.inner.metrics.inc_enqueue();
        tracing::debug!(
            request_id = %receipt.request_id,
            client_id = %receipt.client_id,
            "enqueue committed"
        );
        let sdk = self.clone();
        tokio::spawn(async move {
            let _ = outbox::pump::run_once(&sdk).await;
        });
        Ok(receipt)
    }

    pub async fn cancel_send(&self, id: String) -> Result<(), SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        store.cancel(epoch, session.account, id).await
    }

    pub async fn retry_send(&self, id: String) -> Result<CommandReceipt, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let due = store.load_due(&session.account).await?;
        let row =
            due.into_iter()
                .find(|r| r.client_id == id)
                .ok_or_else(|| SdkError::NotFound {
                    what: "outbox".into(),
                })?;
        let _ = outbox::pump::run_once(self).await;
        Ok(CommandReceipt {
            request_id: uuid::Uuid::new_v4().to_string(),
            client_id: row.client_id,
            dest: row.dest,
            accepted_at: store::now_ms(),
            send_status: SendStatus::Pending,
        })
    }

    pub async fn delete_thread(&self, dest: String) -> Result<(), SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        store.delete_thread(epoch, session.account, dest).await
    }

    pub fn install_protocol(&self, protocol: Arc<dyn ProtocolClient>) {
        *lock(&self.inner.protocol) = Some(protocol);
    }

    pub fn set_agent(&self, agent: Arc<dyn AgentPort>) {
        *lock(&self.inner.agent) = agent;
    }

    pub(crate) fn agent(&self) -> Arc<dyn AgentPort> {
        lock(&self.inner.agent).clone()
    }

    pub fn set_upload_origin(&self, origin: String) -> Result<(), SdkError> {
        *lock(&self.inner.uploader) = Some(Arc::new(MediaUploader::new(origin)?));
        Ok(())
    }

    pub(crate) fn uploader(&self) -> Result<Arc<MediaUploader>, SdkError> {
        let mut g = lock(&self.inner.uploader);
        if let Some(u) = g.as_ref() {
            return Ok(u.clone());
        }
        let u = Arc::new(MediaUploader::new(media::UPLOAD_ORIGIN)?);
        *g = Some(u.clone());
        Ok(u)
    }

    pub(crate) fn protocol(&self) -> Result<Arc<dyn ProtocolClient>, SdkError> {
        if let Some(p) = lock(&self.inner.protocol).clone() {
            return Ok(p);
        }
        Err(SdkError::NotConnected)
    }

    pub async fn load_older(&self, cursor: PageCursor) -> Result<MessagePage, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        store.load_older(&session.account, cursor).await
    }

    pub async fn persist_talks(
        &self,
        talks: Vec<kim_client::IncomingTalk>,
        policy: UnreadPolicy,
    ) -> Result<(), SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        store
            .persist_talks(epoch, session.account, talks, policy)
            .await?;
        self.inner.metrics.inc_persist_talk();
        Ok(())
    }

    pub fn metrics(&self) -> (u64, u64, u64) {
        self.inner.metrics.snapshot()
    }

    pub async fn persist_inbox(
        &self,
        items: Vec<kim_client::InboxItem>,
    ) -> Result<Vec<ThreadView>, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let views = store.persist_inbox(epoch, session.account, items).await?;
        self.emit_session(SessionUpdate::Inbox {
            threads: views.clone(),
        });
        Ok(views)
    }

    pub async fn load_threads(&self) -> Result<Vec<ThreadView>, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        store.load_threads(&session.account).await
    }

    /// Discrete session events: bounded mpsc, every Kickout/token/friend is delivered.
    pub fn subscribe_session(&self) -> mpsc::Receiver<SessionUpdate> {
        let (tx, rx) = mpsc::channel(64);
        lock(&self.inner.session_subs).push(tx);
        rx
    }

    pub fn emit_session(&self, update: SessionUpdate) {
        let mut subs = lock(&self.inner.session_subs);
        subs.retain(|tx| tx.try_send(update.clone()).is_ok());
    }

    pub fn subscribe_session_snapshot(&self) -> watch::Receiver<SessionSnapshot> {
        self.inner.session_snapshot.subscribe()
    }

    pub fn subscribe_timeline(&self, query: TimelineQuery) -> watch::Receiver<TimelineUpdate> {
        let dest = if query.dest.is_empty() {
            String::new()
        } else {
            query.dest.clone()
        };
        let init = TimelineUpdate::Resync {
            dest: dest.clone(),
            reason: "subscribe".into(),
        };
        let mut map = lock(&self.inner.timelines);
        let tx = map
            .entry(dest)
            .or_insert_with(|| watch::channel(init).0)
            .clone();
        tx.subscribe()
    }

    pub fn notify_radio_up(&self) -> Result<(), SdkError> {
        self.supervisor()?.notify_radio_up();
        Ok(())
    }

    pub fn notify_foreground(&self) -> Result<(), SdkError> {
        self.supervisor()?.notify_foreground();
        Ok(())
    }

    pub(crate) fn store(&self) -> Result<Arc<Store>, SdkError> {
        lock(&self.inner.store)
            .clone()
            .ok_or(SdkError::InvalidArgument {
                message: "store not attached".into(),
            })
    }
}

struct SdkPersistHook {
    sdk: KimSdk,
}

#[async_trait::async_trait]
impl kim_client::PersistHook for SdkPersistHook {
    async fn persist_talks(
        &self,
        talks: &[kim_client::IncomingTalk],
        policy: kim_client::UnreadPolicy,
    ) -> Result<(), kim_client::PersistError> {
        let mapped = match policy {
            kim_client::UnreadPolicy::Keep => UnreadPolicy::Keep,
            kim_client::UnreadPolicy::IfInserted => UnreadPolicy::IfInserted,
        };
        self.sdk
            .persist_talks(talks.to_vec(), mapped)
            .await
            .map_err(sdk_to_persist)
    }

    async fn persist_inbox(
        &self,
        items: &[kim_client::InboxItem],
    ) -> Result<(), kim_client::PersistError> {
        self.sdk
            .persist_inbox(items.to_vec())
            .await
            .map(|_| ())
            .map_err(sdk_to_persist)
    }
}

fn sdk_to_persist(err: SdkError) -> kim_client::PersistError {
    match err {
        SdkError::Busy { .. } | SdkError::SqliteBusy => kim_client::PersistError::Busy,
        SdkError::StorageFull => kim_client::PersistError::StorageFull,
        SdkError::StaleEpoch { .. } => kim_client::PersistError::StaleEpoch,
        SdkError::Disk { message } => kim_client::PersistError::Disk { message },
        other => kim_client::PersistError::Disk {
            message: other.to_string(),
        },
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
