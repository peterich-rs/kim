//! Deep module for local message lifecycle: store, outbox, unread, persist-then-ack.

mod agent;
mod command;
mod contacts;
mod error;
mod ids;
mod media;
mod metrics;
mod proto;
mod query;
mod read_sync;
mod session;
mod store;
mod timeline;

pub mod outbox;
pub mod sync;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

pub use agent::{
    AgentPort, AgentProfileRow, AgentRunRequest, AgentRunResult, AgentRuntime, DeviceOverlayRow,
    FfiAgentRuntime, MobileAgent, NoopAgent, ProviderAccountRow, ScriptedRuntime, QUEUE_CAP,
};
pub use command::{
    CommandReceipt, ConversationKey, ConversationVisibility, MessagePage, OutgoingPayload,
    PageCursor, ReadMarker, SendMessageCommand, SendStatus, StartSession, TimelineQuery,
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
    AgentCard, AgentTurnState, ContactsSnapshot, LinkStateView, MessageView, PersonRef,
    SessionSnapshot, SessionUpdate, ThreadView, TimelineDelta, TimelineSnapshot, TimelineUpdate,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HydrateOutcome {
    Skipped,
    InFlight,
    Applied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenPersistEvent {
    Write { token: String },
    Clear,
}

use crate::contacts::ContactsErrorOp;
use crate::query::{spawn_query_publisher, TimelineSub};
use crate::session::lock;
use crate::store::changes::{ChangeLog, CommitEffect};
use crate::store::Store;

const APPLY_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

pub(crate) struct Inner {
    epoch: Arc<AtomicU64>,
    cancel: Mutex<CancellationToken>,
    session: Mutex<Option<StartSession>>,
    store: Mutex<Option<Arc<Store>>>,
    supervisor: Mutex<Option<Arc<kim_client::SessionSupervisor>>>,
    protocol: Mutex<Option<Arc<dyn ProtocolClient>>>,
    uploader: Mutex<Option<Arc<MediaUploader>>>,
    session_subs: Mutex<Vec<mpsc::Sender<SessionUpdate>>>,
    timelines: Mutex<HashMap<String, TimelineSub>>,
    contacts_watch: watch::Sender<ContactsSnapshot>,
    contacts_account: Mutex<String>,
    contacts_epoch: AtomicU64,
    contacts_version: AtomicU64,
    /// Source of truth for `ContactsSnapshot.sync_error`. Mutated only while
    /// publishing a snapshot so a QueryPublisher rebuild cannot clobber a
    /// failed refresh.
    contacts_sync_error: Mutex<Option<String>>,
    changes: Mutex<Option<Arc<ChangeLog>>>,
    /// Only cancelled when the last `KimSdk` is dropped; session reconnects
    /// must not stop the query publisher.
    store_life: CancellationToken,
    session_snapshot: watch::Sender<SessionSnapshot>,
    agent: Mutex<Arc<dyn AgentPort>>,
    metrics: SdkMetrics,
    outbox_kick: Mutex<Option<mpsc::Sender<()>>>,
    outbox_run: Mutex<CancellationToken>,
    read_sync_kick: Mutex<Option<mpsc::Sender<()>>>,
    token_persist: Mutex<Vec<mpsc::Sender<TokenPersistEvent>>>,
    ffi_runtime: Arc<FfiAgentRuntime>,
    media_dir: Mutex<Option<PathBuf>>,
    /// Profiles that permanently failed `agent_spec_upsert` this process.
    /// Prevents a bad local row from hammering the wire on every sync.
    agent_spec_push_skip: Mutex<HashSet<String>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.store_life.cancel();
    }
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
                contacts_watch: watch::channel(ContactsSnapshot {
                    version: 0,
                    contacts: Vec::new(),
                    sync_error: None,
                })
                .0,
                contacts_account: Mutex::new(String::new()),
                contacts_epoch: AtomicU64::new(0),
                contacts_version: AtomicU64::new(0),
                contacts_sync_error: Mutex::new(None),
                changes: Mutex::new(None),
                store_life: CancellationToken::new(),
                session_snapshot: watch::channel(SessionSnapshot::default()).0,
                agent: Mutex::new(Arc::new(NoopAgent)),
                metrics: SdkMetrics::default(),
                outbox_kick: Mutex::new(None),
                outbox_run: Mutex::new(CancellationToken::new()),
                read_sync_kick: Mutex::new(None),
                token_persist: Mutex::new(Vec::new()),
                ffi_runtime: FfiAgentRuntime::new(),
                media_dir: Mutex::new(None),
                agent_spec_push_skip: Mutex::new(HashSet::new()),
            }),
        })
    }

    /// Test / CLI fixture: protocol_only + attach_store. Production bootstrap always attach_store.
    pub async fn open(db_path: String) -> Result<Arc<Self>, SdkError> {
        let sdk = Self::protocol_only();
        sdk.attach_store(db_path).await?;
        Ok(sdk)
    }

    pub(crate) fn downgrade(&self) -> std::sync::Weak<Inner> {
        Arc::downgrade(&self.inner)
    }

    pub(crate) fn from_inner(inner: Arc<Inner>) -> Self {
        Self { inner }
    }

    #[cfg(test)]
    pub(crate) fn debug_inner_strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
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
            tracing::warn!(path = %path.display(), "store wiped");
        }
        let epoch = self.inner.epoch.clone();
        let store = Store::open(path.clone(), epoch).await?;
        let changes = store.changes();
        *lock(&self.inner.store) = Some(store);
        *lock(&self.inner.changes) = Some(changes.clone());
        spawn_query_publisher(self.clone(), changes, self.inner.store_life.clone());
        if let Some(parent) = path.parent() {
            *lock(&self.inner.media_dir) = Some(parent.join("kim-media"));
        }
        tracing::info!(path = %path.display(), wiped, "store attached");
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
        let prev_account = lock(&self.inner.session)
            .as_ref()
            .map(|session| session.account.clone());
        let _ = self.bump_epoch();
        self.stop_supervisor();
        *lock(&self.inner.outbox_kick) = None;
        self.replace_session(s.clone());
        let epoch = self.current_epoch().0;
        let same_account = matches!(prev_account.as_deref(), None | Some(""))
            || prev_account.as_deref() == Some(s.account.as_str());
        {
            let mut timelines = lock(&self.inner.timelines);
            if same_account {
                for sub in timelines.values_mut() {
                    if sub.account.is_empty() || sub.account == s.account {
                        sub.account = s.account.clone();
                        sub.epoch = epoch;
                    }
                }
            } else {
                for sub in timelines.values() {
                    let _ = sub.tx.send(TimelineUpdate::Resync {
                        dest: sub.dest.clone(),
                        reason: "account".into(),
                    });
                }
            }
        }
        let contacts_active = !self.inner.contacts_watch.is_closed();
        *lock(&self.inner.contacts_account) = s.account.clone();
        self.inner.contacts_epoch.store(epoch, Ordering::SeqCst);
        if !same_account {
            self.publish_contacts_snapshot(Vec::new(), ContactsErrorOp::Clear);
        }
        if same_account {
            if let Some(changes) = lock(&self.inner.changes).clone() {
                let dests: Vec<String> = lock(&self.inner.timelines).keys().cloned().collect();
                let mut effect = CommitEffect::inbox();
                for dest in dests {
                    effect.merge(CommitEffect::timeline(dest));
                }
                if contacts_active {
                    effect.merge(CommitEffect::contacts());
                }
                changes.record(s.account.clone(), epoch, effect);
            }
        } else if contacts_active {
            if let Some(changes) = lock(&self.inner.changes).clone() {
                changes.record(s.account.clone(), epoch, CommitEffect::contacts());
            }
        }
        let cfg = kim_client::ClientConfig::new(s.url.clone(), s.token.clone())
            .with_user_agent(s.user_agent.clone())
            .with_device(kim_client::device_for_target_os(std::env::consts::OS).to_string());
        let mut sup = kim_client::SessionSupervisor::new(cfg);
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
        self.spawn_read_sync_worker();
        sup.ensure_running();
        *lock(&self.inner.supervisor) = Some(Arc::new(sup));
        self.refresh_session_snapshot().await;
        if let Ok(store) = self.store() {
            let ((), _seq) = store
                .rekey_agent_profiles(String::new(), s.account.clone())
                .await?;
        }
        tracing::info!(account = %s.account, url = %s.url, "session start");
        Ok(())
    }

    pub async fn stop_session(&self) -> Result<(), SdkError> {
        tracing::info!("session stop");
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
        let (receipt, sequence) = store.enqueue(epoch, session.account, cmd).await?;
        self.inner.metrics.inc_enqueue();
        tracing::info!(
            request_id = %receipt.request_id,
            client_id = %receipt.client_id,
            dest = %receipt.dest,
            "enqueue committed"
        );
        self.after_command(sequence).await;
        self.kick_outbox();
        Ok(receipt)
    }

    /// Local-first bot reply: show the line immediately, then `chat.bot.reply`.
    pub async fn enqueue_bot_reply(
        &self,
        dest: &str,
        body: &str,
        in_reply_to: i64,
        client_id: &str,
    ) -> Result<CommandReceipt, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let (receipt, sequence) = store
            .enqueue_bot_reply(
                epoch,
                session.account,
                dest.to_string(),
                body.to_string(),
                in_reply_to,
                client_id.to_string(),
            )
            .await?;
        self.inner.metrics.inc_enqueue();
        self.after_command(sequence).await;
        self.kick_outbox();
        Ok(receipt)
    }

    pub async fn cancel_send(&self, id: String) -> Result<(), SdkError> {
        self.cancel_outbox_run();
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let ((), sequence) = store.cancel(epoch, session.account, id).await?;
        self.after_command(sequence).await;
        Ok(())
    }

    pub async fn retry_send(&self, id: String) -> Result<CommandReceipt, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let ((), sequence) = store
            .requeue(epoch, session.account.clone(), id.clone())
            .await?;
        let row =
            store
                .get_row(&session.account, &id)
                .await?
                .ok_or_else(|| SdkError::NotFound {
                    what: "outbox".into(),
                })?;
        self.after_command(sequence).await;
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
        let ((), sequence) = store
            .mark_read_local(
                epoch,
                session.account,
                marker.dest.clone(),
                marker.visible_message_id,
            )
            .await?;
        self.after_command(sequence).await;
        self.kick_read_sync();
        Ok(())
    }

    pub async fn mark_thread_read(&self, dest: String, kind: i32) -> Result<(), SdkError> {
        if dest.is_empty() {
            return Ok(());
        }
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let ((), sequence) = store
            .mark_thread_read_local(epoch, session.account, dest, kind)
            .await?;
        self.after_command(sequence).await;
        self.kick_read_sync();
        Ok(())
    }

    pub async fn set_conversation_visibility(
        &self,
        visibility: ConversationVisibility,
    ) -> Result<(), SdkError> {
        let store = match self.store() {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let session = match self.session_snapshot() {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        if session.account.is_empty() {
            return Ok(());
        }
        let epoch = self.current_epoch().0;
        let dest = visibility
            .conversation
            .as_ref()
            .map(|c| c.dest.clone())
            .filter(|d| !d.is_empty());
        let kind = visibility
            .conversation
            .as_ref()
            .map(|c| c.kind)
            .unwrap_or(0);
        let ((), sequence) = store
            .set_visibility(
                epoch,
                session.account,
                visibility.generation,
                visibility.foreground && dest.is_some(),
                dest,
                kind,
            )
            .await?;
        self.after_command(sequence).await;
        self.kick_read_sync();
        Ok(())
    }

    pub async fn delete_thread(&self, dest: String) -> Result<(), SdkError> {
        self.cancel_outbox_run();
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let ((), sequence) = store
            .delete_thread(epoch, session.account, dest.clone())
            .await?;
        if let Some(sub) = lock(&self.inner.timelines).get_mut(&dest) {
            sub.older_bound = None;
            sub.has_more = false;
            sub.loading_older = false;
            sub.hydrating = false;
            sub.history_error = None;
        }
        self.publish_timeline_resync(&dest, "deleted").await;
        self.after_command(sequence).await;
        Ok(())
    }

    pub fn install_protocol(&self, protocol: Arc<dyn ProtocolClient>) {
        *lock(&self.inner.protocol) = Some(protocol);
        let dests: Vec<String> = lock(&self.inner.timelines).keys().cloned().collect();
        if dests.is_empty() {
            return;
        }
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let sdk = self.clone();
            handle.spawn(async move {
                for dest in dests {
                    let _ = sdk.hydrate_latest_if_needed(&dest, false).await;
                }
            });
        }
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
        let ((), _seq) = store
            .upsert_agent_profile(self.agent_account_key(), row)
            .await?;
        Ok(())
    }

    pub async fn delete_agent_profile(&self, profile_id: String) -> Result<(), SdkError> {
        let store = self.store()?;
        let ((), _seq) = store
            .delete_agent_profile(self.agent_account_key(), profile_id)
            .await?;
        Ok(())
    }

    pub async fn list_agent_profiles(&self) -> Result<Vec<AgentProfileRow>, SdkError> {
        let store = self.store()?;
        let rows = store.load_agent_profiles(&self.agent_account_key()).await?;
        Ok(rows.into_iter().filter(|r| r.deleted_at == 0).collect())
    }

    pub async fn import_agent_profiles(&self, rows: Vec<AgentProfileRow>) -> Result<(), SdkError> {
        let store = self.store()?;
        let ((), _seq) = store
            .import_agent_profiles(self.agent_account_key(), rows)
            .await?;
        Ok(())
    }

    pub async fn list_provider_accounts(&self) -> Result<Vec<ProviderAccountRow>, SdkError> {
        let store = self.store()?;
        store
            .load_provider_accounts(&self.agent_account_key())
            .await
    }

    pub async fn upsert_provider_account(&self, row: ProviderAccountRow) -> Result<(), SdkError> {
        let store = self.store()?;
        let ((), _seq) = store
            .upsert_provider_account(self.agent_account_key(), row)
            .await?;
        Ok(())
    }

    pub async fn delete_provider_account(&self, id: String) -> Result<(), SdkError> {
        let store = self.store()?;
        let ((), _seq) = store
            .delete_provider_account(self.agent_account_key(), id)
            .await?;
        Ok(())
    }

    pub async fn get_device_overlay(
        &self,
        profile_id: String,
    ) -> Result<Option<DeviceOverlayRow>, SdkError> {
        let store = self.store()?;
        store
            .load_device_overlay(&self.agent_account_key(), &profile_id)
            .await
    }

    pub async fn upsert_device_overlay(&self, row: DeviceOverlayRow) -> Result<(), SdkError> {
        let store = self.store()?;
        let ((), _seq) = store
            .upsert_device_overlay(self.agent_account_key(), row)
            .await?;
        Ok(())
    }

    pub async fn agent_flags(&self) -> Result<String, SdkError> {
        let store = self.store()?;
        store.load_agent_flags().await
    }

    pub async fn set_agent_flags(&self, flags_json: String) -> Result<(), SdkError> {
        let store = self.store()?;
        let ((), _seq) = store.upsert_agent_flags(flags_json).await?;
        Ok(())
    }

    pub async fn sync_agent_specs(&self) -> Result<(), SdkError> {
        let proto = match self.protocol() {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };
        let (remote_specs, remote_accounts) = proto.agent_spec_sync().await?;
        self.apply_remote_accounts(&remote_accounts).await?;
        let store = self.store()?;
        let account = self.agent_account_key();
        let local_specs = store.load_agent_profiles(&account).await?;
        let mut local_by_id: HashMap<String, AgentProfileRow> = local_specs
            .into_iter()
            .map(|r| (r.profile_id.clone(), r))
            .collect();
        for rec in remote_specs {
            if rec.profile_id.is_empty() || rec.spec.is_empty() {
                continue;
            }
            let local_ts = local_by_id
                .get(&rec.profile_id)
                .map(|r| r.updated_at)
                .unwrap_or(0);
            if rec.updated_at >= local_ts {
                self.upsert_agent_profile(AgentProfileRow {
                    profile_id: rec.profile_id.clone(),
                    nickname: rec.nickname,
                    server_account: rec.server_account,
                    body_json: String::new(),
                    body_blob: rec.spec,
                    placement: "local".into(),
                    updated_at: rec.updated_at,
                    deleted_at: rec.deleted_at,
                })
                .await?;
                local_by_id.remove(&rec.profile_id);
            }
        }
        let local_accounts = store.load_provider_accounts_all(&account).await?;
        let mut push_err: Option<SdkError> = None;
        let skipped = lock(&self.inner.agent_spec_push_skip).clone();
        let mut newly_skipped = Vec::new();
        for row in local_by_id.into_values() {
            // Empty blob is never a valid upsert; soft-delete tombstones still need a
            // non-empty body from the writer. Skipping here stops a permanent 101 loop.
            if row.body_blob.is_empty() {
                continue;
            }
            if skipped.contains(&row.profile_id) {
                continue;
            }
            if let Err(err) = proto
                .agent_spec_upsert(Some(&row_to_spec_record(&row)), None)
                .await
            {
                tracing::warn!(
                    error = %err,
                    profile_id = %row.profile_id,
                    "agent spec upsert failed"
                );
                if err.retryable() {
                    if push_err.is_none() {
                        push_err = Some(err);
                    }
                } else {
                    newly_skipped.push(row.profile_id.clone());
                }
            }
        }
        if !newly_skipped.is_empty() {
            lock(&self.inner.agent_spec_push_skip).extend(newly_skipped);
        }
        for acc in &local_accounts {
            let remote_ts = remote_accounts
                .iter()
                .find(|a| a.id == acc.id)
                .map(|a| a.updated_at);
            if remote_ts.is_some_and(|ts| acc.updated_at <= ts) {
                continue;
            }
            if let Err(err) = proto
                .agent_spec_upsert(None, Some(&row_to_provider_account(acc)))
                .await
            {
                tracing::warn!(error = %err, account_id = %acc.id, "provider account upsert failed");
                if err.retryable() && push_err.is_none() {
                    push_err = Some(err);
                }
            }
        }
        match push_err {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    async fn apply_remote_accounts(
        &self,
        remote: &[kim_client::AgentProviderAccount],
    ) -> Result<(), SdkError> {
        let store = self.store()?;
        let local = store
            .load_provider_accounts_all(&self.agent_account_key())
            .await?;
        let local_ts: HashMap<&str, i64> = local
            .iter()
            .map(|a| (a.id.as_str(), a.updated_at))
            .collect();
        for acc in remote {
            if acc.id.is_empty() {
                continue;
            }
            let ts = local_ts.get(acc.id.as_str()).copied().unwrap_or(0);
            if acc.updated_at >= ts {
                self.upsert_provider_account(ProviderAccountRow {
                    id: acc.id.clone(),
                    vendor_id: acc.vendor_id.clone(),
                    base_url: acc.base_url.clone(),
                    key_ref: acc.key_ref.clone(),
                    display_name: acc.display_name.clone(),
                    models_json: serde_json::to_string(&acc.models).unwrap_or_else(|_| "[]".into()),
                    updated_at: acc.updated_at,
                    deleted_at: acc.deleted_at,
                })
                .await?;
            }
        }
        Ok(())
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
                let ((), _seq) = store.touch_media(url).await?;
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
        let (evicted, _seq) = store.upsert_media(url, local.clone(), size).await?;
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

    /// Expands the registered timeline window. Data is returned only through
    /// `subscribe_timeline`; callers never merge a returned page themselves.
    pub async fn load_older(&self, dest: String) -> Result<(), SdkError> {
        let (account, epoch, limit) = {
            let mut timelines = lock(&self.inner.timelines);
            let sub = timelines
                .get_mut(&dest)
                .ok_or_else(|| SdkError::InvalidArgument {
                    message: "no timeline subscriber".into(),
                })?;
            sub.loading_older = true;
            sub.history_error = None;
            (sub.account.clone(), sub.epoch, sub.limit)
        };
        self.record_timeline_and_wait(&account, epoch, &dest).await;

        type OlderPage = (Option<(i64, String)>, bool);
        let result: Result<Option<OlderPage>, SdkError> = async {
            let window = self.timeline_window_snapshot(&dest, &account, epoch)?;
            let mut rows: Vec<&MessageView> = window
                .messages
                .iter()
                .chain(window.pending.iter())
                .collect();
            rows.sort_by(|left, right| {
                left.at
                    .cmp(&right.at)
                    .then_with(|| left.key.cmp(&right.key))
            });
            let Some(oldest) = rows.first() else {
                match self.hydrate_latest_if_needed(&dest, true).await? {
                    HydrateOutcome::Skipped => return Ok(Some((None, false))),
                    HydrateOutcome::InFlight | HydrateOutcome::Applied => {
                        return Ok(None);
                    }
                }
            };
            let before_id = rows
                .iter()
                .filter_map(|message| (message.message_id != 0).then_some(message.message_id))
                .min()
                .unwrap_or(0);
            let cursor = PageCursor {
                dest: dest.clone(),
                before_at: oldest.at,
                before_key: oldest.key.clone(),
                before_id,
                limit,
            };
            let store = self.store()?;
            let (mut page, local_has_more) = store.load_page(&account, &cursor).await?;
            let mut has_more = local_has_more;

            if !local_has_more {
                let sent = store.count_sent(&account, &dest).await?;
                let cap = i64::from(store::schema::MAX_MESSAGES);
                if sent >= cap {
                    has_more = false;
                } else if (page.len() as i32) < limit && before_id != 0 {
                    let room = cap.saturating_sub(sent);
                    let fetch_limit = i32::try_from(room).unwrap_or(limit).min(limit);
                    if fetch_limit <= 0 {
                        has_more = false;
                    } else {
                        let proto = self.protocol()?;
                        let kind = store
                            .load_threads(&account)
                            .await?
                            .into_iter()
                            .find(|thread| thread.id == dest)
                            .map(|thread| thread.kind)
                            .unwrap_or(0);
                        let remote = proto.history(&dest, kind, before_id, fetch_limit).await?;
                        let remote_full = remote.len() as i32 >= fetch_limit;
                        let talks: Vec<kim_client::IncomingTalk> = remote
                            .into_iter()
                            .take(usize::try_from(fetch_limit).unwrap_or(0))
                            .map(|history| kim_client::IncomingTalk {
                                command: String::new(),
                                dest: dest.clone(),
                                message_id: history.message_id,
                                sender: history.sender,
                                msg_type: history.msg_type,
                                body: history.body,
                                extra: history.extra,
                                send_time: history.send_time,
                            })
                            .collect();
                        if !talks.is_empty() {
                            self.persist_talks_for(
                                epoch,
                                account.clone(),
                                talks,
                                UnreadPolicy::Keep,
                            )
                            .await?;
                            (page, _) = store.load_page(&account, &cursor).await?;
                        }
                        let sent_after = store.count_sent(&account, &dest).await?;
                        has_more = sent_after < cap && remote_full;
                    }
                } else {
                    has_more = false;
                }
            }

            let older_bound = rows
                .into_iter()
                .chain(page.iter())
                .map(|message| (message.at, message.key.clone()))
                .min();
            Ok(Some((older_bound, has_more)))
        }
        .await;

        match result {
            Ok(None) => Ok(()),
            Ok(Some((older_bound, has_more))) => {
                self.finish_timeline_load(&dest, &account, epoch, older_bound, has_more, None)
                    .await;
                Ok(())
            }
            Err(error) => {
                self.finish_timeline_load(
                    &dest,
                    &account,
                    epoch,
                    None,
                    true,
                    Some(error.to_string()),
                )
                .await;
                Err(error)
            }
        }
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
            .map(|_| ())
    }

    pub(crate) async fn persist_talks_for(
        &self,
        epoch: u64,
        account: String,
        talks: Vec<kim_client::IncomingTalk>,
        policy: UnreadPolicy,
    ) -> Result<u64, SdkError> {
        let store = self.store()?;
        // Live IfInserted owner 1:1 text may enqueue after it is durable.
        // History and offline hydration use Keep and must not run the Agent.
        let agent_talks: Vec<_> = talks
            .iter()
            .filter(|talk| {
                policy == UnreadPolicy::IfInserted
                    && talk.command == kim_protocol::CMD_CHAT_USER_TALK
                    && talk.sender == account
                    && talk.message_id > 0
                    && talk.msg_type == kim_protocol::MESSAGE_TYPE_TEXT
                    && !talk.body.trim().is_empty()
            })
            .cloned()
            .collect();
        let count = talks.len();
        let ((), sequence) = store
            .persist_talks(epoch, account.clone(), talks, policy)
            .await?;
        self.inner.metrics.inc_persist_talk();
        tracing::info!(account = %account, count, "persist talks");
        self.kick_read_sync();
        for talk in agent_talks {
            if self.current_epoch().0 != epoch {
                return Err(SdkError::StaleEpoch {
                    expected: epoch,
                    actual: self.current_epoch().0,
                });
            }
            if store
                .agent_profile_id_for_dest(&account, &talk.dest)
                .await?
                .is_some()
            {
                // A full queue leaves the push unacknowledged. Replayed pushes
                // must retry admission even when the message is already stored.
                self.agent()
                    .enqueue_turn(&talk.dest, &talk.body, talk.message_id, SessionEpoch(epoch))
                    .await?;
            }
        }
        Ok(sequence)
    }

    pub fn metrics(&self) -> (u64, u64, u64, u64) {
        self.inner.metrics.snapshot()
    }

    pub fn query_refresh_total(&self) -> u64 {
        self.inner.metrics.query_refresh_total()
    }

    pub fn query_stale_skip_total(&self) -> u64 {
        self.inner.metrics.query_stale_skip_total()
    }

    pub fn query_apply_wait_timeout_total(&self) -> u64 {
        self.inner.metrics.query_apply_wait_timeout_total()
    }

    pub fn install_panic_hook(&self) {
        let inner = self.inner.clone();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info.to_string();
            tracing::error!(panic = %message, "rust panic");
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
        let (views, _sequence) = store.persist_inbox(epoch, account, items).await?;
        self.emit_session_wait(SessionUpdate::Inbox {
            threads: views.clone(),
        })
        .await;
        self.hydrate_watched(&views).await;
        Ok(views)
    }

    async fn hydrate_watched(&self, views: &[ThreadView]) {
        let watched: Vec<String> = lock(&self.inner.timelines).keys().cloned().collect();
        for dest in watched {
            if views.iter().any(|thread| thread.id == dest) {
                let _ = self.hydrate_latest_if_needed(&dest, false).await;
            }
        }
    }

    /// Open-thread catch-up. `prefetch_older` is subscribe-only: protocol
    /// install and inbox apply only fill a tip gap so they cannot steal
    /// `load_older`'s history request.
    async fn hydrate_latest_if_needed(
        &self,
        dest: &str,
        prefetch_older: bool,
    ) -> Result<HydrateOutcome, SdkError> {
        let Ok(session) = self.session_snapshot() else {
            return Ok(HydrateOutcome::Skipped);
        };
        let account = session.account;
        let epoch = self.current_epoch().0;
        let Ok(store) = self.store() else {
            return Ok(HydrateOutcome::Skipped);
        };
        let Some((kind, server_tip)) = store.thread_server_tip(&account, dest).await? else {
            return Ok(HydrateOutcome::Skipped);
        };
        if server_tip <= 0 {
            return Ok(HydrateOutcome::Skipped);
        }
        let local_tip = store.local_message_tip(&account, dest).await?;
        let cap = i64::from(store::schema::MAX_MESSAGES);
        if local_tip >= server_tip {
            if !prefetch_older {
                return Ok(HydrateOutcome::Skipped);
            }
            let sent = store.count_sent(&account, dest).await?;
            if sent >= cap {
                return Ok(HydrateOutcome::Skipped);
            }
        }
        if !self.begin_hydrate(dest, epoch, &account) {
            return Ok(HydrateOutcome::InFlight);
        }
        let proto = match self.protocol() {
            Ok(proto) => proto,
            Err(_) => {
                self.finish_timeline_load(dest, &account, epoch, None, true, None)
                    .await;
                return Ok(HydrateOutcome::Skipped);
            }
        };
        self.record_timeline_and_wait(&account, epoch, dest).await;
        let limit = lock(&self.inner.timelines)
            .get(dest)
            .map(|sub| sub.limit)
            .unwrap_or(50);
        let command = if kind == kim_protocol::INBOX_KIND_GROUP {
            kim_protocol::CMD_CHAT_GROUP_TALK
        } else {
            kim_protocol::CMD_CHAT_USER_TALK
        };
        let mut before_id = 0i64;
        let mut last_full;
        loop {
            let remote = match proto.history(dest, kind, before_id, limit).await {
                Ok(rows) => rows,
                Err(error) => {
                    self.finish_timeline_load(
                        dest,
                        &account,
                        epoch,
                        None,
                        true,
                        Some(error.to_string()),
                    )
                    .await;
                    return Err(error);
                }
            };
            let n = remote.len() as i32;
            last_full = n >= limit;
            let oldest_id = remote
                .iter()
                .map(|row| row.message_id)
                .filter(|id| *id > 0)
                .min()
                .unwrap_or(0);
            let talks: Vec<kim_client::IncomingTalk> = remote
                .into_iter()
                .map(|history| kim_client::IncomingTalk {
                    command: command.to_string(),
                    dest: dest.to_string(),
                    message_id: history.message_id,
                    sender: history.sender,
                    msg_type: history.msg_type,
                    body: history.body,
                    extra: history.extra,
                    send_time: history.send_time,
                })
                .collect();
            if !talks.is_empty() {
                if let Err(error) = self
                    .persist_talks_for(epoch, account.clone(), talks, UnreadPolicy::Keep)
                    .await
                {
                    self.finish_timeline_load(
                        dest,
                        &account,
                        epoch,
                        None,
                        true,
                        Some(error.to_string()),
                    )
                    .await;
                    return Err(error);
                }
            }
            if n == 0 || !last_full || oldest_id <= 0 || oldest_id == before_id {
                break;
            }
            match store.count_sent(&account, dest).await {
                Ok(sent) if sent < cap => before_id = oldest_id,
                _ => break,
            }
        }
        let snapshot = store
            .load_timeline_window(&account, dest, limit, None)
            .await
            .ok();
        let older_bound = snapshot.as_ref().and_then(|snap| {
            snap.messages
                .first()
                .map(|message| (message.at, message.key.clone()))
        });
        let has_more = last_full || snapshot.as_ref().is_some_and(|snap| snap.has_more);
        self.finish_timeline_load(dest, &account, epoch, older_bound, has_more, None)
            .await;
        Ok(HydrateOutcome::Applied)
    }

    fn begin_hydrate(&self, dest: &str, epoch: u64, account: &str) -> bool {
        let mut timelines = lock(&self.inner.timelines);
        let Some(sub) = timelines.get_mut(dest) else {
            return false;
        };
        if sub.account != account || sub.epoch != epoch || sub.hydrating {
            return false;
        }
        sub.hydrating = true;
        sub.loading_older = true;
        sub.history_error = None;
        true
    }

    pub async fn load_threads(&self) -> Result<Vec<ThreadView>, SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        store.load_threads(&session.account).await
    }

    pub async fn replace_contacts(&self, rows: Vec<PersonRef>) -> Result<(), SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let ((), sequence) = store
            .replace_contacts(epoch, session.account, rows.clone())
            .await?;
        self.after_command(sequence).await;
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
        let ((), _seq) = store.upsert_device_settings(row, false).await?;
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
        let ((), _seq) = store.upsert_device_settings(row, true).await?;
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
                                if let kim_client::SessionEvent::ConversationReadSync { account, state } = &ev {
                                    sdk.apply_remote_read(account, state.clone()).await;
                                }
                                if let Err(error) = sdk.persist_contact_event(&ev).await {
                                    tracing::warn!(
                                        error = %error,
                                        "persisting contact session event failed"
                                    );
                                }
                                if let Some(update) = session_update_from_event(ev) {
                                    if matches!(update, SessionUpdate::Link { .. }) {
                                        sdk.refresh_session_snapshot().await;
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
                                            sdk.kick_read_sync();
                                        }
                                        _ => {}
                                    }
                                    sdk.emit_session_wait(update).await;
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                sdk.refresh_session_snapshot().await;
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

    fn spawn_read_sync_worker(&self) {
        if !self.store_attached() {
            return;
        }
        let (tx, rx) = mpsc::channel::<()>(8);
        *lock(&self.inner.read_sync_kick) = Some(tx);
        read_sync::spawn(self.clone(), rx);
        self.kick_read_sync();
    }

    fn kick_outbox(&self) {
        if let Some(tx) = lock(&self.inner.outbox_kick).as_ref() {
            let _ = tx.try_send(());
        }
    }

    pub(crate) fn kick_read_sync(&self) {
        if let Some(tx) = lock(&self.inner.read_sync_kick).as_ref() {
            let _ = tx.try_send(());
        }
    }

    async fn apply_remote_read(&self, account: &str, state: kim_client::ConversationReadState) {
        let Ok(store) = self.store() else {
            return;
        };
        let Ok(session) = self.session_snapshot() else {
            return;
        };
        if session.account != account && !account.is_empty() {
            return;
        }
        let epoch = self.current_epoch().0;
        match store.apply_read_state(epoch, session.account, state).await {
            Ok(((), sequence)) => self.after_command(sequence).await,
            Err(err) => tracing::warn!(error = %err, "apply remote read failed"),
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
        let limit = if query.limit <= 0 {
            50
        } else {
            query.limit.min(50)
        };
        let account = self
            .session_snapshot()
            .map(|session| session.account)
            .unwrap_or_default();
        let epoch = self.current_epoch().0;
        let init = TimelineUpdate::Snapshot {
            snapshot: TimelineSnapshot {
                dest: dest.clone(),
                version: 0,
                messages: Vec::new(),
                pending: Vec::new(),
                unread: 0,
                last_read_message_id: 0,
                has_more: false,
                loading_older: false,
                history_error: None,
            },
        };
        let mut map = lock(&self.inner.timelines);
        let rx = if let Some(sub) = map.get_mut(&dest) {
            if sub.tx.is_closed() {
                let (tx, rx) = watch::channel(init);
                *sub = TimelineSub {
                    account: account.clone(),
                    epoch,
                    dest: dest.clone(),
                    limit,
                    tx,
                    older_bound: None,
                    has_more: false,
                    loading_older: false,
                    history_error: None,
                    hydrating: false,
                };
                rx
            } else {
                sub.account = account.clone();
                sub.epoch = epoch;
                sub.limit = limit;
                sub.tx.subscribe()
            }
        } else {
            let (tx, rx) = watch::channel(init);
            map.insert(
                dest.clone(),
                TimelineSub {
                    account: account.clone(),
                    epoch,
                    dest: dest.clone(),
                    limit,
                    tx,
                    older_bound: None,
                    has_more: false,
                    loading_older: false,
                    history_error: None,
                    hydrating: false,
                },
            );
            rx
        };
        drop(map);
        // FRB sync watchers are not inside a Tokio context unless the FFI
        // layer enters one first. Skip the eager load rather than panic.
        if let (Ok(handle), Some(changes)) = (
            tokio::runtime::Handle::try_current(),
            lock(&self.inner.changes).clone(),
        ) {
            let sdk = self.clone();
            let dest_h = dest.clone();
            handle.spawn(async move {
                changes.record(account, epoch, CommitEffect::timeline(dest_h.clone()));
                let _ = sdk.hydrate_latest_if_needed(&dest_h, true).await;
            });
        }
        rx
    }

    pub(crate) async fn refresh_session_snapshot(&self) {
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
        self.inner.metrics.inc_query_refresh();
    }

    /// Held stamps (account switch) keep the old account/epoch so a new-account
    /// Snapshot cannot paint into the still-open dest watch. Skip until Dart
    /// resubscribes and restamps; otherwise an in-flight refresh for the old
    /// stamp overwrites the `Resync { reason: "account" }` watch value.
    fn timeline_notice_applies(
        &self,
        sub: &TimelineSub,
        notice_epoch: u64,
        notice_account: &str,
    ) -> bool {
        if sub.epoch != notice_epoch || sub.account != notice_account {
            return false;
        }
        if sub.epoch != self.current_epoch().0 {
            return false;
        }
        matches!(
            self.session_snapshot(),
            Ok(session) if session.account == sub.account
        )
    }

    pub(crate) async fn refresh_timeline(
        &self,
        dest: &str,
        notice_epoch: u64,
        notice_account: &str,
    ) {
        let Some(sub) = lock(&self.inner.timelines).get(dest).cloned() else {
            return;
        };
        if !self.timeline_notice_applies(&sub, notice_epoch, notice_account) {
            self.inner.metrics.inc_query_stale_skip();
            return;
        }
        let Ok(store) = self.store() else {
            return;
        };
        let mut snapshot = match store
            .load_timeline_window(&sub.account, dest, sub.limit, sub.older_bound.as_ref())
            .await
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                tracing::warn!(
                    account = %notice_account,
                    epoch = notice_epoch,
                    dest,
                    error = %error,
                    "refreshing timeline failed"
                );
                match sub.tx.borrow().clone() {
                    TimelineUpdate::Snapshot { snapshot } => snapshot,
                    TimelineUpdate::Delta { .. } | TimelineUpdate::Resync { .. } => return,
                }
            }
        };
        snapshot.has_more = if sub.older_bound.is_some() || sub.history_error.is_some() {
            sub.has_more
        } else {
            snapshot.has_more
        };
        snapshot.loading_older = sub.loading_older;
        snapshot.history_error = sub.history_error.clone();
        let Some(still) = lock(&self.inner.timelines).get(dest).cloned() else {
            return;
        };
        if !self.timeline_notice_applies(&still, notice_epoch, notice_account) {
            self.inner.metrics.inc_query_stale_skip();
            return;
        }
        let _ = still.tx.send(TimelineUpdate::Snapshot { snapshot });
        self.inner.metrics.inc_query_refresh();
    }

    fn timeline_window_snapshot(
        &self,
        dest: &str,
        account: &str,
        epoch: u64,
    ) -> Result<TimelineSnapshot, SdkError> {
        let timelines = lock(&self.inner.timelines);
        let sub = timelines
            .get(dest)
            .ok_or_else(|| SdkError::InvalidArgument {
                message: "no timeline subscriber".into(),
            })?;
        if sub.account != account || sub.epoch != epoch {
            return Err(SdkError::StaleEpoch {
                expected: epoch,
                actual: self.current_epoch().0,
            });
        }
        let update = sub.tx.borrow().clone();
        match update {
            TimelineUpdate::Snapshot { snapshot } => Ok(snapshot),
            TimelineUpdate::Delta { .. } | TimelineUpdate::Resync { .. } => {
                Err(SdkError::InvalidArgument {
                    message: "timeline snapshot unavailable".into(),
                })
            }
        }
    }

    async fn finish_timeline_load(
        &self,
        dest: &str,
        account: &str,
        epoch: u64,
        older_bound: Option<(i64, String)>,
        has_more: bool,
        history_error: Option<String>,
    ) {
        {
            let mut timelines = lock(&self.inner.timelines);
            let Some(sub) = timelines.get_mut(dest) else {
                return;
            };
            if sub.account != account || sub.epoch != epoch {
                return;
            }
            if let Some(older_bound) = older_bound {
                sub.older_bound = Some(older_bound);
            }
            sub.has_more = has_more;
            sub.loading_older = false;
            sub.hydrating = false;
            sub.history_error = history_error;
        }
        self.record_timeline_and_wait(account, epoch, dest).await;
    }

    async fn record_timeline_and_wait(&self, account: &str, epoch: u64, dest: &str) {
        let Some(changes) = lock(&self.inner.changes).clone() else {
            return;
        };
        let sequence = changes.record(account.to_string(), epoch, CommitEffect::timeline(dest));
        changes.wait_applied(sequence, APPLY_WAIT).await;
    }

    pub(crate) async fn refresh_contacts_view(&self, epoch: u64, clear_error: bool) {
        if self.current_epoch().0 != epoch || self.inner.contacts_watch.is_closed() {
            return;
        }
        let Ok(session) = self.session_snapshot() else {
            return;
        };
        let account = session.account;
        if lock(&self.inner.contacts_account).as_str() != account
            || self.inner.contacts_epoch.load(Ordering::SeqCst) != epoch
        {
            self.inner.metrics.inc_query_stale_skip();
            return;
        }
        let Ok(store) = self.store() else {
            return;
        };
        let contacts = match store.load_contacts(&account).await {
            Ok(contacts) => contacts,
            Err(error) => {
                tracing::warn!(
                    account = %account,
                    epoch,
                    error = %error,
                    "refreshing contacts failed"
                );
                return;
            }
        };
        if self.current_epoch().0 != epoch
            || lock(&self.inner.contacts_account).as_str() != account
            || self.inner.contacts_epoch.load(Ordering::SeqCst) != epoch
        {
            self.inner.metrics.inc_query_stale_skip();
            return;
        }
        self.publish_contacts_snapshot(
            contacts,
            if clear_error {
                ContactsErrorOp::Clear
            } else {
                ContactsErrorOp::Keep
            },
        );
        self.inner.metrics.inc_query_refresh();
    }

    async fn publish_timeline_resync(&self, dest: &str, reason: &str) {
        if let Some(sub) = lock(&self.inner.timelines).get(dest) {
            let _ = sub.tx.send(TimelineUpdate::Resync {
                dest: dest.into(),
                reason: reason.into(),
            });
            self.inner.metrics.inc_timeline_resync();
        }
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
            let ((), _seq) = store.due_now(epoch, session.account).await?;
        }
        self.kick_outbox();
        Ok(())
    }

    async fn after_command(&self, sequence: u64) {
        let Some(changes) = lock(&self.inner.changes).clone() else {
            return;
        };
        if !changes.wait_applied(sequence, APPLY_WAIT).await {
            self.inner.metrics.inc_query_apply_wait_timeout();
            tracing::warn!(sequence, "timed out waiting for query refresh");
        }
    }

    fn next_contacts_version(&self) -> u64 {
        self.inner.contacts_version.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub(crate) fn store(&self) -> Result<Arc<Store>, SdkError> {
        lock(&self.inner.store)
            .clone()
            .ok_or(SdkError::InvalidArgument {
                message: "store not attached".into(),
            })
    }

    /// Test hook: drop the attached store so a queued turn surfaces `Error`.
    #[doc(hidden)]
    pub fn detach_store_for_test(&self) {
        *lock(&self.inner.store) = None;
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
            .map(|_| ())
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
            phase,
        } => Some(SessionUpdate::Typing {
            typer,
            dest,
            kind,
            active,
            phase,
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

fn row_to_spec_record(row: &AgentProfileRow) -> kim_client::AgentSpecRecord {
    kim_client::AgentSpecRecord {
        profile_id: row.profile_id.clone(),
        nickname: row.nickname.clone(),
        server_account: row.server_account.clone(),
        spec: row.body_blob.clone(),
        key_ciphertext: Vec::new(),
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
    }
}

fn row_to_provider_account(row: &ProviderAccountRow) -> kim_client::AgentProviderAccount {
    let models: Vec<String> = serde_json::from_str(&row.models_json).unwrap_or_default();
    kim_client::AgentProviderAccount {
        id: row.id.clone(),
        vendor_id: row.vendor_id.clone(),
        base_url: row.base_url.clone(),
        key_ref: row.key_ref.clone(),
        display_name: row.display_name.clone(),
        models,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
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
