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

pub use agent::{
    AgentPort, AgentProfileRow, AgentRunRequest, AgentRunResult, AgentRuntime, FfiAgentRuntime,
    MobileAgent, NoopAgent, ScriptedRuntime, QUEUE_CAP,
};
pub use command::{
    CommandReceipt, MessagePage, OutgoingPayload, PageCursor, ReadMarker, SendMessageCommand,
    SendStatus, StartSession, TimelineQuery,
};
pub use error::{map_client, SdkError};
pub use ids::{
    incoming_message_key, is_client_key, prefer_key, AccountId, ClientMessageId, DestId,
    SessionEpoch,
};
pub use media::{image_mime_ok, validate_media_path, MediaRef, MediaUploader, MAX_IMAGE_BYTES};
pub use metrics::SdkMetrics;
pub use proto::ProtocolClient;
pub use store::prepare::{prepare_store_file, PrepareOutcome};
pub use store::settings::DeviceSettings;
pub use sync::UnreadPolicy;
pub use timeline::{
    AgentCard, AgentTurnState, LinkStateView, MessageView, PersonRef, SessionSnapshot,
    SessionUpdate, ThreadView, TimelineDelta, TimelineSnapshot, TimelineUpdate,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenPersistEvent {
    Write { token: String },
    Clear,
}

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
    token_persist: Mutex<Vec<mpsc::Sender<TokenPersistEvent>>>,
    ffi_runtime: Arc<FfiAgentRuntime>,
    media_dir: Mutex<Option<PathBuf>>,
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
                token_persist: Mutex::new(Vec::new()),
                ffi_runtime: FfiAgentRuntime::new(),
                media_dir: Mutex::new(None),
            }),
        })
    }

    /// Test / CLI fixture: protocol_only + attach_store. Production bootstrap always attach_store.
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
        let wiped = tokio::task::spawn_blocking(move || migrate_blocking(migrate_path))
            .await
            .map_err(|e| SdkError::Internal {
                message: format!("join: {e}"),
            })??;
        if wiped {
            self.inner.metrics.inc_store_wipe();
        }
        let epoch = self.inner.epoch.clone();
        let store = Store::open(path.clone(), epoch).await?;
        *lock(&self.inner.store) = Some(store);
        if let Some(parent) = path.parent() {
            *lock(&self.inner.media_dir) = Some(parent.join("kim-media"));
        }
        Ok(())
    }

    pub fn store_wipe_total(&self) -> u64 {
        self.inner.metrics.store_wipe_total()
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
        // Subscribe before the reconnect loop so the first Link events are not missed.
        self.spawn_session_bridge(&sup);
        self.spawn_outbox_worker();
        sup.ensure_running();
        *lock(&self.inner.supervisor) = Some(Arc::new(sup));
        self.publish_session_snapshot().await;
        if let Ok(store) = self.store() {
            store
                .rekey_agent_profiles(String::new(), s.account.clone())
                .await?;
        }
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
        self.publish_timeline(&receipt.dest).await;
        self.publish_session_snapshot().await;
        self.kick_outbox();
        Ok(receipt)
    }

    pub async fn cancel_send(&self, id: String) -> Result<(), SdkError> {
        self.cancel_outbox_run();
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let dest = store.get_row(&session.account, &id).await?.map(|r| r.dest);
        store.cancel(epoch, session.account, id).await?;
        if let Some(dest) = dest {
            self.publish_timeline(&dest).await;
        }
        Ok(())
    }

    pub async fn retry_send(&self, id: String) -> Result<CommandReceipt, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        store
            .requeue(epoch, session.account.clone(), id.clone())
            .await?;
        let row =
            store
                .get_row(&session.account, &id)
                .await?
                .ok_or_else(|| SdkError::NotFound {
                    what: "outbox".into(),
                })?;
        self.publish_timeline(&row.dest).await;
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
        self.publish_session_snapshot().await;
        self.publish_timeline(&marker.dest).await;
        Ok(())
    }

    pub async fn delete_thread(&self, dest: String) -> Result<(), SdkError> {
        self.cancel_outbox_run();
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        store
            .delete_thread(epoch, session.account, dest.clone())
            .await?;
        self.publish_timeline_resync(&dest, "deleted").await;
        self.publish_session_snapshot().await;
        Ok(())
    }

    pub fn install_protocol(&self, protocol: Arc<dyn ProtocolClient>) {
        *lock(&self.inner.protocol) = Some(protocol);
    }

    pub fn set_agent(&self, agent: Arc<dyn AgentPort>) {
        *lock(&self.inner.agent) = agent;
    }

    /// Desktop only. Phone leaves [`NoopAgent`].
    pub fn install_mobile_agent(&self) {
        let runtime = self.inner.ffi_runtime.clone();
        self.set_agent(Arc::new(MobileAgent::new(self.clone(), runtime)));
    }

    pub fn subscribe_agent_run(&self) -> mpsc::Receiver<AgentRunRequest> {
        self.inner.ffi_runtime.subscribe()
    }

    pub fn submit_agent_run(&self, result: AgentRunResult) {
        self.inner.ffi_runtime.submit(result);
    }

    fn agent_account_key(&self) -> String {
        self.session_snapshot()
            .map(|s| s.account)
            .unwrap_or_default()
    }

    pub async fn upsert_agent_profile(&self, row: AgentProfileRow) -> Result<(), SdkError> {
        let store = self.store()?;
        store
            .upsert_agent_profile(self.agent_account_key(), row)
            .await
    }

    pub async fn delete_agent_profile(&self, profile_id: String) -> Result<(), SdkError> {
        let store = self.store()?;
        store
            .delete_agent_profile(self.agent_account_key(), profile_id)
            .await
    }

    pub async fn list_agent_profiles(&self) -> Result<Vec<AgentProfileRow>, SdkError> {
        let store = self.store()?;
        store.load_agent_profiles(&self.agent_account_key()).await
    }

    pub async fn import_agent_profiles(&self, rows: Vec<AgentProfileRow>) -> Result<(), SdkError> {
        let store = self.store()?;
        store
            .import_agent_profiles(self.agent_account_key(), rows)
            .await
    }

    pub async fn enqueue_agent_turn(
        &self,
        dest: String,
        text: String,
        in_reply_to: i64,
    ) -> Result<(), SdkError> {
        self.agent()
            .enqueue_turn(&dest, &text, in_reply_to, self.current_epoch())
            .await
    }

    pub async fn search_messages(
        &self,
        query: String,
        dest: Option<String>,
    ) -> Result<Vec<MessageView>, SdkError> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(vec![]);
        }
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let rows = store
            .search_messages(&session.account, q, dest.as_deref())
            .await?;
        Ok(rows
            .into_iter()
            .map(|(d, key, sender, body, at, message_id)| MessageView {
                key,
                dest: d,
                sender,
                body,
                local_path: None,
                at,
                sys: false,
                kind: 1,
                width: 0,
                height: 0,
                message_id,
                batch_id: None,
                send_status: SendStatus::Sent,
            })
            .collect())
    }

    pub fn current_session(&self) -> Result<StartSession, SdkError> {
        self.session_snapshot()
    }

    pub async fn upload_media(&self, media: MediaRef) -> Result<String, SdkError> {
        let session = self.session_snapshot()?;
        self.uploader()?.upload_image(&session.token, &media).await
    }

    pub async fn fetch_media(&self, url: String) -> Result<String, SdkError> {
        if url.trim().is_empty() {
            return Err(SdkError::InvalidArgument {
                message: "media url is required".into(),
            });
        }
        let store = self.store()?;
        if let Some(row) = store.lookup_media(&url).await? {
            if tokio::fs::metadata(&row.local_path).await.is_ok() {
                store.touch_media(url).await?;
                return Ok(row.local_path);
            }
        }
        let dir = lock(&self.inner.media_dir)
            .clone()
            .ok_or_else(|| SdkError::InvalidArgument {
                message: "media dir missing".into(),
            })?;
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| SdkError::Disk {
                message: e.to_string(),
            })?;
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .map_err(|e| SdkError::Internal {
                message: format!("reqwest: {e}"),
            })?;
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| SdkError::Internal {
                message: format!("fetch: {e}"),
            })?;
        if !resp.status().is_success() {
            return Err(SdkError::Protocol {
                status: i32::from(resp.status().as_u16()),
            });
        }
        let bytes = resp.bytes().await.map_err(|e| SdkError::Internal {
            message: format!("fetch body: {e}"),
        })?;
        let name = media_cache_name(&url);
        let path = dir.join(name);
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| SdkError::Disk {
                message: e.to_string(),
            })?;
        let size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
        let local = path.to_string_lossy().into_owned();
        let evicted = store.upsert_media(url, local.clone(), size).await?;
        for old in evicted {
            let _ = tokio::fs::remove_file(old).await;
        }
        Ok(local)
    }

    fn spawn_agent_catch_up(&self) {
        let epoch = self.current_epoch();
        let sdk = self.clone();
        tokio::spawn(async move {
            if sdk.current_epoch() != epoch {
                return;
            }
            sdk.maybe_agent_catch_up().await;
        });
    }

    async fn maybe_agent_catch_up(&self) {
        let epoch = self.current_epoch();
        let Ok(store) = self.store() else {
            return;
        };
        let Ok(session) = self.session_snapshot() else {
            return;
        };
        let dests = match store.agent_owned_dests(&session.account).await {
            Ok(d) => d,
            Err(_) => return,
        };
        if dests.is_empty() {
            return;
        }
        if self.current_epoch() != epoch {
            return;
        }
        let _ = self.agent().catch_up(&dests, epoch).await;
    }

    async fn recover_lagged_fatal(&self) {
        let Ok(sup) = self.supervisor() else {
            return;
        };
        match sup.last_drop_reason() {
            Some(kim_client::DropReason::Kickout) => {
                self.emit_session_wait(SessionUpdate::Kickout {
                    channel_id: "lagged".into(),
                })
                .await;
            }
            Some(kim_client::DropReason::AuthFailed) => {
                self.emit_session_wait(SessionUpdate::AuthExpired {
                    reason: "lagged".into(),
                })
                .await;
            }
            _ => {}
        }
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
        let dest = cursor.dest.clone();
        let limit = cursor.limit;
        let before_id = cursor.before_id;
        let mut page = store.load_older(&session.account, cursor.clone()).await?;
        if (page.messages.len() as i32) < limit && before_id != 0 {
            if let Ok(proto) = self.protocol() {
                let kind = store
                    .load_threads(&session.account)
                    .await
                    .ok()
                    .and_then(|ts| ts.into_iter().find(|t| t.id == dest).map(|t| t.kind))
                    .unwrap_or(0);
                if let Ok(remote) = proto.history(&dest, kind, before_id, limit).await {
                    let talks: Vec<kim_client::IncomingTalk> = remote
                        .into_iter()
                        .map(|h| kim_client::IncomingTalk {
                            command: String::new(),
                            dest: dest.clone(),
                            message_id: h.message_id,
                            sender: h.sender,
                            msg_type: h.msg_type,
                            body: h.body,
                            extra: h.extra,
                            send_time: h.send_time,
                        })
                        .collect();
                    if !talks.is_empty() {
                        let _ = self
                            .persist_talks_for(
                                self.current_epoch().0,
                                session.account.clone(),
                                talks,
                                UnreadPolicy::Keep,
                            )
                            .await;
                        if let Ok(again) = store.load_older(&session.account, cursor).await {
                            page = again;
                        }
                    }
                }
            }
        }
        self.publish_timeline(&dest).await;
        Ok(page)
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
        let dests: Vec<String> = {
            let mut d: Vec<String> = talks.iter().map(|t| t.dest.clone()).collect();
            d.sort();
            d.dedup();
            d
        };
        store.persist_talks(epoch, account, talks, policy).await?;
        self.inner.metrics.inc_persist_talk();
        for dest in dests {
            self.publish_timeline(&dest).await;
        }
        self.publish_session_snapshot().await;
        Ok(())
    }

    pub fn metrics(&self) -> (u64, u64, u64, u64) {
        self.inner.metrics.snapshot()
    }

    pub fn install_panic_hook(&self) {
        let inner = self.inner.clone();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info.to_string();
            if let Ok(subs) = inner.session_subs.try_lock() {
                for tx in subs.iter() {
                    let _ = tx.try_send(SessionUpdate::RustPanic {
                        message: message.clone(),
                    });
                }
            }
            previous(info);
        }));
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
        self.publish_session_snapshot().await;
        self.emit_session_wait(SessionUpdate::Inbox {
            threads: views.clone(),
        })
        .await;
        Ok(views)
    }

    pub async fn load_threads(&self) -> Result<Vec<ThreadView>, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        store.load_threads(&session.account).await
    }

    pub async fn replace_contacts(&self, rows: Vec<PersonRef>) -> Result<(), SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        store
            .replace_contacts(session.account, rows.clone())
            .await?;
        self.emit_session_wait(SessionUpdate::ContactsChanged { contacts: rows })
            .await;
        Ok(())
    }

    pub async fn load_contacts(&self) -> Result<Vec<PersonRef>, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        store.load_contacts(&session.account).await
    }

    pub async fn settings_get(&self) -> Result<DeviceSettings, SdkError> {
        let mut row = match self.store() {
            Ok(store) => store.load_device_settings().await?,
            Err(_) => DeviceSettings::default(),
        };
        if let Ok(session) = self.session_snapshot() {
            row.account = session.account;
        }
        Ok(row)
    }

    pub async fn settings_patch(
        &self,
        ws_url: Option<String>,
        http_origin: Option<String>,
        env: Option<String>,
        locale: Option<String>,
    ) -> Result<DeviceSettings, SdkError> {
        let store = self.store()?;
        let mut row = store.load_device_settings().await?;
        if let Some(v) = ws_url {
            row.ws_url = v;
        }
        if let Some(v) = http_origin {
            row.http_origin = v;
        }
        if let Some(v) = env {
            row.env = v;
        }
        if let Some(v) = locale {
            row.locale = v;
        }
        store.upsert_device_settings(row, false).await?;
        self.settings_get().await
    }

    pub async fn import_device_settings(
        &self,
        ws_url: String,
        http_origin: String,
        env: String,
        locale: String,
    ) -> Result<DeviceSettings, SdkError> {
        let store = self.store()?;
        if store.prefs_imported().await? {
            return self.settings_get().await;
        }
        let row = DeviceSettings {
            ws_url,
            http_origin,
            env: if env.is_empty() { "prod".into() } else { env },
            locale,
            account: String::new(),
        };
        store.upsert_device_settings(row, true).await?;
        self.settings_get().await
    }

    pub fn subscribe_token_persist(&self) -> mpsc::Receiver<TokenPersistEvent> {
        let (tx, rx) = mpsc::channel(8);
        lock(&self.inner.token_persist).push(tx);
        rx
    }

    async fn publish_token_persist(&self, event: TokenPersistEvent) {
        let subs = lock(&self.inner.token_persist).clone();
        for tx in subs {
            let _ = tx.send(event.clone()).await;
        }
        lock(&self.inner.token_persist).retain(|tx| !tx.is_closed());
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

    pub(crate) async fn emit_session_wait(&self, update: SessionUpdate) {
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
                                    if matches!(update, SessionUpdate::Link { .. }) {
                                        sdk.publish_session_snapshot().await;
                                    }
                                    match &update {
                                        SessionUpdate::TokenRenew { token, .. } => {
                                            sdk.publish_token_persist(TokenPersistEvent::Write {
                                                token: token.clone(),
                                            })
                                            .await;
                                        }
                                        SessionUpdate::AuthExpired { .. } => {
                                            sdk.publish_token_persist(TokenPersistEvent::Clear)
                                                .await;
                                        }
                                        SessionUpdate::Link {
                                            state: LinkStateView::Online,
                                            ..
                                        }
                                        | SessionUpdate::SyncProgress {
                                            catching_up: false,
                                            ..
                                        } => {
                                            sdk.spawn_agent_catch_up();
                                        }
                                        _ => {}
                                    }
                                    sdk.emit_session_wait(update).await;
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                sdk.publish_session_snapshot().await;
                                sdk.recover_lagged_fatal().await;
                            }
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
        let dest = query.dest.clone();
        let limit = if query.limit <= 0 { 50 } else { query.limit };
        let init = TimelineUpdate::Snapshot {
            snapshot: TimelineSnapshot {
                dest: dest.clone(),
                version: 0,
                messages: Vec::new(),
                pending: Vec::new(),
                unread: 0,
                last_read_message_id: 0,
                has_more: false,
            },
        };
        let mut map = lock(&self.inner.timelines);
        let tx = map
            .entry(dest.clone())
            .or_insert_with(|| watch::channel(init).0)
            .clone();
        drop(map);
        let sdk = self.clone();
        // FRB sync watchers are not inside a Tokio context unless the FFI
        // layer enters one first. Skip the eager load rather than panic.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                sdk.publish_timeline_limit(&dest, limit).await;
            });
        }
        tx.subscribe()
    }

    async fn publish_session_snapshot(&self) {
        let (link, last_error) = match self.supervisor() {
            Ok(sup) => {
                let link = match sup.state() {
                    kim_client::LinkState::Connecting => LinkStateView::Connecting,
                    kim_client::LinkState::Online => LinkStateView::Online,
                    kim_client::LinkState::Reconnecting { attempt } => {
                        LinkStateView::Reconnecting { attempt }
                    }
                    kim_client::LinkState::Offline => LinkStateView::Offline,
                };
                let last_error = sup.last_drop_reason().map(|r| r.as_str().to_string());
                (link, last_error)
            }
            Err(_) => (LinkStateView::Offline, None),
        };
        let threads = match (self.store(), self.session_snapshot()) {
            (Ok(store), Ok(session)) => store
                .load_threads(&session.account)
                .await
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let unread_total = threads
            .iter()
            .map(|t| i64::from(t.unread.max(0)))
            .sum::<i64>()
            .clamp(0, i64::from(i32::MAX)) as i32;
        let _ = self.inner.session_snapshot.send(SessionSnapshot {
            link,
            last_error,
            threads,
            unread_total,
        });
    }

    pub(crate) async fn publish_timeline(&self, dest: &str) {
        self.publish_timeline_limit(dest, 50).await;
    }

    async fn publish_timeline_limit(&self, dest: &str, limit: i32) {
        let Ok(store) = self.store() else {
            return;
        };
        let Ok(session) = self.session_snapshot() else {
            return;
        };
        let Ok(snapshot) = store.load_hot_window(&session.account, dest, limit).await else {
            return;
        };
        if let Some(tx) = lock(&self.inner.timelines).get(dest) {
            let _ = tx.send(TimelineUpdate::Snapshot { snapshot });
        }
    }

    async fn publish_timeline_resync(&self, dest: &str, reason: &str) {
        if let Some(tx) = lock(&self.inner.timelines).get(dest) {
            let _ = tx.send(TimelineUpdate::Resync {
                dest: dest.into(),
                reason: reason.into(),
            });
        }
        self.publish_timeline(dest).await;
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
        kim_client::SessionEvent::ProfileUpdated { profile } => {
            Some(SessionUpdate::ProfileUpdated {
                account: profile.account,
                nickname: profile.nickname,
                avatar: profile.avatar,
            })
        }
        kim_client::SessionEvent::PresenceUpdated {
            account,
            status,
            last_seen,
        } => Some(SessionUpdate::Presence {
            account,
            status,
            last_seen,
        }),
        kim_client::SessionEvent::TypingUpdated {
            typer,
            dest,
            kind,
            active,
        } => Some(SessionUpdate::Typing {
            typer,
            dest,
            kind,
            active,
        }),
        kim_client::SessionEvent::ReceiptRead {
            reader,
            dest,
            kind,
            message_id,
        } => Some(SessionUpdate::ReceiptRead {
            reader,
            dest,
            kind,
            message_id,
        }),
        kim_client::SessionEvent::GroupCreate { group_id, members } => {
            Some(SessionUpdate::GroupCreate { group_id, members })
        }
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

fn migrate_blocking(path: PathBuf) -> Result<bool, SdkError> {
    let outcome = prepare_store_file(&path)?;
    let wiped = matches!(outcome, PrepareOutcome::CreateEmpty { wiped: true });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| SdkError::Internal {
            message: format!("runtime: {e}"),
        })?;
    rt.block_on(store::migrate_path(path))?;
    Ok(wiped)
}

fn media_cache_name(url: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:016x}.bin", hasher.finish())
}
