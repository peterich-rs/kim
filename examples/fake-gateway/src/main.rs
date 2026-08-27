use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_container::{Container, ContainerOpts, InnerTcpDialer};
use kim_core::{Acceptor, Agent, Conn, Error, MessageListener, Server, StateListener};
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_protocol::pkt::{Flag, Status};
use kim_protocol::{
    marshal, read, BasicPkt, Packet, CODE_PING, CODE_PONG,
};
use kim_ws::WsServer;
use serde::Deserialize;
use tracing::{info, warn};

#[derive(Deserialize)]
struct File {
    #[serde(rename = "self")]
    this: SelfSection,
    #[serde(default)]
    services: Vec<ServiceRow>,
}

#[derive(Deserialize)]
struct SelfSection {
    service_id: String,
    service_name: String,
    listen: String,
    protocol: String,
}

#[derive(Deserialize)]
struct ServiceRow {
    service_id: String,
    service_name: String,
    protocol: String,
    public_address: String,
    public_port: u16,
}

struct GatewayHandler {
    container: Arc<Container>,
}

#[async_trait]
impl Acceptor for GatewayHandler {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| Error::HandshakeTimeout(timeout))??;
        let id = String::from_utf8_lossy(&frame.payload).trim().to_string();
        if id.is_empty() {
            return Err(Error::Handshake("empty id".into()));
        }
        Ok(id)
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
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.toml"));
    let cfg: File = toml::from_str(&std::fs::read_to_string(&path)?)?;

    let regs: Vec<DefaultRegistration> = cfg
        .services
        .into_iter()
        .map(|s| DefaultRegistration {
            service_id: s.service_id,
            service_name: s.service_name,
            protocol: s.protocol,
            public_address: s.public_address,
            public_port: s.public_port,
            tags: vec![],
            meta: HashMap::new(),
        })
        .collect();
    let naming = Arc::new(StaticNaming::from_slice(regs));
    let identity = DefaultRegistration {
        service_id: cfg.this.service_id.clone(),
        service_name: cfg.this.service_name,
        protocol: cfg.this.protocol,
        public_address: String::new(),
        public_port: 0,
        tags: vec![],
        meta: HashMap::new(),
    };

    let mut server = WsServer::bind(&cfg.this.listen).await?;
    let container = Container::new(ContainerOpts {
        naming,
        identity,
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: cfg.this.service_id,
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(kim_container::HashSelector),
    });
    let handler = Arc::new(GatewayHandler {
        container: container.clone(),
    });
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    container.attach_server(Arc::new(server));

    let c = container.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = c.shutdown().await;
    });
    container.start().await?;
    Ok(())
}
