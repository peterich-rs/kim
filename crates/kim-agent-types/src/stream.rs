use crate::messages::Usage;
use serde::{Deserialize, Serialize};

/// Provider-agnostic stream events produced by `kim-agent-llm`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    ResponseStarted {
        provider_response_id: String,
    },
    TextDelta {
        output_index: u32,
        delta: String,
    },
    ToolCallStarted {
        output_index: u32,
        call_id: String,
        name: String,
    },
    ToolCallArgsDelta {
        output_index: u32,
        call_id: String,
        delta: String,
    },
    ToolCallFinished {
        output_index: u32,
        call_id: String,
        name: String,
        /// Raw JSON object string (may be invalid).
        arguments: String,
    },
    UsageHint(Usage),
    Completed,
    Failed {
        message: String,
    },
}
