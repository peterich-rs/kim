use std::time::Duration;

use kim_core::Error as CoreError;
use kim_protocol::ProtocolError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("not connected")]
    NotConnected,
    #[error("already connected")]
    AlreadyConnected,
    #[error("not logged in")]
    NotLoggedIn,
    #[error("handshake timeout after {0:?}")]
    HandshakeTimeout(Duration),
    #[error("handshake failed: {0}")]
    Handshake(String),
    #[error("status {0}")]
    Status(i32),
    #[error("protocol: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("unauthorized")]
    Unauthorized,
    #[error("invalid token")]
    InvalidToken,
    #[error("invalid account")]
    InvalidAccount,
    #[error("invalid password")]
    InvalidPassword,
    #[error("http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("{0}")]
    Core(#[from] CoreError),
    #[error("{0}")]
    Other(String),
}

impl ClientError {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Login will not succeed with this token; do not reconnect.
    pub fn is_fatal_auth(&self) -> bool {
        matches!(self, Self::Unauthorized | Self::InvalidToken)
    }
}
