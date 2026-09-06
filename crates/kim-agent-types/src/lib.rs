//! Shared types for the kim agent harness.
//!
//! Named `kim-agent-*` to avoid clashing with IM `kim-session`.

mod ids;
mod messages;
mod op;
mod stream;

pub use ids::*;
pub use messages::*;
pub use op::*;
pub use stream::*;

use tokio_util::sync::CancellationToken;

/// Process-local invocation context (never durable).
#[derive(Debug, Clone)]
pub struct Context {
    pub abort: CancellationToken,
    pub trace_id: Option<String>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            abort: CancellationToken::new(),
            trace_id: None,
        }
    }

    pub fn with_trace(trace_id: impl Into<String>) -> Self {
        Self {
            abort: CancellationToken::new(),
            trace_id: Some(trace_id.into()),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.abort.is_cancelled()
    }

    pub fn cancel(&self) {
        self.abort.cancel();
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

pub type JsonValue = serde_json::Value;
