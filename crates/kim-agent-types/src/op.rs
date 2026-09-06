use crate::ids::{EntryId, OperationId, UsageId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    Running,
    CancelRequested { requested_at_ms: u64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSettings {
    pub parallel_tool_calls: bool,
    pub max_retries: u32,
}

impl Default for RunSettings {
    fn default() -> Self {
        Self {
            parallel_tool_calls: true,
            max_retries: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationScope {
    pub control: Control,
    pub settings: RunSettings,
    pub latest_assistant_entry_id: Option<EntryId>,
}

impl OperationScope {
    pub fn new() -> Self {
        Self {
            control: Control::Running,
            settings: RunSettings::default(),
            latest_assistant_entry_id: None,
        }
    }

    pub fn is_cancel_requested(&self) -> bool {
        matches!(self.control, Control::CancelRequested { .. })
    }
}

impl Default for OperationScope {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationIntent {
    Run { prompt_entry_ids: Vec<EntryId> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationMeta {
    pub operation_id: OperationId,
    pub lane: String,
    pub source_tip_id: Option<EntryId>,
    pub started_at_ms: u64,
    pub intent: OperationIntent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Continuation {
    NeedAssistant,
    MayFinish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayPolicy {
    Never,
    Safe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum ToolCallState {
    Planned {
        source_index: usize,
        result_entry_id: EntryId,
    },
    EffectPending {
        source_index: usize,
        result_entry_id: EntryId,
        replay: ReplayPolicy,
    },
    OutcomeReady {
        source_index: usize,
        result_entry_id: EntryId,
        terminate: bool,
    },
    Completed {
        source_index: usize,
        result_entry_id: EntryId,
        terminate: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum OperationState {
    Starting {
        scope: OperationScope,
    },
    Checkpoint {
        scope: OperationScope,
        continuation: Continuation,
        trigger_entry_id: EntryId,
    },
    AssistantReady {
        scope: OperationScope,
    },
    AssistantEffectPending {
        scope: OperationScope,
        response_entry_id: EntryId,
        usage_id: UsageId,
        attempt: u32,
    },
    AssistantRetryWait {
        scope: OperationScope,
        not_before_ms: u64,
        attempt: u32,
    },
    Tools {
        scope: OperationScope,
        step_id: String,
        calls: Vec<ToolCallState>,
    },
}

impl OperationState {
    pub fn scope(&self) -> &OperationScope {
        match self {
            Self::Starting { scope }
            | Self::Checkpoint { scope, .. }
            | Self::AssistantReady { scope }
            | Self::AssistantEffectPending { scope, .. }
            | Self::AssistantRetryWait { scope, .. }
            | Self::Tools { scope, .. } => scope,
        }
    }

    pub fn scope_mut(&mut self) -> &mut OperationScope {
        match self {
            Self::Starting { scope }
            | Self::Checkpoint { scope, .. }
            | Self::AssistantReady { scope }
            | Self::AssistantEffectPending { scope, .. }
            | Self::AssistantRetryWait { scope, .. }
            | Self::Tools { scope, .. } => scope,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LaneState {
    pub current_operation_id: Option<OperationId>,
    pub last_operation_id: Option<OperationId>,
    pub inbox: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OperationResult {
    Completed {
        operation_id: OperationId,
        final_entry_id: Option<EntryId>,
    },
    Aborted {
        operation_id: OperationId,
        final_entry_id: Option<EntryId>,
    },
    Failed {
        operation_id: OperationId,
        message: String,
    },
}
