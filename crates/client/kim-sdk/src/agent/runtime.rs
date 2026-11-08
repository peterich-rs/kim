use std::sync::{Arc, Mutex};

use crate::error::SdkError;
use crate::ids::SessionEpoch;
use crate::session::lock;

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
