/// 帧类型。数值刻意与 WebSocket RFC 6455 对齐，
/// 这样 TCP 和 WebSocket 两套实现可以共用同一套业务逻辑。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xa,
}

impl OpCode {
    pub fn from_u8(n: u8) -> Option<Self> {
        Some(match n {
            0x0 => Self::Continuation,
            0x1 => Self::Text,
            0x2 => Self::Binary,
            0x8 => Self::Close,
            0x9 => Self::Ping,
            0xa => Self::Pong,
            _ => return None,
        })
    }
}

impl From<OpCode> for u8 {
    fn from(value: OpCode) -> Self {
        value as u8
    }
}
