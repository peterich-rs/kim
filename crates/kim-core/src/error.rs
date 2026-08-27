use std::io;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] io::Error),

    #[error("frame too large: {size} bytes (max {max})")]
    FrameTooLarge { size: usize, max: usize },

    #[error("incomplete frame")]
    IncompleteFrame,

    #[error("connection closed")]
    Closed,

    #[error("channel `{0}` not found")]
    ChannelNotFound(String),

    #[error("channel `{0}` already exists")]
    ChannelExists(String),

    #[error("handshake timeout after {0:?}")]
    HandshakeTimeout(Duration),

    #[error("handshake failed: {0}")]
    Handshake(String),

    #[error("client already connected")]
    AlreadyConnected,

    #[error("client not connected")]
    NotConnected,

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
