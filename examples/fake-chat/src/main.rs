use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer};
use kim_core::{Acceptor, Agent, Conn, Error, MessageListener, Server, StateListener};
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_protocol::pkt::{Flag, InnerHandshakeReq, Status};
use kim_protocol::{
    read_logic, CMD_DEMO_ECHO, META_DEST_CHANNELS, META_DEST_SERVER,
};
use kim_tcp::TcpServer;
use prost::Message;
use serde::Deserialize;
use tracing::{info, warn};

#[derive(Deserialize)]
struct File {
    #[serde(rename = "self")]
    this: SelfSection,
}

#[derive(Deserialize)]
struct SelfSection {
    service_id: String,
    service_name: String,
    listen: String,
    protocol: String,
}

struct ChatHandler {
    container: Arc<Container>,
}

#[async_trait]
impl Acceptor for ChatHandler {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| Error::HandshakeTimeout(timeout))??;
        let req = InnerHandshakeReq::decode(frame.payload.as_ref())
            .map_err(|e| Error::Handshake(e.to_string()))?;
        if req.service_id.is_empty() {
            return Err(Error::Handshake("empty service id".into()));
        }
        Ok(req.service_id)
    }
}

#[async_trait]
impl MessageListener for ChatHandler {
    async fn receive(&self, _agent: &dyn Agent, payload: Bytes) {
        let mut pkt = match read_logic(&payload) {
            Ok(p) => p,
            Err(err) => {
                warn!(%err, "unexpected basic pkt or bad logic");
                return;
            }
        };
        if pkt.header.command != CMD_DEMO_ECHO {
            pkt.header.flag = Flag::Response as i32;
            pkt.header.status = Status::CommandNotFound as i32;
        } else {
            pkt.header.flag = Flag::Response as i32;
            pkt.header.status = Status::Success as i32;
        }
        let gw = pkt.get_meta(META_DEST_SERVER).unwrap_or("").to_string();
        let ch = pkt.header.channel_id.clone();
        pkt.set_meta(META_DEST_SERVER, &gw);
        pkt.set_meta(META_DEST_CHANNELS, &ch);
        if let Err(err) = self.container.push(&gw, pkt).await {
            warn!(%err, "push to gateway failed");
        }
    }
}

#[async_trait]
impl StateListener for ChatHandler {
    async fn disconnect(&self, channel_id: &str) -> Result<(), Error> {
        info!(channel = channel_id, "gateway disconnected");
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

    let naming = Arc::new(StaticNaming::from_slice(vec![]));
    let identity = DefaultRegistration {
        service_id: cfg.this.service_id.clone(),
        service_name: cfg.this.service_name,
        protocol: cfg.this.protocol,
        public_address: String::new(),
        public_port: 0,
        tags: vec![],
        meta: HashMap::new(),
    };

    let mut server = TcpServer::bind(&cfg.this.listen).await?;
    let container = Container::new(ContainerOpts {
        naming,
        identity,
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: cfg.this.service_id,
        }),
        deps: vec![],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
    });
    let handler = Arc::new(ChatHandler {
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
