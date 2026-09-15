//! Desktop MobileAgent. Phone keeps [`NoopAgent`] and never installs queues.

#[allow(dead_code)]
mod permissions;
mod profiles;
mod queue;
mod runtime;
mod sessions;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::error::SdkError;
use crate::ids::SessionEpoch;
use crate::session::lock;
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

pub struct MobileAgent {
    sdk: KimSdk,
    runtime: Arc<dyn AgentRuntime>,
    queues: Mutex<HashMap<String, mpsc::Sender<Turn>>>,
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
                match runtime
                    .run_turn(
                        &turn.dest,
                        &profile_id,
                        &turn.text,
                        turn.in_reply_to,
                        turn.epoch,
                    )
                    .await
                {
                    Ok(run) if run.error.is_none() && !run.output.is_empty() => {
                        if let Ok(proto) = sdk.protocol() {
                            let client_id = uuid::Uuid::new_v4().to_string();
                            let _ = proto
                                .bot_reply(&turn.dest, &run.output, turn.in_reply_to, &client_id)
                                .await;
                        }
                        let _ = sdk
                            .emit_session_wait(SessionUpdate::AgentTurn {
                                dest: turn.dest,
                                state: AgentTurnState::Done,
                                text: run.output,
                            })
                            .await;
                    }
                    Ok(run) => {
                        let _ = sdk
                            .emit_session_wait(SessionUpdate::AgentTurn {
                                dest: turn.dest,
                                state: AgentTurnState::Error,
                                text: run.error.unwrap_or_default(),
                            })
                            .await;
                    }
                    Err(err) => {
                        let _ = sdk
                            .emit_session_wait(SessionUpdate::AgentTurn {
                                dest: turn.dest,
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
        {
            let mut lru = lock(&self.lru);
            lru.touch(dest, &profile_id);
        }
        let tx = {
            let mut queues = lock(&self.queues);
            if let Some(tx) = queues.get(dest) {
                tx.clone()
            } else {
                let (tx, rx) = mpsc::channel(QUEUE_CAP);
                queues.insert(dest.to_string(), tx.clone());
                drop(queues);
                self.spawn_worker(rx);
                tx
            }
        };
        tx.try_send(Turn {
            dest: dest.to_string(),
            text: text.to_string(),
            in_reply_to,
            epoch,
        })
        .map_err(|_| SdkError::Busy {
            queue: "agent".into(),
        })?;
        self.sdk
            .emit_session_wait(SessionUpdate::AgentTurn {
                dest: dest.to_string(),
                state: AgentTurnState::Queued,
                text: text.to_string(),
            })
            .await;
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
