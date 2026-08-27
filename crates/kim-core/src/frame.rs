use bytes::Bytes;

use crate::OpCode;

/// 通信层看到的最小完整包裹。
///
/// 业务层只关心「这是什么类型」和「里面的字节」。
/// TCP 怎么切流、WebSocket 怎么拆 Frame，都被闷在 Conn 里。
#[derive(Clone, Debug)]
pub struct Frame {
    pub opcode: OpCode,
    pub payload: Bytes,
}

impl Frame {
    pub fn new(opcode: OpCode, payload: impl Into<Bytes>) -> Self {
        Self {
            opcode,
            payload: payload.into(),
        }
    }

    pub fn binary(payload: impl Into<Bytes>) -> Self {
        Self::new(OpCode::Binary, payload)
    }

    pub fn ping() -> Self {
        Self::new(OpCode::Ping, Bytes::new())
    }

    pub fn pong() -> Self {
        Self::new(OpCode::Pong, Bytes::new())
    }

    pub fn close(reason: impl Into<Bytes>) -> Self {
        Self::new(OpCode::Close, reason)
    }
}
