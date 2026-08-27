use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{Acceptor, Agent, Conn, Error, MessageListener, Server, StateListener};
use kim_tcp::{ClientOptions, IdentityDialer, TcpClient, TcpServer};

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
async fn echo_roundtrip() {
    let handler = Arc::new(EchoHandler);
    let mut server = TcpServer::bind("127.0.0.1:0").await.unwrap();
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

    let mut client = TcpClient::new(
        "bob",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(IdentityDialer));
    client.connect(&addr.to_string()).await.unwrap();
    client.send(Bytes::from_static(b"hello")).await.unwrap();
    let frame = client.read().await.unwrap();
    assert_eq!(&frame.payload[..], b"hello from server");
    client.close().await.unwrap();
    let _ = server.shutdown().await;
}

#[tokio::test]
async fn send_while_read_pending() {
    let handler = Arc::new(EchoHandler);
    let mut server = TcpServer::bind("127.0.0.1:0").await.unwrap();
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    let addr = server.local_addr();
    let server = Arc::new(server);
    let running = server.clone();
    tokio::spawn(async move {
        running.start().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(30)).await;

    let mut client = TcpClient::new(
        "carol",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(IdentityDialer));
    client.connect(&addr.to_string()).await.unwrap();
    let client = Arc::new(client);
    let reader = client.clone();
    let pending = tokio::spawn(async move { reader.read().await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    client
        .send(Bytes::from_static(b"hello"))
        .await
        .expect("send must succeed while read is pending");
    let frame = pending.await.unwrap().unwrap();
    assert_eq!(&frame.payload[..], b"hello from server");
    let _ = server.shutdown().await;
}

#[tokio::test]
async fn shutdown_before_start_returns() {
    let handler = Arc::new(EchoHandler);
    let mut server = TcpServer::bind("127.0.0.1:0").await.unwrap();
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    let server = Arc::new(server);
    server.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), server.start())
        .await
        .expect("start should return after shutdown")
        .unwrap();
}
