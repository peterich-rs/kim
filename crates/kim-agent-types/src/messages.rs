use crate::ids::{EntryId, UsageId};
use crate::JsonValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Stop,
    ToolUse,
    Length,
    Aborted,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantContent {
    Text {
        text: String,
    },
    ToolCall {
        call_id: String,
        name: String,
        arguments: JsonValue,
        /// Position in content[] for source-order materialization.
        source_index: usize,
        /// Set when arguments JSON failed to parse; harness synthesizes tool error.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parse_error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettledAssistantMessage {
    pub content: Vec<AssistantContent>,
    pub stop_reason: StopReason,
    pub provider_response_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageRow {
    pub id: UsageId,
    pub operation_id: crate::ids::OperationId,
    pub usage: Usage,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum AgentMessage {
    System {
        text: String,
    },
    User {
        text: String,
    },
    Assistant {
        content: Vec<AssistantContent>,
        stop_reason: StopReason,
        provider_response_id: Option<String>,
    },
    Tool {
        call_id: String,
        name: String,
        output: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryBase {
    pub id: EntryId,
    pub parent_id: Option<EntryId>,
    /// Assigned at commit; 0 until then.
    #[serde(default)]
    pub seq: u64,
    /// Assigned at commit; 0 until then.
    #[serde(default)]
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    Message {
        base: EntryBase,
        message: AgentMessage,
    },
}

impl Entry {
    pub fn base(&self) -> &EntryBase {
        match self {
            Entry::Message { base, .. } => base,
        }
    }

    pub fn base_mut(&mut self) -> &mut EntryBase {
        match self {
            Entry::Message { base, .. } => base,
        }
    }

    pub fn id(&self) -> EntryId {
        self.base().id
    }

    pub fn parent_id(&self) -> Option<EntryId> {
        self.base().parent_id
    }

    pub fn message(&self) -> Option<&AgentMessage> {
        match self {
            Entry::Message { message, .. } => Some(message),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: JsonValue,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    #[default]
    Auto,
    None,
    Required,
    Specific(String),
}
