use goose_agent::operation::{ConversationEffect, MachineEffect};
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::token_usage::ProviderUsage;

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
    Finished {
        text: String,
    },
    Failed {
        message: String,
    },
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
    #[error("{0}")]
    Failed(String),
}

impl From<anyhow::Error> for HostError {
    fn from(err: anyhow::Error) -> Self {
        HostError::Failed(err.to_string())
    }
}

pub enum HostEffect {
    Conversation(ConversationEffect),
    Usage(#[allow(dead_code)] ProviderUsage),
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
