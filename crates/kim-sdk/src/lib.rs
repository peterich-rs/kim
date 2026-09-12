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
    outbox_kick: Mutex<Option<mpsc::Sender<()>>>,
    outbox_run: Mutex<CancellationToken>,
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
                outbox_kick: Mutex::new(None),
                outbox_run: Mutex::new(CancellationToken::new()),
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
        let _ = self.bump_epoch();
        self.stop_supervisor();
        *lock(&self.inner.outbox_kick) = None;
        self.replace_session(s.clone());
        let cfg = kim_client::ClientConfig::new(s.url.clone(), s.token.clone())
            .with_user_agent(s.user_agent.clone())
            .with_device(kim_client::device_for_target_os(std::env::consts::OS).to_string());
        let mut sup = kim_client::SessionSupervisor::new(cfg);
        let epoch = self.current_epoch().0;
        if self.store_attached() {
            sup = sup.with_persist(Arc::new(SdkPersistHook {
                sdk: self.clone(),
                account: s.account.clone(),
                epoch,
            }));
        }
        self.install_protocol(sup.client());
        self.spawn_session_bridge(&sup);
        self.spawn_outbox_worker();
        *lock(&self.inner.supervisor) = Some(Arc::new(sup));
        Ok(())
    }

    pub async fn stop_session(&self) -> Result<(), SdkError> {
        let _ = self.bump_epoch();
        self.cancel_outbox_run();
        self.stop_supervisor();
        *lock(&self.inner.outbox_kick) = None;
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
        self.kick_outbox();
        Ok(receipt)
    }

    pub async fn cancel_send(&self, id: String) -> Result<(), SdkError> {
        self.cancel_outbox_run();
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        store.cancel(epoch, session.account, id).await
    }

    pub async fn retry_send(&self, id: String) -> Result<CommandReceipt, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        store
            .requeue(epoch, session.account.clone(), id.clone())
            .await?;
        let row = store
            .get_row(&session.account, &id)
            .await?
            .ok_or_else(|| SdkError::NotFound {
                what: "outbox".into(),
            })?;
        self.kick_outbox();
        Ok(CommandReceipt {
            request_id: uuid::Uuid::new_v4().to_string(),
            client_id: row.client_id,
            dest: row.dest,
            accepted_at: store::now_ms(),
            send_status: SendStatus::Pending,
        })
    }

    pub async fn mark_read(&self, marker: ReadMarker) -> Result<(), SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        store
            .mark_read_local(
                epoch,
                session.account,
                marker.dest.clone(),
                marker.visible_message_id,
            )
            .await?;
        if let Ok(proto) = self.protocol() {
            let _ = proto
                .mark_read(&marker.dest, marker.kind, marker.visible_message_id)
                .await;
        }
        Ok(())
    }

    pub async fn delete_thread(&self, dest: String) -> Result<(), SdkError> {
        self.cancel_outbox_run();
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
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        self.persist_talks_for(epoch, session.account, talks, policy)
            .await
    }

    pub(crate) async fn persist_talks_for(
        &self,
        epoch: u64,
        account: String,
        talks: Vec<kim_client::IncomingTalk>,
        policy: UnreadPolicy,
    ) -> Result<(), SdkError> {
        let store = self.store()?;
        store.persist_talks(epoch, account, talks, policy).await?;
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
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        self.persist_inbox_for(epoch, session.account, items).await
    }

    pub(crate) async fn persist_inbox_for(
        &self,
        epoch: u64,
        account: String,
        items: Vec<kim_client::InboxItem>,
    ) -> Result<Vec<ThreadView>, SdkError> {
        let store = self.store()?;
        let views = store.persist_inbox(epoch, account, items).await?;
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
        subs.retain(|tx| match tx.try_send(update.clone()) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => true,
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        });
    }

    async fn emit_session_wait(&self, update: SessionUpdate) {
        let subs = lock(&self.inner.session_subs).clone();
        for tx in subs {
            let _ = tx.send(update.clone()).await;
        }
        lock(&self.inner.session_subs).retain(|tx| !tx.is_closed());
    }

    fn spawn_session_bridge(&self, sup: &kim_client::SessionSupervisor) {
        let mut rx = sup.events();
        let sdk = self.clone();
        let cancel = self.child_token();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    ev = rx.recv() => {
                        match ev {
                            Ok(ev) => {
                                if let Some(update) = session_update_from_event(ev) {
                                    sdk.emit_session_wait(update).await;
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });
    }

    fn spawn_outbox_worker(&self) {
        if !self.store_attached() {
            return;
        }
        let (tx, mut rx) = mpsc::channel::<()>(8);
        *lock(&self.inner.outbox_kick) = Some(tx);
        let sdk = self.clone();
        let parent = lock(&self.inner.cancel).clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = parent.cancelled() => break,
                    msg = rx.recv() => {
                        if msg.is_none() {
                            break;
                        }
                    }
                }
                let mut prev_due: Option<Vec<String>> = None;
                loop {
                    if parent.is_cancelled() {
                        return;
                    }
                    let run = parent.child_token();
                    *lock(&sdk.inner.outbox_run) = run.clone();
                    if let Err(SdkError::StaleEpoch { .. }) =
                        outbox::pump::run_once(&sdk, &run).await
                    {
                        sdk.metrics_inc_epoch_drop();
                    }
                    let mut again = false;
                    while rx.try_recv().is_ok() {
                        again = true;
                    }
                    let due = outbox_due_ids(&sdk).await;
                    if !again {
                        if due.is_empty() {
                            break;
                        }
                        if prev_due.as_ref() == Some(&due) {
                            break;
                        }
                        again = true;
                    }
                    prev_due = Some(due);
                    if !again {
                        break;
                    }
                }
            }
        });
        self.kick_outbox();
    }

    fn kick_outbox(&self) {
        if let Some(tx) = lock(&self.inner.outbox_kick).as_ref() {
            let _ = tx.try_send(());
        }
    }

    fn cancel_outbox_run(&self) {
        lock(&self.inner.outbox_run).cancel();
    }

    pub(crate) fn metrics_inc_epoch_drop(&self) {
        self.inner.metrics.inc_epoch_drop();
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

    pub async fn notify_radio_up(&self) -> Result<(), SdkError> {
        self.supervisor()?.notify_radio_up();
        self.wake_outbox().await
    }

    pub async fn notify_foreground(&self) -> Result<(), SdkError> {
        self.supervisor()?.notify_foreground();
        self.wake_outbox().await
    }

    async fn wake_outbox(&self) -> Result<(), SdkError> {
        if let (Ok(store), Ok(session)) = (self.store(), self.session_snapshot()) {
            let epoch = self.current_epoch().0;
            store.due_now(epoch, session.account).await?;
        }
        self.kick_outbox();
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
    account: String,
    epoch: u64,
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
            .persist_talks_for(self.epoch, self.account.clone(), talks.to_vec(), mapped)
            .await
            .map_err(sdk_to_persist)
    }

    async fn persist_inbox(
        &self,
        items: &[kim_client::InboxItem],
    ) -> Result<(), kim_client::PersistError> {
        self.sdk
            .persist_inbox_for(self.epoch, self.account.clone(), items.to_vec())
            .await
            .map(|_| ())
            .map_err(sdk_to_persist)
    }
}

async fn outbox_due_ids(sdk: &KimSdk) -> Vec<String> {
    let Ok(store) = sdk.store() else {
        return Vec::new();
    };
    let Ok(session) = sdk.session_snapshot() else {
        return Vec::new();
    };
    match store.load_due(&session.account).await {
        Ok(rows) => rows.into_iter().map(|r| r.client_id).collect(),
        Err(_) => Vec::new(),
    }
}

fn session_update_from_event(ev: kim_client::SessionEvent) -> Option<SessionUpdate> {
    match ev {
        kim_client::SessionEvent::Kickout { channel_id } => {
            Some(SessionUpdate::Kickout { channel_id })
        }
        kim_client::SessionEvent::TokenRenew { token, exp } => {
            Some(SessionUpdate::TokenRenew { token, exp })
        }
        kim_client::SessionEvent::FriendRequest { from, nickname } => {
            Some(SessionUpdate::FriendRequest { from, nickname })
        }
        kim_client::SessionEvent::FriendAccepted { from, nickname } => {
            Some(SessionUpdate::FriendAccepted { from, nickname })
        }
        kim_client::SessionEvent::AuthFailed { reason } => {
            Some(SessionUpdate::AuthExpired { reason })
        }
        kim_client::SessionEvent::Link(state) => Some(SessionUpdate::Link {
            state: match state {
                kim_client::LinkState::Connecting => LinkStateView::Connecting,
                kim_client::LinkState::Online => LinkStateView::Online,
                kim_client::LinkState::Reconnecting { attempt } => {
                    LinkStateView::Reconnecting { attempt }
                }
                kim_client::LinkState::Offline => LinkStateView::Offline,
            },
            last_error: None,
        }),
        kim_client::SessionEvent::SyncProgress {
            pulled,
            page_pending,
        } => Some(SessionUpdate::SyncProgress {
            pulled: pulled as u64,
            catching_up: page_pending,
        }),
        _ => None,
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
