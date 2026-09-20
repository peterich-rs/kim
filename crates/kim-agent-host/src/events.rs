use goose_agent::operation::{ConversationEffect, MachineEffect};
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::token_usage::ProviderUsage;
use goose_provider_types::errors::ProviderError;

#[derive(Debug, Clone)]
pub enum TurnOutcome {
    Finished {
        text: String,
        /// Non-empty user-visible assistant text on this dest.
        replied: bool,
        /// `replied || send_ok`. UI only; does not enqueue `bot.reply` by itself.
        visible: bool,
    },
    Yielded {
        kind: YieldKind,
        call_id: String,
        name: String,
    },
    Cancelled {
        reason: CancelReason,
    },
    TimedOut {
        kind: TimeoutKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutKind {
    Idle,
    Hard { recently_active: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    UserAbort,
    YieldAbandoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YieldKind {
    ToolRequest,
    ActionRequired,
}

#[derive(Debug, Clone)]
pub struct PendingYield {
    pub call_id: String,
    pub name: String,
    pub arguments_json: String,
    pub kind: YieldKind,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub enum HostEvent {
    TextDelta {
        delta: String,
    },
    ToolRequest {
        call_id: String,
        name: String,
        arguments_json: String,
    },
    ToolResult {
        call_id: String,
        name: String,
        output_preview: String,
        ok: bool,
    },
    ActionRequired {
        call_id: String,
        name: String,
        arguments_json: String,
        prompt: String,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
    },
    /// UI-only. Does not reset the idle clock.
    Keepalive,
}

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("api key missing")]
    MissingApiKey,
    #[error("unknown provider {0}")]
    UnknownProvider(String),
    #[error("invalid provider url: {0}")]
    InvalidUrl(String),
    #[error("session {0} is busy")]
    Busy(String),
    #[error("unknown session {0}")]
    UnknownSession(String),
    #[error("unknown tool call {0}")]
    UnknownToolCall(String),
    #[error("profile: {0}")]
    Profile(String),
    #[error(transparent)]
    Provider(ProviderFail),
    #[error("host poisoned: {message}")]
    Poisoned {
        message: String,
        recently_active: bool,
    },
    /// Last resort. New code must not stuff a classifiable failure in here.
    #[error("{0}")]
    Failed(String),
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum ProviderFail {
    #[error("rate limited")]
    RateLimited {
        retry_after: Option<std::time::Duration>,
    },
    #[error("context exceeded")]
    ContextExceeded,
    #[error("output truncated")]
    Truncated,
    #[error("unsupported image")]
    UnsupportedImage,
    #[error("{0}")]
    Other(String),
}

impl ProviderFail {
    pub fn from_provider(err: &ProviderError) -> Self {
        match err {
            ProviderError::RateLimitExceeded { retry_delay, .. } => Self::RateLimited {
                retry_after: *retry_delay,
            },
            ProviderError::ContextLengthExceeded(_) => Self::ContextExceeded,
            other => Self::Other(other.to_string()),
        }
    }
}

impl From<anyhow::Error> for HostError {
    fn from(err: anyhow::Error) -> Self {
        HostError::Failed(err.to_string())
    }
}

pub enum HostEffect {
    Conversation(ConversationEffect),
    Usage(ProviderUsage),
}

impl From<Message> for HostEffect {
    fn from(message: Message) -> Self {
        Self::Conversation(ConversationEffect::AppendMessage(message))
    }
}

impl goose_agent::inference::InferenceEffect for HostEffect {
    fn record_usage(usage: ProviderUsage) -> Self {
        Self::Usage(usage)
    }
}

impl MachineEffect for HostEffect {
    fn ensure_message_ids(&mut self) {
        if let Self::Conversation(effect) = self {
            effect.ensure_message_ids();
        }
    }
}

/// Stable `stop_reason` strings. Control flow reads these, not error text.
pub fn stop_reason_finished(replied: bool, visible: bool) -> &'static str {
    if replied {
        "completed"
    } else if visible {
        "side_effect"
    } else {
        "empty"
    }
}

pub fn is_idle_activity(event: &HostEvent) -> bool {
    matches!(
        event,
        HostEvent::TextDelta { .. }
            | HostEvent::ToolRequest { .. }
            | HostEvent::ToolResult { .. }
            | HostEvent::ActionRequired { .. }
            | HostEvent::Usage { .. }
    )
}
