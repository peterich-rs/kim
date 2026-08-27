use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{Acceptor, Agent, Conn, Error, MessageListener, Server, StateListener};
use kim_ws::{ClientOptions, WsClient, WsIdentityDialer, WsServer};

struct EchoHandler;

#[async_trait]
impl Acceptor for EchoHandler {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| Error::HandshakeTimeout(timeout))??;
        Ok(String::from_utf8_lossy(&frame.payload).to_string())
    }
}

#[async_trait]
impl MessageListener for EchoHandler {
    async fn receive(&self, agent: &dyn Agent, payload: Bytes) {
        let mut out = payload.to_vec();
        out.extend_from_slice(b" from server");
        let _ = agent.push(Bytes::from(out)).await;
    }
}

#[async_trait]
impl StateListener for EchoHandler {
    async fn disconnect(&self, _channel_id: &str) -> Result<(), Error> {
        Ok(())
    }
}

#[tokio::test]
async fn ws_echo_roundtrip() {
    let handler = Arc::new(EchoHandler);
    let mut server = WsServer::bind("127.0.0.1:0").await.unwrap();
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    let addr = server.local_addr();
    let server = Arc::new(server);
    let running = server.clone();
    tokio::spawn(async move {
        running.start().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = WsClient::new(
        "bob",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(WsIdentityDialer));
    client.connect(&format!("ws://{addr}/")).await.unwrap();
    client.send(Bytes::from_static(b"hello")).await.unwrap();
    let frame = client.read().await.unwrap();
    assert_eq!(&frame.payload[..], b"hello from server");
    client.close().await.unwrap();
    let _ = server.shutdown().await;
}
