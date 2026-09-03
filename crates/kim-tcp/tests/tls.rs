//! TLS e2e for [`serve_conn`] plus keepalive / connection-limit checks.
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{Acceptor, ChannelHandle, Conn, Error, MessageListener, Server, StateListener};
use kim_tcp::{
    acquire_permit, apply_socket_opts, serve_conn, ClientOptions, IdentityDialer, Keepalive,
    SocketOpts, TcpClient, TcpConn, TcpServer,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use socket2::SockRef;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};

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
    async fn receive(&self, handle: &dyn ChannelHandle, payload: Bytes) {
        let mut out = payload.to_vec();
        out.extend_from_slice(b" from server");
        let _ = handle.push(Bytes::from(out)).await;
    }
}

#[async_trait]
impl StateListener for EchoHandler {
    async fn disconnect(&self, _channel_id: &str) -> Result<(), Error> {
        Ok(())
    }
}

fn test_tls() -> (Arc<ServerConfig>, Arc<ClientConfig>) {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).expect("params");
    params
        .subject_alt_names
        .push(rcgen::SanType::IpAddress(std::net::IpAddr::V4(
            std::net::Ipv4Addr::LOCALHOST,
        )));
    let key_pair = rcgen::KeyPair::generate().expect("key");
    let cert = params.self_signed(&key_pair).expect("self-signed");
    let cert_der = CertificateDer::from(cert.der().to_vec());
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der()));

    let server = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], key_der)
        .expect("server config");

    let mut roots = RootCertStore::empty();
    roots.add(cert_der).expect("root");
    let client = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    (Arc::new(server), Arc::new(client))
}

async fn spawn_tls_frontend(server: TcpServer, tls: Arc<ServerConfig>) -> SocketAddr {
    let addr = server.local_addr();
    let state = server.into_frontend_state();
    let acceptor = TlsAcceptor::from(tls);
    tokio::spawn(async move {
        let listener = state
            .take_listener()
            .await
            .expect("tls frontend requires the listener");
        let handshake_wait = Duration::from_secs(10);
        loop {
            tokio::select! {
                _ = state.shutdown_notify().notified() => break,
                accepted = listener.accept() => {
                    let Ok((mut stream, peer)) = accepted else { continue };
                    let _ = stream.set_nodelay(true);
                    let _ = apply_socket_opts(&stream, &state.socket_opts());
                    let permit = match acquire_permit(state.connection_limit().as_ref(), &mut stream).await {
                        Ok(p) => p,
                        Err(()) => continue,
                    };
                    let acceptor = acceptor.clone();
                    let ctx = state.serve_ctx();
                    state.spawn_conn(async move {
                        let _permit = permit;
                        match tokio::time::timeout(handshake_wait, acceptor.accept(stream)).await {
                            Ok(Ok(tls)) => {
                                let conn = TcpConn::with_peer(tls, Some(peer.to_string()));
                                let _ = serve_conn(conn, ctx).await;
                            }
                            Ok(Err(err)) => tracing::warn!(%err, "tls handshake failed"),
                            Err(_) => tracing::warn!("tls handshake timeout"),
                        }
                    }).await;
                }
            }
        }
    });
    addr
}

#[tokio::test]
async fn tls_echo_through_serve_conn() {
    let handler = Arc::new(EchoHandler);
    let mut server = TcpServer::bind("127.0.0.1:0").await.unwrap();
    server.set_drain_wait(Duration::from_millis(50));
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    let (server_tls, client_tls) = test_tls();
    let addr = spawn_tls_frontend(server, server_tls).await;
    tokio::time::sleep(Duration::from_millis(30)).await;

    let tcp = TcpStream::connect(addr).await.unwrap();
    let connector = TlsConnector::from(client_tls);
    let tls = connector
        .connect(ServerName::try_from("localhost").expect("name"), tcp)
        .await
        .unwrap();
    let mut conn = TcpConn::with_peer(tls, Some(addr.to_string()));
    conn.write_frame(kim_core::OpCode::Binary, Bytes::from_static(b"bob"))
        .await
        .unwrap();
    conn.flush().await.unwrap();
    conn.write_frame(kim_core::OpCode::Binary, Bytes::from_static(b"hello"))
        .await
        .unwrap();
    conn.flush().await.unwrap();
    let frame = conn.read_frame().await.unwrap();
    assert_eq!(&frame.payload[..], b"hello from server");
}

#[tokio::test]
async fn keepalive_apply_sets_so_keepalive() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let client = TcpStream::connect(addr).await.unwrap();
    let (server, _) = listener.accept().await.unwrap();
    let opts = SocketOpts {
        keepalive: Some(Keepalive::default()),
    };
    apply_socket_opts(&server, &opts).unwrap();
    apply_socket_opts(&client, &opts).unwrap();
    assert!(SockRef::from(&server).keepalive().unwrap());
    assert!(SockRef::from(&client).keepalive().unwrap());
}

#[tokio::test]
async fn max_connections_rejects_second() {
    let handler = Arc::new(EchoHandler);
    let mut server = TcpServer::bind("127.0.0.1:0").await.unwrap();
    server.set_drain_wait(Duration::from_millis(50));
    server.set_max_connections(Some(1));
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

    let mut first = TcpClient::new(
        "first",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    first.set_dialer(Arc::new(IdentityDialer));
    first.connect(&addr.to_string()).await.unwrap();

    let mut second = TcpClient::new(
        "second",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    second.set_dialer(Arc::new(IdentityDialer));
    let _ = tokio::time::timeout(
        Duration::from_millis(400),
        second.connect(&addr.to_string()),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        server.channel_map().contains("first").await,
        "first connection keeps the only permit"
    );
    assert!(
        !server.channel_map().contains("second").await,
        "second connection must be closed before handshake when max=1"
    );
    first.close().await.unwrap();
    let _ = server.shutdown().await;
}

#[tokio::test]
async fn tls_handshake_timeout_releases_permit() {
    let handler = Arc::new(EchoHandler);
    let mut server = TcpServer::bind("127.0.0.1:0").await.unwrap();
    server.set_drain_wait(Duration::from_millis(50));
    server.set_max_connections(Some(1));
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    let state = server.into_frontend_state();
    let (server_tls, client_tls) = test_tls();
    let acceptor = TlsAcceptor::from(server_tls);
    let listener = state.take_listener().await.expect("listener");
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = state.shutdown_notify().notified() => break,
                accepted = listener.accept() => {
                    let Ok((mut stream, peer)) = accepted else { continue };
                    let _ = stream.set_nodelay(true);
                    let permit = match acquire_permit(state.connection_limit().as_ref(), &mut stream).await {
                        Ok(p) => p,
                        Err(()) => continue,
                    };
                    let acceptor = acceptor.clone();
                    let ctx = state.serve_ctx();
                    state.spawn_conn(async move {
                        let _permit = permit;
                        if let Ok(Ok(tls)) = tokio::time::timeout(
                            Duration::from_millis(80),
                            acceptor.accept(stream),
                        )
                        .await
                        {
                            let conn = TcpConn::with_peer(tls, Some(peer.to_string()));
                            let _ = serve_conn(conn, ctx).await;
                        }
                    }).await;
                }
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Hold the only permit with a slow (non-TLS) client, then drop so timeout releases it.
    let stall = TcpStream::connect(addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(stall);
    tokio::time::sleep(Duration::from_millis(30)).await;

    let tcp = TcpStream::connect(addr).await.unwrap();
    let connector = TlsConnector::from(client_tls);
    let tls = tokio::time::timeout(
        Duration::from_secs(2),
        connector.connect(ServerName::try_from("localhost").expect("name"), tcp),
    )
    .await
    .expect("new connection should be accepted after handshake timeout")
    .unwrap();
    let mut conn = TcpConn::with_peer(tls, Some(addr.to_string()));
    conn.write_frame(kim_core::OpCode::Binary, Bytes::from_static(b"ok"))
        .await
        .unwrap();
    conn.flush().await.unwrap();
}

fn write_temp_pem(prefix: &str, contents: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "kim-tcp-{prefix}-{}-{}.pem",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(contents).unwrap();
    path
}

#[test]
fn rcgen_pem_roundtrip_files() {
    let key_pair = rcgen::KeyPair::generate().unwrap();
    let params = rcgen::CertificateParams::new(vec!["localhost".into()]).unwrap();
    let cert = params.self_signed(&key_pair).unwrap();
    let cert_path = write_temp_pem("cert", cert.pem().as_bytes());
    let key_path = write_temp_pem("key", key_pair.serialize_pem().as_bytes());
    assert!(std::fs::read_to_string(&cert_path)
        .unwrap()
        .contains("BEGIN"));
    assert!(std::fs::read_to_string(&key_path)
        .unwrap()
        .contains("BEGIN"));
    let _ = std::fs::remove_file(cert_path);
    let _ = std::fs::remove_file(key_path);
}
