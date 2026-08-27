use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{Acceptor, Agent, Conn, Error, MessageListener, Server, StateListener};
use kim_tcp::TcpServer;
use tracing::info;

/// echo 的「业务」：握手时把对方名字当连接 id，收到什么就加一句 from server 回回去。
struct EchoHandler;

#[async_trait]
impl Acceptor for EchoHandler {
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
impl MessageListener for EchoHandler {
    async fn receive(&self, agent: &dyn Agent, payload: Bytes) {
        let mut out = payload.to_vec();
        out.extend_from_slice(b" from server");
        if let Err(err) = agent.push(Bytes::from(out)).await {
            tracing::warn!(channel = agent.id(), %err, "echo push failed");
        }
    }
}

#[async_trait]
impl StateListener for EchoHandler {
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

    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8000".to_string());

    let handler = Arc::new(EchoHandler);
    let mut server = TcpServer::bind(&addr).await?;
    info!("echo server bound to {}", server.local_addr());
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);

    let server = Arc::new(server);
    let running = server.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        info!("ctrl-c, shutting down");
        let _ = running.shutdown().await;
    });

    server.start().await?;
    Ok(())
}
