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

/// Send `login.signin` on an already-connected socket and wait for `LoginResp`.
pub async fn perform_login(conn: &mut dyn Conn, token: String) -> Result<String, Error> {
    perform_login_with_device(conn, token, String::new()).await
}

pub async fn perform_login_with_device(
    conn: &mut dyn Conn,
    token: String,
    device: impl Into<String>,
) -> Result<String, Error> {
    let mut pkt = LogicPkt::new(CMD_LOGIN_SIGN_IN, 1, Bytes::new());
    pkt.write_body(&LoginReq {
        token,
        device: device.into(),
    });
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
                    return Ok(resp.channel_id);
                }
                Ok(_) => return Err(Error::Handshake("expected logic".into())),
                Err(e) => return Err(Error::Handshake(e.to_string())),
            },
        }
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn mint_token(secret: &str, account: &str) -> Result<String, Error> {
    let base = std::env::var("KIM_AUTH_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty());
    let password = std::env::var("KIM_PASSWORD")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let (Some(base), Some(password)) = (base, password) {
        match fetch_login(&base, account, &password).await {
            Ok(t) => return Ok(t),
            Err(err) => tracing::warn!(%err, "KIM_AUTH_URL failed; local generate"),
        }
    }
    generate(secret, account, "kim", now_ts() + 86400).map_err(|e| Error::Handshake(e.to_string()))
}

async fn fetch_login(base: &str, account: &str, password: &str) -> Result<String, Error> {
    use kim_protocol::pkt::{AuthReq, AuthResp};
    use prost::Message;

    let url = format!("{base}/api/v1/auth/login");
    let body = AuthReq {
        account: account.to_string(),
        password: password.to_string(),
    }
    .encode_to_vec();
    let resp = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/x-protobuf")
        .header("Accept", "application/x-protobuf")
        .body(body)
        .send()
        .await
        .map_err(|e| Error::Handshake(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(Error::Handshake(format!("login status {}", resp.status())));
    }
    let buf = resp
        .bytes()
        .await
        .map_err(|e| Error::Handshake(e.to_string()))?;
    let decoded = AuthResp::decode(buf.as_ref()).map_err(|e| Error::Handshake(e.to_string()))?;
    if decoded.token.is_empty() {
        return Err(Error::Handshake("token missing".into()));
    }
    Ok(decoded.token)
}

pub struct LoginDialer {
    secret: String,
    bad_token: bool,
    device: String,
    channel_id: Mutex<Option<String>>,
    token: Option<String>,
}

impl LoginDialer {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            bad_token: false,
            device: String::new(),
            channel_id: Mutex::new(None),
            token: None,
        }
    }

    pub fn with_bad_token(mut self) -> Self {
        self.bad_token = true;
        self
    }

    pub fn with_device(mut self, device: impl Into<String>) -> Self {
        self.device = device.into();
        self
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
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
        } else if let Some(token) = &self.token {
            token.clone()
        } else {
            mint_token(&self.secret, &ctx.id).await?
        };
        let channel_id = perform_login_with_device(&mut conn, token, self.device.clone()).await?;
        self.store_channel_id(channel_id);
        Ok(conn)
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
