use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use fastwebsockets::handshake;
use http_body_util::Empty;
use hyper::header::{CONNECTION, HOST, UPGRADE, USER_AGENT};
use hyper::upgrade::Upgraded;
use hyper::Request;
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::TlsConnector;
use tracing::debug;

use kim_core::{
    Conn, DialerContext, Error, Frame, OpCode, DEFAULT_HEARTBEAT, DEFAULT_LOGIN_WAIT,
    DEFAULT_WRITE_WAIT,
};

use crate::conn::{WsConn, WsReadHalf, WsWriteHalf};

type UpgradedIo = TokioIo<Upgraded>;

/// Default `User-Agent` on the HTTP Upgrade request when the caller does not set one.
pub const DEFAULT_USER_AGENT: &str = "kim-ws";

/// HTTP Upgrade 完成后、尚未 split 的客户端连接。实现 Conn；不暴露 hyper::upgrade::Upgraded。
pub struct WsHandshakeConn {
    inner: WsConn<UpgradedIo>,
}

impl WsHandshakeConn {
    pub(crate) fn into_split(self) -> (WsReadHalf<UpgradedIo>, WsWriteHalf<UpgradedIo>) {
        self.inner.into_split()
    }

    /// Read and write halves as [`Conn`] trait objects. `kim-client` pumps them
    /// on separate tasks so `recv` does not stall `talk`.
    pub fn split_conn(self) -> (Box<dyn Conn + Send>, Box<dyn Conn + Send>) {
        let (read, write) = self.into_split();
        (Box::new(read), Box::new(write))
    }
}

#[async_trait]
impl Conn for WsHandshakeConn {
    async fn read_frame(&mut self) -> Result<Frame, Error> {
        self.inner.read_frame().await
    }

    async fn write_frame(&mut self, opcode: OpCode, payload: Bytes) -> Result<(), Error> {
        self.inner.write_frame(opcode, payload).await
    }

    async fn flush(&mut self) -> Result<(), Error> {
        self.inner.flush().await
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        self.inner.shutdown().await
    }

    fn peer_addr(&self) -> Option<String> {
        self.inner.peer_addr()
    }
}

/// 只做 HTTP Upgrade，不发业务包、不 flush。
///
/// `ws://` is plaintext TCP then Upgrade. `wss://` is TLS then the same Upgrade
/// (TLS below HTTP; [`WsServer`] stays plaintext and expects a terminator).
pub async fn connect_ws(url: &str) -> Result<WsHandshakeConn, Error> {
    connect_ws_with_tls(url, None).await
}

/// Like [`connect_ws`], with an explicit `User-Agent` on the Upgrade request.
pub async fn connect_ws_with_user_agent(
    url: &str,
    user_agent: &str,
) -> Result<WsHandshakeConn, Error> {
    connect_ws_inner(url, None, user_agent).await
}

/// Like [`connect_ws`], with an optional rustls client config (tests / extra CAs).
/// `None` on `wss://` uses Mozilla roots via `webpki-roots`.
pub async fn connect_ws_with_tls(
    url: &str,
    tls: Option<Arc<ClientConfig>>,
) -> Result<WsHandshakeConn, Error> {
    connect_ws_inner(url, tls, DEFAULT_USER_AGENT).await
}

async fn connect_ws_inner(
    url: &str,
    tls: Option<Arc<ClientConfig>>,
    user_agent: &str,
) -> Result<WsHandshakeConn, Error> {
    let parsed = parse_ws_url(url)?;
    let ua = if user_agent.trim().is_empty() {
        DEFAULT_USER_AGENT
    } else {
        user_agent
    };
    let stream = TcpStream::connect(&parsed.connect)
        .await
        .map_err(Error::from)?;
    if parsed.tls {
        let cfg = match tls {
            Some(c) => c,
            None => default_client_tls()?,
        };
        install_ring_provider();
        let name = ServerName::try_from(parsed.sni.clone())
            .map_err(|_| Error::other(format!("invalid tls name {}", parsed.sni)))?;
        let tls_stream = TlsConnector::from(cfg)
            .connect(name, stream)
            .await
            .map_err(|e| Error::other(e.to_string()))?;
        upgrade_http(&parsed.hostport, &parsed.path, tls_stream, ua).await
    } else {
        upgrade_http(&parsed.hostport, &parsed.path, stream, ua).await
    }
}

fn install_ring_provider() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn default_client_tls() -> Result<Arc<ClientConfig>, Error> {
    install_ring_provider();
    static CFG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    Ok(CFG
        .get_or_init(|| {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            Arc::new(
                ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            )
        })
        .clone())
}

async fn upgrade_http<S>(
    hostport: &str,
    path: &str,
    stream: S,
    user_agent: &str,
) -> Result<WsHandshakeConn, Error>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let req = Request::builder()
        .method("GET")
        .uri(format!("http://{hostport}{path}"))
        .header(HOST, hostport)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(USER_AGENT, user_agent)
        .header("Sec-WebSocket-Key", handshake::generate_key())
        .header("Sec-WebSocket-Version", "13")
        .body(Empty::<Bytes>::new())
        .map_err(|e| Error::other(e.to_string()))?;
    let (mut ws, _) = handshake::client(&SpawnExecutor, req, stream)
        .await
        .map_err(|e| Error::other(e.to_string()))?;
    ws.set_auto_pong(false);
    ws.set_auto_close(false);
    ws.set_writev(true);
    ws.set_max_message_size(1024 * 1024);
    Ok(WsHandshakeConn {
        inner: WsConn::new(ws, None),
    })
}

pub struct WsClient {
    id: String,
    name: String,
    dialer: Option<Arc<dyn WsDialer>>,
    reader: Option<Mutex<WsReadHalf<UpgradedIo>>>,
    writer: Option<Arc<Mutex<WsWriteHalf<UpgradedIo>>>>,
    connected: AtomicBool,
    options: ClientOptions,
}

#[derive(Clone, Debug)]
pub struct ClientOptions {
    pub heartbeat: Option<Duration>,
    pub write_wait: Duration,
    pub handshake_timeout: Duration,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            heartbeat: Some(DEFAULT_HEARTBEAT),
            write_wait: DEFAULT_WRITE_WAIT,
            handshake_timeout: DEFAULT_LOGIN_WAIT,
        }
    }
}

#[async_trait]
pub trait WsDialer: Send + Sync {
    async fn dial_and_handshake(&self, ctx: DialerContext) -> Result<WsHandshakeConn, Error>;
}

impl WsClient {
    pub fn new(id: impl Into<String>, name: impl Into<String>, options: ClientOptions) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            dialer: None,
            reader: None,
            writer: None,
            connected: AtomicBool::new(false),
            options,
        }
    }

    pub fn set_dialer(&mut self, dialer: Arc<dyn WsDialer>) {
        self.dialer = Some(dialer);
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn connect(&mut self, addr: &str) -> Result<(), Error> {
        if self.connected.swap(true, Ordering::SeqCst) {
            return Err(Error::AlreadyConnected);
        }
        let dialer = self
            .dialer
            .clone()
            .ok_or_else(|| Error::other("dialer not set"))?;
        let ctx = DialerContext {
            id: self.id.clone(),
            name: self.name.clone(),
            address: addr.to_string(),
            timeout: self.options.handshake_timeout,
        };
        let conn = match tokio::time::timeout(ctx.timeout, dialer.dial_and_handshake(ctx)).await {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => {
                self.connected.store(false, Ordering::SeqCst);
                return Err(e);
            }
            Err(_) => {
                self.connected.store(false, Ordering::SeqCst);
                return Err(Error::HandshakeTimeout(self.options.handshake_timeout));
            }
        };
        let (reader, writer) = conn.into_split();
        let writer = Arc::new(Mutex::new(writer));
        self.reader = Some(Mutex::new(reader));
        self.writer = Some(writer.clone());
        if let Some(interval) = self.options.heartbeat {
            let write_wait = self.options.write_wait;
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(interval);
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    tick.tick().await;
                    let mut g = writer.lock().await;
                    if tokio::time::timeout(write_wait, g.write_frame(OpCode::Ping, Bytes::new()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }
        Ok(())
    }

    pub async fn send(&self, payload: Bytes) -> Result<(), Error> {
        let writer = self.writer.as_ref().ok_or(Error::NotConnected)?;
        let mut g = writer.lock().await;
        g.write_frame(OpCode::Binary, payload).await?;
        g.flush().await
    }

    pub async fn read(&self) -> Result<Frame, Error> {
        let mut reader = self
            .reader
            .as_ref()
            .ok_or(Error::NotConnected)?
            .lock()
            .await;
        loop {
            let frame = reader.read_frame().await?;
            match frame.opcode {
                OpCode::Close => return Err(Error::Closed),
                OpCode::Ping => {
                    if let Some(w) = &self.writer {
                        let mut g = w.lock().await;
                        let _ = g.write_frame(OpCode::Pong, Bytes::new()).await;
                    }
                }
                OpCode::Pong => {
                    debug!(client = %self.id, "recv pong");
                }
                OpCode::Binary | OpCode::Text => return Ok(frame),
                OpCode::Continuation => {}
            }
        }
    }

    pub async fn close(&mut self) -> Result<(), Error> {
        self.connected.store(false, Ordering::SeqCst);
        if let Some(w) = self.writer.take() {
            let mut g = w.lock().await;
            let _ = g.write_frame(OpCode::Close, Bytes::new()).await;
            let _ = g.shutdown().await;
        }
        self.reader.take();
        Ok(())
    }
}

pub struct WsIdentityDialer;

struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: std::future::Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::spawn(fut);
    }
}

struct ParsedWsUrl {
    tls: bool,
    /// Host header / request authority (may omit default port).
    hostport: String,
    /// `host:port` for `TcpStream::connect`.
    connect: String,
    /// rustls SNI (no port).
    sni: String,
    path: String,
}

fn parse_ws_url(url: &str) -> Result<ParsedWsUrl, Error> {
    let (tls, rest) = if let Some(r) = url.strip_prefix("wss://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("ws://") {
        (false, r)
    } else {
        return Err(Error::other("url must start with ws:// or wss://"));
    };
    let (hostport, path) = rest.split_once('/').unwrap_or((rest, ""));
    if hostport.is_empty() {
        return Err(Error::other("url missing host"));
    }
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{path}")
    };
    let default_port = if tls { 443 } else { 80 };
    let connect = with_default_port(hostport, default_port);
    let sni = sni_host(hostport)?;
    Ok(ParsedWsUrl {
        tls,
        hostport: hostport.to_string(),
        connect,
        sni,
        path,
    })
}

fn with_default_port(hostport: &str, default: u16) -> String {
    if let Some(rest) = hostport.strip_prefix('[') {
        if rest.contains("]:") {
            return hostport.to_string();
        }
        return format!("{hostport}:{default}");
    }
    if hostport.rsplit_once(':').is_some() {
        return hostport.to_string();
    }
    format!("{hostport}:{default}")
}

fn sni_host(hostport: &str) -> Result<String, Error> {
    if let Some(rest) = hostport.strip_prefix('[') {
        let end = rest
            .find(']')
            .ok_or_else(|| Error::other("invalid ipv6 host"))?;
        return Ok(rest[..end].to_string());
    }
    Ok(match hostport.rsplit_once(':') {
        Some((h, _)) => h.to_string(),
        None => hostport.to_string(),
    })
}

#[async_trait]
impl WsDialer for WsIdentityDialer {
    async fn dial_and_handshake(&self, ctx: DialerContext) -> Result<WsHandshakeConn, Error> {
        let mut conn = connect_ws(&ctx.address).await?;
        conn.write_frame(OpCode::Binary, Bytes::from(ctx.id.into_bytes()))
            .await?;
        Ok(conn)
    }
}

#[cfg(test)]
mod parse_tests {
    use super::parse_ws_url;

    #[test]
    fn ws_keeps_explicit_port() {
        let p = parse_ws_url("ws://127.0.0.1:8001/").unwrap();
        assert!(!p.tls);
        assert_eq!(p.connect, "127.0.0.1:8001");
        assert_eq!(p.path, "/");
        assert_eq!(p.sni, "127.0.0.1");
    }

    #[test]
    fn wss_defaults_to_443() {
        let p = parse_ws_url("wss://kim.example/chat").unwrap();
        assert!(p.tls);
        assert_eq!(p.connect, "kim.example:443");
        assert_eq!(p.hostport, "kim.example");
        assert_eq!(p.sni, "kim.example");
        assert_eq!(p.path, "/chat");
    }

    #[test]
    fn rejects_http() {
        assert!(parse_ws_url("http://127.0.0.1:8001/").is_err());
    }
}
