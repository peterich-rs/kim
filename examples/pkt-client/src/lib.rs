//! LoginDialer: mint JWT, send `login.signin`, wait LoginResp or handshake error.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{Conn, DialerContext, Error, OpCode};
use kim_protocol::pkt::{Flag, KickoutNotify, LoginReq, LoginResp, Status};
use kim_protocol::{
    generate, marshal, read, LogicPkt, Packet, CMD_LOGIN_SIGN_IN, DEMO_DEFAULT_SECRET,
};
use kim_ws::{connect_ws, WsDialer, WsHandshakeConn};

pub fn resolve_jwt_secret() -> String {
    match std::env::var("KIM_JWT_SECRET") {
        Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => DEMO_DEFAULT_SECRET.to_string(),
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub struct LoginDialer {
    secret: String,
    bad_token: bool,
    channel_id: Mutex<Option<String>>,
}

impl LoginDialer {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            bad_token: false,
            channel_id: Mutex::new(None),
        }
    }

    pub fn with_bad_token(mut self) -> Self {
        self.bad_token = true;
        self
    }

    pub fn channel_id(&self) -> Option<String> {
        self.channel_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn store_channel_id(&self, id: String) {
        *self.channel_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
    }
}

#[async_trait]
impl WsDialer for LoginDialer {
    async fn dial_and_handshake(&self, ctx: DialerContext) -> Result<WsHandshakeConn, Error> {
        let mut conn = connect_ws(&ctx.address).await?;
        let token = if self.bad_token {
            "not-a-jwt".to_string()
        } else {
            generate(&self.secret, &ctx.id, "kim", now_ts() + 86400)
                .map_err(|e| Error::Handshake(e.to_string()))?
        };
        let mut pkt = LogicPkt::new(CMD_LOGIN_SIGN_IN, 1, Bytes::new());
        pkt.write_body(&LoginReq { token });
        conn.write_frame(OpCode::Binary, marshal(&Packet::Logic(pkt)))
            .await?;
        loop {
            let frame = match conn.read_frame().await {
                Ok(f) => f,
                Err(Error::Closed) => return Err(Error::Handshake("closed".into())),
                Err(e) => return Err(e),
            };
            match frame.opcode {
                OpCode::Close => return Err(Error::Handshake("closed".into())),
                OpCode::Ping => {
                    let _ = conn.write_frame(OpCode::Pong, Bytes::new()).await;
                }
                OpCode::Pong | OpCode::Continuation => {}
                OpCode::Binary | OpCode::Text => match read(&frame.payload) {
                    Ok(Packet::Logic(p)) => {
                        let st = p.header.status;
                        if st == Status::Unauthorized as i32
                            || st == Status::InvalidCommand as i32
                            || st == Status::ServiceUnavailable as i32
                        {
                            return Err(Error::Handshake(format!("status={st}")));
                        }
                        if st != Status::Success as i32 {
                            return Err(Error::Handshake(format!("unexpected status={st}")));
                        }
                        let resp: LoginResp =
                            p.read_body().map_err(|e| Error::Handshake(e.to_string()))?;
                        if resp.channel_id.is_empty() || resp.channel_id == "alice" {
                            return Err(Error::Handshake("bad channel_id".into()));
                        }
                        self.store_channel_id(resp.channel_id);
                        return Ok(conn);
                    }
                    Ok(_) => return Err(Error::Handshake("expected logic".into())),
                    Err(e) => return Err(Error::Handshake(e.to_string())),
                },
            }
        }
    }
}

pub fn is_kickout(pkt: &LogicPkt) -> Option<KickoutNotify> {
    if pkt.header.flag != Flag::Push as i32 {
        return None;
    }
    if pkt.header.command != CMD_LOGIN_SIGN_IN {
        return None;
    }
    pkt.read_body().ok()
}
