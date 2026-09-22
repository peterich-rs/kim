//! FFI error. The name must not end in `Error`: FRB collapses those into anyhow.

use kim_agent_host::{HostError, ProviderFail};

#[derive(Debug, thiserror::Error)]
pub enum AgentFailure {
    #[error("api key missing")]
    MissingApiKey,
    #[error("unknown provider {name}")]
    UnknownProvider { name: String },
    #[error("invalid provider url")]
    InvalidUrl,
    #[error("session busy")]
    Busy,
    #[error("unknown session")]
    UnknownSession,
    #[error("unknown tool call")]
    UnknownToolCall,
    #[error("rate limited")]
    RateLimited,
    #[error("context exceeded")]
    ContextExceeded,
    #[error("host poisoned")]
    Poisoned { recently_active: bool },
    #[error("{message}")]
    Failed { message: String },
}

impl From<HostError> for AgentFailure {
    fn from(err: HostError) -> Self {
        match err {
            HostError::MissingApiKey => Self::MissingApiKey,
            HostError::UnknownProvider(name) => Self::UnknownProvider { name },
            HostError::InvalidUrl(_) => Self::InvalidUrl,
            HostError::Busy(_) => Self::Busy,
            HostError::UnknownSession(_) => Self::UnknownSession,
            HostError::UnknownToolCall(_) => Self::UnknownToolCall,
            HostError::Provider(ProviderFail::RateLimited { .. }) => Self::RateLimited,
            HostError::Provider(ProviderFail::ContextExceeded) => Self::ContextExceeded,
            HostError::Poisoned {
                recently_active, ..
            } => Self::Poisoned { recently_active },
            other => Self::Failed {
                message: other.to_string(),
            },
        }
    }
}
