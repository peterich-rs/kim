use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Core(#[from] kim_core::Error),
    #[error(transparent)]
    Naming(#[from] kim_naming::Error),
    #[error(transparent)]
    Protocol(#[from] kim_protocol::ProtocolError),
    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
