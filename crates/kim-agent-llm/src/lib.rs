//! OpenAI Responses API protocol: SSE → StreamEvent → SettledAssistantMessage.

pub mod reduce;
pub mod responses;
pub mod scripted;

pub use reduce::{reduce_events, reduce_stream, Reducer};
pub use responses::parse::{parse_sse_chunk, parse_sse_stream, ResponsesParseError};
pub use scripted::ScriptedLlm;

use async_trait::async_trait;
use futures::stream::BoxStream;
use kim_agent_types::{
    Context, JsonValue, StreamEvent, ToolChoice, ToolSchema,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("llm aborted")]
    Aborted,
    #[error("llm failed: {0}")]
    Failed(String),
    #[error(transparent)]
    Parse(#[from] ResponsesParseError),
}

pub type LlmResult<T> = Result<T, LlmError>;

#[derive(Debug, Clone)]
pub struct ModelRef {
    pub id: String,
}

impl ModelRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[derive(Debug, Clone)]
pub enum ResponseInputItem {
    Message {
        role: String,
        content: Vec<ContentPart>,
    },
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    FunctionCallOutput {
        call_id: String,
        output: String,
    },
}

#[derive(Debug, Clone)]
pub enum ContentPart {
    InputText { text: String },
    OutputText { text: String },
}

#[derive(Debug, Clone)]
pub struct ResponseRequest {
    pub model: ModelRef,
    pub input: Vec<ResponseInputItem>,
    pub tools: Vec<ToolSchema>,
    pub tool_choice: ToolChoice,
    pub parallel_tool_calls: bool,
    pub stream: bool,
    pub previous_response_id: Option<String>,
    pub max_output_tokens: Option<u32>,
    /// Optional system/developer instructions (AGENTS.md injection).
    pub instructions: Option<String>,
}

impl Default for ResponseRequest {
    fn default() -> Self {
        Self {
            model: ModelRef::new("gpt-test"),
            input: Vec::new(),
            tools: Vec::new(),
            tool_choice: ToolChoice::Auto,
            parallel_tool_calls: true,
            stream: true,
            previous_response_id: None,
            max_output_tokens: None,
            instructions: None,
        }
    }
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn stream(
        &self,
        req: ResponseRequest,
        cx: &Context,
    ) -> LlmResult<BoxStream<'static, LlmResult<StreamEvent>>>;
}

/// Map session-ish messages into Responses input items (Phase 0 linear flatten).
pub fn map_history_to_input(items: &[(String, String)]) -> Vec<ResponseInputItem> {
    items
        .iter()
        .map(|(role, text)| {
            let (role, part) = if role == "assistant" {
                (
                    "assistant".to_string(),
                    ContentPart::OutputText {
                        text: text.clone(),
                    },
                )
            } else {
                (
                    "user".to_string(),
                    ContentPart::InputText {
                        text: text.clone(),
                    },
                )
            };
            ResponseInputItem::Message {
                role,
                content: vec![part],
            }
        })
        .collect()
}

pub fn json_obj(v: impl IntoIterator<Item = (String, JsonValue)>) -> JsonValue {
    JsonValue::Object(v.into_iter().collect())
}
