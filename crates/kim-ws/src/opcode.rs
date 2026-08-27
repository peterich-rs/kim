use fastwebsockets::OpCode as WsOp;
use kim_core::{Error, OpCode};

pub fn from_ws(op: WsOp) -> Result<OpCode, Error> {
    Ok(match op {
        WsOp::Continuation => OpCode::Continuation,
        WsOp::Text => OpCode::Text,
        WsOp::Binary => OpCode::Binary,
        WsOp::Close => OpCode::Close,
        WsOp::Ping => OpCode::Ping,
        WsOp::Pong => OpCode::Pong,
    })
}

pub fn to_ws(op: OpCode) -> WsOp {
    match op {
        OpCode::Continuation => WsOp::Continuation,
        OpCode::Text => WsOp::Text,
        OpCode::Binary => WsOp::Binary,
        OpCode::Close => WsOp::Close,
        OpCode::Ping => WsOp::Ping,
        OpCode::Pong => WsOp::Pong,
    }
}
