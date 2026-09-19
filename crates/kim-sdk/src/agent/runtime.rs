use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use crate::error::SdkError;
use crate::ids::SessionEpoch;
use crate::session::lock;

#[derive(Clone, Debug)]
pub struct AgentRunRequest {
    pub dest: String,
    pub profile_id: String,
    pub text: String,
    pub in_reply_to: i64,
    pub epoch: u64,
}

#[derive(Clone, Debug)]
pub struct AgentRunResult {
    pub dest: String,
    pub profile_id: String,
    pub epoch: u64,
    pub output: String,
    pub error: Option<String>,
    pub stop_reason: String,
    pub replied: bool,
    pub visible: bool,
    pub recently_active: bool,
}

impl AgentRunResult {
    #[must_use]
    pub fn ok(
        dest: impl Into<String>,
        profile_id: impl Into<String>,
        epoch: u64,
        output: impl Into<String>,
    ) -> Self {
        let output = output.into();
        let replied = !output.trim().is_empty();
        Self {
            dest: dest.into(),
            profile_id: profile_id.into(),
            epoch,
            output,
            error: None,
            stop_reason: if replied { "completed" } else { "empty" }.into(),
            replied,
            visible: replied,
            recently_active: false,
        }
    }
}

#[async_trait::async_trait]
pub trait AgentRuntime: Send + Sync {
    async fn run_turn(
        &self,
        dest: &str,
        profile_id: &str,
        text: &str,
        in_reply_to: i64,
        epoch: SessionEpoch,
    ) -> Result<AgentRunResult, SdkError>;
}

/// FFI runtime: push [`AgentRunRequest`] to Dart, wait for submit.
pub struct FfiAgentRuntime {
    runs: Mutex<Vec<mpsc::Sender<AgentRunRequest>>>,
    waiters: Mutex<HashMap<(String, u64), oneshot::Sender<AgentRunResult>>>,
    seq: Mutex<u64>,
}

impl FfiAgentRuntime {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            runs: Mutex::new(Vec::new()),
            waiters: Mutex::new(HashMap::new()),
            seq: Mutex::new(0),
        })
    }

    pub fn subscribe(&self) -> mpsc::Receiver<AgentRunRequest> {
        let (tx, rx) = mpsc::channel(8);
        lock(&self.runs).push(tx);
        rx
    }

    pub fn submit(&self, result: AgentRunResult) {
        if let Some(tx) = lock(&self.waiters).remove(&(result.dest.clone(), result.epoch)) {
            let _ = tx.send(result);
        }
    }

    fn next_epoch(&self, session: SessionEpoch) -> u64 {
        let mut seq = lock(&self.seq);
        *seq = seq.saturating_add(1);
        session.0.saturating_mul(1_000_000).saturating_add(*seq)
    }
}

#[async_trait::async_trait]
impl AgentRuntime for FfiAgentRuntime {
    async fn run_turn(
        &self,
        dest: &str,
        profile_id: &str,
        text: &str,
        in_reply_to: i64,
        epoch: SessionEpoch,
    ) -> Result<AgentRunResult, SdkError> {
        let run_epoch = self.next_epoch(epoch);
        let req = AgentRunRequest {
            dest: dest.to_string(),
            profile_id: profile_id.to_string(),
            text: text.to_string(),
            in_reply_to,
            epoch: run_epoch,
        };
        let (tx, rx) = oneshot::channel();
        lock(&self.waiters).insert((dest.to_string(), run_epoch), tx);
        let send_err = {
            let mut subs = lock(&self.runs);
            subs.retain(|s| !s.is_closed());
            if subs.is_empty() {
                Some(SdkError::Busy {
                    queue: "agent_run".into(),
                })
            } else {
                let mut sent = false;
                for s in subs.iter() {
                    match s.try_send(req.clone()) {
                        Ok(()) => sent = true,
                        Err(
                            mpsc::error::TrySendError::Full(_)
                            | mpsc::error::TrySendError::Closed(_),
                        ) => {}
                    }
                }
                if sent {
                    None
                } else {
                    Some(SdkError::Busy {
                        queue: "agent_run".into(),
                    })
                }
            }
        };
        if let Some(err) = send_err {
            lock(&self.waiters).remove(&(dest.to_string(), run_epoch));
            return Err(err);
        }
        rx.await.map_err(|_| SdkError::Internal {
            message: "agent run waiter dropped".into(),
        })
    }
}

/// Test double: records turns and returns a scripted output.
pub struct ScriptedRuntime {
    pub turns: Mutex<Vec<(String, String, i64)>>,
    pub output: String,
}

impl ScriptedRuntime {
    #[must_use]
    pub fn new(output: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            turns: Mutex::new(Vec::new()),
            output: output.into(),
        })
    }
}

#[async_trait::async_trait]
impl AgentRuntime for ScriptedRuntime {
    async fn run_turn(
        &self,
        dest: &str,
        profile_id: &str,
        text: &str,
        in_reply_to: i64,
        epoch: SessionEpoch,
    ) -> Result<AgentRunResult, SdkError> {
        lock(&self.turns).push((dest.to_string(), text.to_string(), in_reply_to));
        Ok(AgentRunResult::ok(
            dest.to_string(),
            profile_id.to_string(),
            epoch.0,
            self.output.clone(),
        ))
    }
}
