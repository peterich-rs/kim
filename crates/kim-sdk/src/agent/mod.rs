//! Desktop MobileAgent. Phone keeps [`NoopAgent`] and never installs queues.

#[allow(dead_code)]
mod permissions;
mod profiles;
mod queue;
mod runtime;
mod sessions;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::error::SdkError;
use crate::ids::SessionEpoch;
use crate::session::lock;
use crate::sync::UnreadPolicy;
use crate::timeline::{AgentTurnState, SessionUpdate};
use crate::KimSdk;

pub use profiles::{AgentProfileRow, DeviceOverlayRow, ProviderAccountRow};
pub use runtime::{
    AgentRunRequest, AgentRunResult, AgentRuntime, FfiAgentRuntime, ScriptedRuntime,
};
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
}

struct Turn {
    dest: String,
    text: String,
    in_reply_to: i64,
    epoch: SessionEpoch,
}

struct TurnQueue {
    tx: mpsc::Sender<Turn>,
    // Bound replay protection per destination; include epoch for account changes.
    admitted: VecDeque<(SessionEpoch, i64)>,
}

const RECENT_TURN_CAP: usize = 1024;

pub struct MobileAgent {
    sdk: KimSdk,
    runtime: Arc<dyn AgentRuntime>,
    queues: Mutex<HashMap<String, TurnQueue>>,
    lru: Arc<Mutex<SessionLru>>,
}

impl MobileAgent {
    #[must_use]
    pub fn new(sdk: KimSdk, runtime: Arc<dyn AgentRuntime>) -> Self {
        Self {
            sdk,
            runtime,
            queues: Mutex::new(HashMap::new()),
            lru: Arc::new(Mutex::new(SessionLru::new())),
        }
    }

    #[must_use]
    pub fn lru_len(&self) -> usize {
        lock(&self.lru).len()
    }

    fn spawn_worker(&self, mut rx: mpsc::Receiver<Turn>) {
        let sdk = self.sdk.clone();
        let runtime = self.runtime.clone();
        let lru = self.lru.clone();
        tokio::spawn(async move {
            while let Some(turn) = rx.recv().await {
                if sdk.current_epoch() != turn.epoch {
                    continue;
                }
                let store = match sdk.store() {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let account = match sdk.session_snapshot() {
                    Ok(s) => s.account,
                    Err(_) => continue,
                };
                let profile_id =
                    match profiles::profile_id_for_dest(&store, &account, &turn.dest).await {
                        Ok(Some(id)) => id,
                        _ => continue,
                    };
                {
                    let mut lru = lock(&lru);
                    lru.touch(&turn.dest, &profile_id);
                }
                let _ = sdk
                    .emit_session_wait(SessionUpdate::AgentTurn {
                        dest: turn.dest.clone(),
                        state: AgentTurnState::Running,
                        text: turn.text.clone(),
                    })
                    .await;
                let dest = turn.dest.clone();
                set_bot_busy(&sdk, &dest, true).await;
                let (stop_hb, stop_rx) = tokio::sync::oneshot::channel();
                {
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
                    });
                }
                let result = runtime
                    .run_turn(&dest, &profile_id, &turn.text, turn.in_reply_to, turn.epoch)
                    .await;
                let _ = stop_hb.send(());
                match result {
                    Ok(run) if run.error.is_none() && !run.output.is_empty() => {
                        if let Ok(proto) = sdk.protocol() {
                            let client_id = uuid::Uuid::new_v4().to_string();
                            match proto
                                .bot_reply(&dest, &run.output, turn.in_reply_to, &client_id)
                                .await
                            {
                                Ok((message_id, send_time)) if message_id != 0 => {
                                    persist_bot_reply(
                                        &sdk,
                                        &account,
                                        &dest,
                                        &run.output,
                                        message_id,
                                        send_time,
                                    )
                                    .await;
                                }
                                _ => {}
                            }
                        }
                        set_bot_busy(&sdk, &dest, false).await;
                        let _ = sdk
                            .emit_session_wait(SessionUpdate::AgentTurn {
                                dest,
                                state: AgentTurnState::Done,
                                text: run.output,
                            })
                            .await;
                    }
                    Ok(run) => {
                        set_bot_busy(&sdk, &dest, false).await;
                        let _ = sdk
                            .emit_session_wait(SessionUpdate::AgentTurn {
                                dest,
                                state: AgentTurnState::Error,
                                text: run.error.unwrap_or_default(),
                            })
                            .await;
                    }
                    Err(err) => {
                        set_bot_busy(&sdk, &dest, false).await;
                        let _ = sdk
                            .emit_session_wait(SessionUpdate::AgentTurn {
                                dest,
                                state: AgentTurnState::Error,
                                text: err.to_string(),
                            })
                            .await;
                    }
                }
            }
        });
    }
}

async fn set_bot_busy(sdk: &KimSdk, dest: &str, active: bool) {
    let _ = sdk
        .emit_session_wait(SessionUpdate::Typing {
            typer: dest.to_string(),
            dest: dest.to_string(),
            kind: 0,
            active,
            phase: if active {
                kim_protocol::TYPING_PHASE_RUNNING
            } else {
                kim_protocol::TYPING_PHASE_COMPOSING
            },
        })
        .await;
    if let Ok(proto) = sdk.protocol() {
        let _ = proto.bot_typing(dest, 0, active).await;
    }
}

async fn persist_bot_reply(
    sdk: &KimSdk,
    account: &str,
    dest: &str,
    body: &str,
    message_id: i64,
    send_time: i64,
) {
    let talk = kim_client::IncomingTalk {
        command: kim_protocol::CMD_CHAT_USER_TALK.to_string(),
        dest: dest.to_string(),
        message_id,
        sender: dest.to_string(),
        msg_type: kim_protocol::MESSAGE_TYPE_TEXT,
        body: body.to_string(),
        extra: String::new(),
        send_time,
    };
    let _ = sdk
        .persist_talks_for(
            sdk.current_epoch().0,
            account.to_string(),
            vec![talk],
            UnreadPolicy::Keep,
        )
        .await;
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
        let store = self.sdk.store()?;
        let account = self.sdk.session_snapshot()?.account;
        let Some(profile_id) = profiles::profile_id_for_dest(&store, &account, dest).await? else {
            return Ok(());
        };
        if self.sdk.current_epoch() != epoch {
            return Err(SdkError::StaleEpoch {
                expected: epoch.0,
                actual: self.sdk.current_epoch().0,
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
                self.spawn_worker(rx);
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
                })
                .map_err(|_| SdkError::Busy {
                    queue: "agent".into(),
                })?;
            // Record only successful admissions; a full queue remains retryable.
            if in_reply_to > 0 {
                if queue.admitted.len() == RECENT_TURN_CAP {
                    queue.admitted.pop_front();
                }
                queue.admitted.push_back(key);
            }
        }
        self.sdk
            .emit_session_wait(SessionUpdate::AgentTurn {
                dest: dest.to_string(),
                state: AgentTurnState::Queued,
                text: text.to_string(),
            })
            .await;
        set_bot_busy(&self.sdk, dest, true).await;
        Ok(())
    }

    async fn catch_up(&self, dests: &[String], epoch: SessionEpoch) -> Result<(), SdkError> {
        let proto = match self.sdk.protocol() {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };
        for dest in dests {
            let store = self.sdk.store()?;
            let account = self.sdk.session_snapshot()?.account;
            if profiles::profile_id_for_dest(&store, &account, dest)
                .await?
                .is_none()
            {
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
}
