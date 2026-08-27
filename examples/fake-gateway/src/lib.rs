//! WGateway demo handler: JWT login Accept, local ping, SN_LOGIN forward.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_container::{Container, DownlinkHook};
use kim_core::{Acceptor, Agent, Conn, Error, MessageListener, OpCode, Server, StateListener};
use kim_protocol::pkt::{Flag, KickoutNotify, LoginReq, Session, Status};
use kim_protocol::{
    marshal, parse, read, read_logic, BasicPkt, LogicPkt, Packet, CMD_LOGIN_SIGN_IN,
    CMD_LOGIN_SIGN_OUT, CODE_PING, CODE_PONG, DEMO_DEFAULT_SECRET, SN_LOGIN,
};
use tracing::{info, warn};

pub fn resolve_jwt_secret(config_secret: &str) -> String {
    if let Ok(env) = std::env::var("KIM_JWT_SECRET") {
        let trimmed = env.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let trimmed = config_secret.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    tracing::warn!(secret = "demo-default", "do not use in production");
    DEMO_DEFAULT_SECRET.to_string()
}

pub fn remote_ip(conn: &dyn Conn) -> String {
    let Some(peer) = conn.peer_addr() else {
        return String::new();
    };
    strip_port(&peer)
}

fn strip_port(peer: &str) -> String {
    if let Some(rest) = peer.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return rest[..end].to_string();
        }
        return peer.to_string();
    }
    match peer.rsplit_once(':') {
        Some((ip, _)) => ip.to_string(),
        None => peer.to_string(),
    }
}

async fn write_status(
    conn: &mut dyn Conn,
    header: &kim_protocol::pkt::Header,
    status: Status,
) -> Result<(), Error> {
    let mut pkt = LogicPkt::new_from(header);
    pkt.header.status = status as i32;
    pkt.header.flag = Flag::Response as i32;
    conn.write_frame(OpCode::Binary, marshal(&Packet::Logic(pkt)))
        .await
}

pub struct GatewayHandler {
    container: Arc<Container>,
    gateway_id: String,
    jwt_secret: String,
    seq: AtomicU64,
    pending: Mutex<HashMap<String, LogicPkt>>,
}

impl GatewayHandler {
    pub fn new(
        container: Arc<Container>,
        gateway_id: impl Into<String>,
        jwt_secret: impl Into<String>,
    ) -> Self {
        Self {
            container,
            gateway_id: gateway_id.into(),
            jwt_secret: jwt_secret.into(),
            seq: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
        }
    }

    fn generate_channel_id(&self, account: &str) -> String {
        let n = self.seq.fetch_add(1, Ordering::Relaxed);
        format!("{}_{account}_{n}", self.gateway_id)
    }
}

#[async_trait]
impl Acceptor for GatewayHandler {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| Error::HandshakeTimeout(timeout))??;
        let mut pkt = match read_logic(&frame.payload) {
            Ok(p) => p,
            Err(_) => return Err(Error::Handshake("expected login.signin".into())),
        };
        if pkt.header.command != CMD_LOGIN_SIGN_IN {
            write_status(conn, &pkt.header, Status::InvalidCommand).await?;
            return Err(Error::Handshake("invalid command".into()));
        }
        let req: LoginReq = match pkt.read_body() {
            Ok(r) => r,
            Err(_) => {
                write_status(conn, &pkt.header, Status::InvalidPacketBody).await?;
                return Err(Error::Handshake("invalid packet body".into()));
            }
        };
        let claims = match parse(&self.jwt_secret, &req.token) {
            Ok(c) => c,
            Err(err) => {
                warn!(%err, "unauthorized");
                write_status(conn, &pkt.header, Status::Unauthorized).await?;
                return Err(Error::Handshake("unauthorized".into()));
            }
        };
        let id = self.generate_channel_id(&claims.account);
        pkt.header.channel_id = id.clone();
        pkt.write_body(&Session {
            channel_id: id.clone(),
            gate_id: self.gateway_id.clone(),
            account: claims.account.clone(),
            app: claims.app,
            remote_ip: remote_ip(conn),
            ..Session::default()
        });
        {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.insert(id.clone(), pkt);
        }
        info!(account = %claims.account, channel = %id, "accept login");
        Ok(id)
    }

    async fn on_accept_abandoned(&self, channel_id: &str) {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending.remove(channel_id);
    }

    async fn on_channel_ready(&self, channel_id: &str) -> Result<(), Error> {
        let pkt = {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.remove(channel_id)
        };
        let Some(pkt) = pkt else {
            return Err(Error::other("pending login missing"));
        };
        let mut resp = LogicPkt::new_from(&pkt.header);
        resp.header.flag = Flag::Response as i32;
        resp.header.status = Status::ServiceUnavailable as i32;
        if let Err(err) = self.container.forward(SN_LOGIN, pkt).await {
            warn!(%err, "forward login failed");
            if let Err(e) = self.container.push(channel_id, resp).await {
                warn!(%e, "push ServiceUnavailable failed");
            }
            return Err(Error::other(err.to_string()));
        }
        Ok(())
    }
}

#[async_trait]
impl MessageListener for GatewayHandler {
    async fn receive(&self, agent: &dyn Agent, payload: Bytes) {
        let pkt = match read(&payload) {
            Ok(p) => p,
            Err(err) => {
                warn!(%err, "bad payload");
                return;
            }
        };
        match pkt {
            Packet::Basic(p) if p.code == CODE_PING => {
                info!(channel = agent.id(), "basic ping, local pong");
                let _ = agent
                    .push(marshal(&Packet::Basic(BasicPkt {
                        code: CODE_PONG,
                        body: Bytes::new(),
                    })))
                    .await;
            }
            Packet::Basic(_) => {}
            Packet::Logic(mut logic) => {
                logic.header.channel_id = agent.id().to_string();
                let svc = logic.service_name().to_string();
                if let Err(err) = self.container.forward(&svc, logic).await {
                    warn!(%err, "forward failed");
                    let mut resp = match read(&payload) {
                        Ok(Packet::Logic(p)) => p,
                        _ => return,
                    };
                    resp.header.channel_id = agent.id().to_string();
                    resp.header.flag = Flag::Response as i32;
                    resp.header.status = Status::ServiceUnavailable as i32;
                    let _ = agent.push(marshal(&Packet::Logic(resp))).await;
                }
            }
        }
    }
}

#[async_trait]
impl StateListener for GatewayHandler {
    async fn disconnect(&self, channel_id: &str) -> Result<(), Error> {
        info!(channel = channel_id, "disconnect");
        let mut logout = LogicPkt::new(CMD_LOGIN_SIGN_OUT, 0, Bytes::new());
        logout.header.channel_id = channel_id.to_string();
        if let Err(err) = self.container.forward(SN_LOGIN, logout).await {
            warn!(%err, "signout forward failed");
        }
        Ok(())
    }
}

/// Closes the channel after a Kickout Push is written. Attach the WsServer
/// after two-phase assembly (`set_*` then `Arc::new`).
pub struct KickHook {
    server: OnceLock<Arc<dyn Server + Send + Sync>>,
}

impl KickHook {
    pub fn new() -> Self {
        Self {
            server: OnceLock::new(),
        }
    }

    pub fn attach(&self, server: Arc<dyn Server + Send + Sync>) {
        let _ = self.server.set(server);
    }
}

impl Default for KickHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DownlinkHook for KickHook {
    async fn after_push(&self, channel_id: &str, pkt: &LogicPkt) {
        if pkt.header.flag != Flag::Push as i32 {
            return;
        }
        if pkt.header.command != CMD_LOGIN_SIGN_IN {
            return;
        }
        let Ok(notify) = pkt.read_body::<KickoutNotify>() else {
            return;
        };
        if notify.channel_id != channel_id {
            return;
        }
        info!(channel = channel_id, "kick close");
        if let Some(srv) = self.server.get() {
            let _ = srv.close_channel(channel_id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::strip_port;

    #[test]
    fn strip_ipv4_port() {
        assert_eq!(strip_port("127.0.0.1:8001"), "127.0.0.1");
    }

    #[test]
    fn strip_ipv6_port() {
        assert_eq!(strip_port("[::1]:8001"), "::1");
    }
}
