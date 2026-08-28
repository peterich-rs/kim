use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{Acceptor, Agent, Conn, Error, MessageListener, Server, StateListener};
use kim_tcp::{ClientOptions, IdentityDialer, TcpClient, TcpServer};

fn assert_accept_peer(conn: &dyn Conn) {
    let peer = conn
        .peer_addr()
        .expect("unsplit server conn must have peer_addr");
    assert!(
        peer.contains("127.0.0.1"),
        "peer_addr should include client IP, got {peer}"
    );
}

struct EchoHandler;

#[async_trait]
impl Acceptor for EchoHandler {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
        assert_accept_peer(conn);
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
async fn push_then_close_channel_emits_binary_then_close() {
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
        "dave",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(IdentityDialer));
    client.connect(&addr.to_string()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if server.channel_map().contains("dave").await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("channel dave should be added after handshake");

    server
        .push("dave", Bytes::from_static(b"bye"))
        .await
        .unwrap();
    server.close_channel("dave").await.unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(2), client.read())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&frame.payload[..], b"bye");
    let closed = tokio::time::timeout(Duration::from_secs(2), client.read()).await;
    match closed {
        Ok(Err(Error::Closed)) => {}
        other => panic!("expected Closed after Binary, got {other:?}"),
    }
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

#[derive(Default)]
struct HookTrace {
    accepted: Mutex<Vec<String>>,
    abandoned: Mutex<Vec<String>>,
    ready: Mutex<Vec<String>>,
    received: Mutex<Vec<String>>,
    disconnected: Mutex<Vec<String>>,
}

struct Probe {
    log: Arc<HookTrace>,
    fail_ready: bool,
}

#[async_trait]
impl Acceptor for Probe {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
        assert_accept_peer(conn);
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| Error::HandshakeTimeout(timeout))??;
        let id = String::from_utf8_lossy(&frame.payload).to_string();
        self.log.accepted.lock().unwrap().push(id.clone());
        Ok(id)
    }

    async fn on_channel_ready(&self, id: &str) -> Result<(), Error> {
        self.log.ready.lock().unwrap().push(id.to_string());
        if self.fail_ready {
            return Err(Error::other("ready failed"));
        }
        Ok(())
    }

    async fn on_accept_abandoned(&self, id: &str) {
        self.log.abandoned.lock().unwrap().push(id.to_string());
    }
}

#[async_trait]
impl MessageListener for Probe {
    async fn receive(&self, _agent: &dyn Agent, _payload: Bytes) {
        self.log.received.lock().unwrap().push("recv".into());
    }
}

#[async_trait]
impl StateListener for Probe {
    async fn disconnect(&self, channel_id: &str) -> Result<(), Error> {
        self.log
            .disconnected
            .lock()
            .unwrap()
            .push(channel_id.to_string());
        Ok(())
    }
}

async fn wait_until<F, Fut>(mut cond: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if cond().await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("timeout waiting for condition");
}

fn snapshot(m: &Mutex<Vec<String>>) -> Vec<String> {
    m.lock().unwrap().clone()
}

async fn connect_named(addr: &str, id: &str) -> TcpClient {
    let mut client = TcpClient::new(
        id,
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(IdentityDialer));
    client.connect(addr).await.unwrap();
    client
}

#[tokio::test]
async fn duplicate_id_abandons_without_ready() {
    let log = Arc::new(HookTrace::default());
    let probe = Arc::new(Probe {
        log: log.clone(),
        fail_ready: false,
    });
    let mut server = TcpServer::bind("127.0.0.1:0").await.unwrap();
    server.set_acceptor(probe.clone());
    server.set_message_listener(probe.clone());
    server.set_state_listener(probe);
    let addr = server.local_addr().to_string();
    let server = Arc::new(server);
    let running = server.clone();
    tokio::spawn(async move {
        running.start().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(30)).await;

    let _first = connect_named(&addr, "dup").await;
    wait_until(|| {
        let server = server.clone();
        async move { server.channel_map().contains("dup").await }
    })
    .await;
    wait_until(|| {
        let log = log.clone();
        async move { snapshot(&log.ready) == ["dup".to_string()] }
    })
    .await;

    let _second = connect_named(&addr, "dup").await;
    wait_until(|| {
        let log = log.clone();
        async move { snapshot(&log.abandoned) == ["dup".to_string()] }
    })
    .await;

    assert_eq!(snapshot(&log.ready), vec!["dup".to_string()]);
    assert_eq!(snapshot(&log.abandoned), vec!["dup".to_string()]);
    assert!(snapshot(&log.disconnected).is_empty());
    let _ = server.shutdown().await;
}

#[tokio::test]
async fn missing_listener_abandons_without_ready() {
    let log = Arc::new(HookTrace::default());
    let probe = Arc::new(Probe {
        log: log.clone(),
        fail_ready: false,
    });
    let mut server = TcpServer::bind("127.0.0.1:0").await.unwrap();
    server.set_acceptor(probe.clone());
    server.set_state_listener(probe);
    let addr = server.local_addr().to_string();
    let server = Arc::new(server);
    let running = server.clone();
    tokio::spawn(async move {
        running.start().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(30)).await;

    let _client = connect_named(&addr, "solo").await;
    wait_until(|| {
        let log = log.clone();
        async move { snapshot(&log.abandoned) == ["solo".to_string()] }
    })
    .await;
    wait_until(|| {
        let server = server.clone();
        async move { !server.channel_map().contains("solo").await }
    })
    .await;

    assert_eq!(snapshot(&log.accepted), vec!["solo".to_string()]);
    assert!(snapshot(&log.ready).is_empty());
    assert!(snapshot(&log.disconnected).is_empty());
    assert!(snapshot(&log.received).is_empty());
    let _ = server.shutdown().await;
}

#[tokio::test]
async fn ready_err_closes_without_abandon() {
    let log = Arc::new(HookTrace::default());
    let probe = Arc::new(Probe {
        log: log.clone(),
        fail_ready: true,
    });
    let mut server = TcpServer::bind("127.0.0.1:0").await.unwrap();
    server.set_acceptor(probe.clone());
    server.set_message_listener(probe.clone());
    server.set_state_listener(probe);
    let addr = server.local_addr().to_string();
    let server = Arc::new(server);
    let running = server.clone();
    tokio::spawn(async move {
        running.start().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(30)).await;

    let _client = connect_named(&addr, "fail").await;
    wait_until(|| {
        let log = log.clone();
        async move { snapshot(&log.ready) == ["fail".to_string()] }
    })
    .await;
    wait_until(|| {
        let server = server.clone();
        async move { !server.channel_map().contains("fail").await }
    })
    .await;

    assert_eq!(snapshot(&log.accepted), vec!["fail".to_string()]);
    assert!(snapshot(&log.abandoned).is_empty());
    assert!(snapshot(&log.received).is_empty());
    assert!(snapshot(&log.disconnected).is_empty());
    let _ = server.shutdown().await;
}
