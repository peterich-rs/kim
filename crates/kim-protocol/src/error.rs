use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("bad magic")]
    BadMagic,
    #[error("incomplete packet")]
    Incomplete,
    #[error("basic pkt body too large: {0}")]
    BasicTooLarge(usize),
    #[error("not a logic packet")]
    NotLogic,
    #[error("protobuf: {0}")]
    Prost(#[from] prost::DecodeError),
}
