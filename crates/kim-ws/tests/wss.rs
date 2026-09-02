//! TLS terminator in front of plaintext [`WsServer`]: same shape as a reverse proxy.
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{
    Acceptor, ChannelHandle, Conn, DialerContext, Error, MessageListener, Server, StateListener,
};
use kim_ws::{connect_ws_with_tls, ClientOptions, WsClient, WsDialer, WsHandshakeConn, WsServer};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

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

async fn spawn_tls_proxy(backend: SocketAddr, server_tls: Arc<ServerConfig>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("tls bind");
    let addr = listener.local_addr().expect("tls addr");
    let acceptor = TlsAcceptor::from(server_tls);
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                break;
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let Ok(mut tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let Ok(mut back) = TcpStream::connect(backend).await else {
                    return;
                };
                let _ = copy_bidirectional(&mut tls, &mut back).await;
            });
        }
    });
    addr
}

struct TlsDialer {
    tls: Arc<ClientConfig>,
}

#[async_trait]
impl WsDialer for TlsDialer {
    async fn dial_and_handshake(&self, ctx: DialerContext) -> Result<WsHandshakeConn, Error> {
        let mut conn = connect_ws_with_tls(&ctx.address, Some(self.tls.clone())).await?;
        conn.write_frame(kim_core::OpCode::Binary, Bytes::from(ctx.id.into_bytes()))
            .await?;
        Ok(conn)
    }
}

#[tokio::test]
async fn wss_echo_through_tls_terminator() {
    let handler = Arc::new(EchoHandler);
    let mut server = WsServer::bind("127.0.0.1:0").await.unwrap();
    server.set_drain_wait(Duration::from_millis(50));
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    let backend = server.local_addr();
    let server = Arc::new(server);
    let running = server.clone();
    tokio::spawn(async move {
        running.start().await.unwrap();
    });

    let (server_tls, client_tls) = test_tls();
    let tls_addr = spawn_tls_proxy(backend, server_tls).await;

    let url = format!("wss://{tls_addr}/");
    let mut client = WsClient::new(
        "bob",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(TlsDialer { tls: client_tls }));
    client.connect(&url).await.unwrap();
    client.send(Bytes::from_static(b"hello")).await.unwrap();
    let frame = client.read().await.unwrap();
    assert_eq!(&frame.payload[..], b"hello from server");
    client.close().await.unwrap();
    let _ = server.shutdown().await;
}
