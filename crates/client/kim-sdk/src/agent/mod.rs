//! Desktop MobileAgent. Phone keeps [`NoopAgent`] and never installs queues.

#[allow(dead_code)]
mod permissions;
mod profiles;
mod queue;
mod runtime;
mod sessions;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, Weak};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::SdkError;
use crate::ids::SessionEpoch;
use crate::session::lock;
use crate::timeline::{AgentTurnState, SessionUpdate};
use crate::{Inner, KimSdk};

pub use profiles::{AgentProfileRow, DeviceOverlayRow, ProviderAccountRow};
pub use runtime::{AgentRunResult, AgentRuntime, ScriptedRuntime};
pub use sessions::SessionLru;

pub const QUEUE_CAP: usize = queue::QUEUE_CAP;

#[async_trait::async_trait]
pub trait AgentPort: Send + Sync {
    async fn enqueue_turn(
        &self,
        dest: &str,
        text: &str,
        in_reply_to: i64,
        epoch: SessionEpoch,
    ) -> Result<(), SdkError>;
    async fn catch_up(&self, dests: &[String], epoch: SessionEpoch) -> Result<(), SdkError>;
    /// Cancel queued and running turns for `epoch` and wait for their tasks to stop.
    async fn end_session(&self, epoch: SessionEpoch) -> Result<(), SdkError>;
    /// Reopen admission after a session ends. The default agent has nothing to resume.
    fn resume(&self) {}
}

pub struct NoopAgent;

#[async_trait::async_trait]
impl AgentPort for NoopAgent {
    async fn enqueue_turn(
        &self,
        _dest: &str,
        _text: &str,
        _in_reply_to: i64,
        _epoch: SessionEpoch,
    ) -> Result<(), SdkError> {
        Ok(())
    }

    async fn catch_up(&self, _dests: &[String], _epoch: SessionEpoch) -> Result<(), SdkError> {
        Ok(())
    }

    async fn end_session(&self, _epoch: SessionEpoch) -> Result<(), SdkError> {
        Ok(())
    }
}

struct Turn {
    dest: String,
    text: String,
    in_reply_to: i64,
    epoch: SessionEpoch,
    retried: bool,
    cancel: CancellationToken,
}

struct TurnQueue {
    tx: mpsc::Sender<Turn>,
    // Bound replay protection per destination; include epoch for account changes.
    admitted: VecDeque<(SessionEpoch, i64)>,
}

const RECENT_TURN_CAP: usize = 1024;

pub struct MobileAgent {
    sdk: Weak<Inner>,
    runtime: Arc<dyn AgentRuntime>,
    queues: Mutex<HashMap<String, TurnQueue>>,
    lru: Arc<Mutex<SessionLru>>,
    /// Cancelled by `end_session`. Stays cancelled until `resume` replaces it,
    /// so a turn for the ended epoch cannot be admitted behind the cancellation.
    session_cancel: Mutex<CancellationToken>,
}

impl MobileAgent {
    #[must_use]
    pub fn new(sdk: &KimSdk, runtime: Arc<dyn AgentRuntime>) -> Self {
        Self {
            sdk: sdk.downgrade(),
            runtime,
            queues: Mutex::new(HashMap::new()),
            lru: Arc::new(Mutex::new(SessionLru::new())),
            session_cancel: Mutex::new(CancellationToken::new()),
        }
    }

    fn sdk(&self) -> Result<KimSdk, SdkError> {
        self.sdk
            .upgrade()
            .map(KimSdk::from_inner)
            .ok_or(SdkError::NotConnected)
    }

    /// Open admission again. `start_session` calls this once the new epoch is current.
    pub fn resume(&self) {
        *lock(&self.session_cancel) = CancellationToken::new();
    }

    #[must_use]
    pub fn lru_len(&self) -> usize {
        lock(&self.lru).len()
    }

    fn spawn_worker(&self, tx: mpsc::Sender<Turn>, mut rx: mpsc::Receiver<Turn>) {
        let weak = self.sdk.clone();
        let runtime = self.runtime.clone();
        let lru = self.lru.clone();
        tokio::spawn(async move {
            while let Some(turn) = rx.recv().await {
                if turn.cancel.is_cancelled() {
                    continue;
                }
                let Some(inner) = weak.upgrade() else {
                    continue;
                };
                let sdk = KimSdk::from_inner(inner);
                if sdk.current_epoch() != turn.epoch || sdk.closed() {
                    continue;
                }
                let profile_id = match sdk.agent_profile_for_dest(&turn.dest).await {
                    Ok(Some(id)) => id,
                    Ok(None) => continue,
                    Err(err) => {
                        emit_turn(&sdk, &turn.dest, AgentTurnState::Error, err.to_string()).await;
                        continue;
                    }
                };
                {
                    let mut lru = lock(&lru);
                    lru.touch(&turn.dest, &profile_id);
                }
                emit_turn(&sdk, &turn.dest, AgentTurnState::Running, turn.text.clone()).await;
                let dest = turn.dest.clone();
                set_bot_busy(&sdk, &dest, true).await;
                let (stop_hb, stop_rx) = tokio::sync::oneshot::channel();
                let hb_task = {
                    let sdk = sdk.clone();
                    let dest = dest.clone();
                    tokio::spawn(async move {
                        let mut stop_rx = stop_rx;
                        let mut hb = tokio::time::interval(std::time::Duration::from_secs(2));
                        hb.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                        loop {
                            tokio::select! {
                                _ = &mut stop_rx => break,
                                _ = hb.tick() => set_bot_busy(&sdk, &dest, true).await,
                            }
                        }
                    })
                };
                let result = tokio::select! {
                    () = turn.cancel.cancelled() => continue,
                    result = runtime.run_turn(&dest, &profile_id, &turn.text, turn.in_reply_to, turn.epoch) => result,
                };
                let _ = stop_hb.send(());
                // In-flight heartbeat `true` must finish before `false`, or the
                // indicator lights again after the reply is already on screen.
                let _ = hb_task.await;
                // The host already ran. A session that ended while it ran must not
                // publish into whatever account is current now.
                if turn.cancel.is_cancelled() || sdk.current_epoch() != turn.epoch || sdk.closed() {
                    set_bot_busy(&sdk, &dest, false).await;
                    continue;
                }
                let run = match result {
                    Ok(run) => run,
                    Err(err) => {
                        set_bot_busy(&sdk, &dest, false).await;
                        emit_turn(&sdk, &dest, AgentTurnState::Error, err.to_string()).await;
                        continue;
                    }
                };
                if should_requeue(&run, turn.retried) {
                    let queued = tx.try_send(Turn {
                        dest: turn.dest.clone(),
                        text: turn.text.clone(),
                        in_reply_to: turn.in_reply_to,
                        epoch: turn.epoch,
                        retried: true,
                        cancel: turn.cancel.clone(),
                    });
                    if queued.is_err() {
                        set_bot_busy(&sdk, &dest, false).await;
                        emit_turn(&sdk, &dest, AgentTurnState::Error, "agent busy".into()).await;
                    }
                    continue;
                }
                match classify_turn(&run) {
                    TurnClass::Reply => {
                        let client_id = uuid::Uuid::new_v4().to_string();
                        if let Err(err) = sdk
                            .enqueue_bot_reply(&dest, &run.output, turn.in_reply_to, &client_id)
                            .await
                        {
                            tracing::warn!(error = %err, dest = %dest, "enqueue bot reply");
                            set_bot_busy(&sdk, &dest, false).await;
                            emit_turn(&sdk, &dest, AgentTurnState::Error, err.to_string()).await;
                            continue;
                        }
                        set_bot_busy(&sdk, &dest, false).await;
                        emit_turn(&sdk, &dest, AgentTurnState::Done, run.output).await;
                    }
                    TurnClass::Done => {
                        set_bot_busy(&sdk, &dest, false).await;
                        emit_turn(&sdk, &dest, AgentTurnState::Done, String::new()).await;
                    }
                    TurnClass::Empty => {
                        set_bot_busy(&sdk, &dest, false).await;
                        emit_turn(&sdk, &dest, AgentTurnState::Empty, String::new()).await;
                    }
                    TurnClass::Error(text) => {
                        set_bot_busy(&sdk, &dest, false).await;
                        emit_turn(&sdk, &dest, AgentTurnState::Error, text).await;
                    }
                }
            }
        });
    }
}

async fn emit_turn(sdk: &KimSdk, dest: &str, state: AgentTurnState, text: String) {
    let _ = sdk
        .emit_session_wait(SessionUpdate::AgentTurn {
            dest: dest.to_string(),
            state,
            text,
        })
        .await;
}

fn should_requeue(run: &AgentRunResult, retried: bool) -> bool {
    if retried || !run.recently_active {
        return false;
    }
    matches!(run.stop_reason.as_str(), "hard_timeout" | "poisoned")
}

enum TurnClass {
    Reply,
    Done,
    Empty,
    Error(String),
}

fn classify_turn(run: &AgentRunResult) -> TurnClass {
    let reason = run.stop_reason.as_str();
    if run.error.is_some()
        || matches!(
            reason,
            "cancelled"
                | "yield_abandoned"
                | "idle_timeout"
                | "hard_timeout"
                | "poisoned"
                | "provider"
                | "failed"
        )
    {
        let text = run
            .error
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| reason.to_string());
        return TurnClass::Error(text);
    }
    if run.replied {
        return TurnClass::Reply;
    }
    if run.visible {
        return TurnClass::Done;
    }
    TurnClass::Empty
}

async fn set_bot_busy(sdk: &KimSdk, dest: &str, active: bool) {
    // Wire only. Local UI follows AgentTurn; `dispatch_cmd` already skips this
    // channel, so a local Typing emit would be a second source on the owner.
    if let Ok(proto) = sdk.protocol() {
        let _ = proto.bot_typing(dest, 0, active).await;
    }
}

#[async_trait::async_trait]
impl AgentPort for MobileAgent {
    async fn enqueue_turn(
        &self,
        dest: &str,
        text: &str,
        in_reply_to: i64,
        epoch: SessionEpoch,
    ) -> Result<(), SdkError> {
        let sdk = self.sdk()?;
        let Some(profile_id) = sdk.agent_profile_for_dest(dest).await? else {
            return Ok(());
        };
        if sdk.current_epoch() != epoch {
            return Err(SdkError::StaleEpoch {
                expected: epoch.0,
                actual: sdk.current_epoch().0,
            });
        }
        let cancel = lock(&self.session_cancel).child_token();
        if cancel.is_cancelled() {
            return Err(SdkError::StaleEpoch {
                expected: epoch.0,
                actual: sdk.current_epoch().0,
            });
        }
        {
            let mut lru = lock(&self.lru);
            lru.touch(dest, &profile_id);
        }
        {
            let mut queues = lock(&self.queues);
            let queue = queues.entry(dest.to_string()).or_insert_with(|| {
                let (tx, rx) = mpsc::channel(QUEUE_CAP);
                self.spawn_worker(tx.clone(), rx);
                TurnQueue {
                    tx,
                    admitted: VecDeque::new(),
                }
            });
            let key = (epoch, in_reply_to);
            if in_reply_to > 0 && queue.admitted.contains(&key) {
                return Ok(());
            }
            queue
                .tx
                .try_send(Turn {
                    dest: dest.to_string(),
                    text: text.to_string(),
                    in_reply_to,
                    epoch,
                    retried: false,
                    cancel,
                })
                .map_err(|_| {
                    tracing::warn!(dest, in_reply_to, "agent queue full");
                    SdkError::Busy {
                        queue: "agent".into(),
                    }
                })?;
            // Record only successful admissions; a full queue remains retryable.
            if in_reply_to > 0 {
                if queue.admitted.len() == RECENT_TURN_CAP {
                    queue.admitted.pop_front();
                }
                queue.admitted.push_back(key);
            }
        }
        tracing::info!(dest, profile_id = %profile_id, in_reply_to, "agent turn queued");
        sdk.emit_session_wait(SessionUpdate::AgentTurn {
            dest: dest.to_string(),
            state: AgentTurnState::Queued,
            text: text.to_string(),
        })
        .await;
        set_bot_busy(&sdk, dest, true).await;
        Ok(())
    }

    async fn catch_up(&self, dests: &[String], epoch: SessionEpoch) -> Result<(), SdkError> {
        let sdk = match self.sdk() {
            Ok(sdk) => sdk,
            Err(_) => return Ok(()),
        };
        let proto = match sdk.protocol() {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };
        for dest in dests {
            if sdk.agent_profile_for_dest(dest).await?.is_none() {
                continue;
            }
            let items = proto.bot_pending(dest, 20).await?;
            for item in items {
                self.enqueue_turn(dest, &item.body, item.message_id, epoch)
                    .await?;
            }
        }
        Ok(())
    }

    fn resume(&self) {
        MobileAgent::resume(self);
    }

    async fn end_session(&self, epoch: SessionEpoch) -> Result<(), SdkError> {
        lock(&self.session_cancel).cancel();
        // Drop the queue senders. A worker finishes the turn it already popped,
        // then exits; turns still queued carry the cancelled token and stop.
        lock(&self.queues).clear();
        if let Ok(sdk) = self.sdk() {
            if sdk.current_epoch() == epoch {
                tracing::debug!(epoch = epoch.0, "agent session ended");
            }
        }
        Ok(())
    }
}
