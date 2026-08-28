use async_trait::async_trait;
use kim_protocol::{LogicPkt, ProtocolError};
use thiserror::Error;

use crate::storage::SessionError;

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("dispatcher: {0}")]
    Dispatcher(String),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("{0}")]
    Other(String),
}

/// Push a packet to one gateway for one or more channels.
///
/// Implementations **must** set `dest.server` to `gateway` (the target, not the
/// origin request's gateway) and `dest.channels` to the comma-joined ids.
#[async_trait]
pub trait Dispatcher: Send + Sync {
    async fn push(
        &self,
        gateway: &str,
        channels: &[String],
        pkt: LogicPkt,
    ) -> Result<(), RouterError>;
}
