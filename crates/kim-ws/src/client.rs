use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use fastwebsockets::handshake;
use http_body_util::Empty;
use hyper::header::{CONNECTION, HOST, UPGRADE};
use hyper::upgrade::Upgraded;
use hyper::Request;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::debug;

use kim_core::{
    Conn, DialerContext, Error, Frame, OpCode, DEFAULT_HEARTBEAT, DEFAULT_LOGIN_WAIT,
    DEFAULT_WRITE_WAIT,
};

use crate::conn::{WsConn, WsReadHalf, WsWriteHalf};

type UpgradedIo = TokioIo<Upgraded>;

pub struct WsClient {
    id: String,
    name: String,
    dialer: Option<Arc<dyn WsDialer>>,
    reader: Option<Mutex<WsReadHalf<UpgradedIo>>>,
    writer: Option<Arc<Mutex<WsWriteHalf<UpgradedIo>>>>,
    connected: AtomicBool,
    options: crate::ClientOptions,
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
    async fn dial_and_handshake(
        &self,
        ctx: DialerContext,
    ) -> Result<WsConn<UpgradedIo>, Error>;
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
        if addr.starts_with("wss://") {
            return Err(Error::other("本阶段无 TLS，请用 ws://"));
        }
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
        let mut reader = self.reader.as_ref().ok_or(Error::NotConnected)?.lock().await;
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

fn parse_ws_url(url: &str) -> Result<(String, String), Error> {
    let rest = url
        .strip_prefix("ws://")
        .ok_or_else(|| Error::other("url must start with ws://"))?;
    let (hostport, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{path}")
    };
    Ok((hostport.to_string(), path))
}

#[async_trait]
impl WsDialer for WsIdentityDialer {
    async fn dial_and_handshake(
        &self,
        ctx: DialerContext,
    ) -> Result<WsConn<UpgradedIo>, Error> {
        let (hostport, path) = parse_ws_url(&ctx.address)?;
        let stream = TcpStream::connect(&hostport)
            .await
            .map_err(Error::from)?;
        let req = Request::builder()
            .method("GET")
            .uri(format!("http://{hostport}{path}"))
            .header(HOST, hostport)
            .header(UPGRADE, "websocket")
            .header(CONNECTION, "upgrade")
            .header(
                "Sec-WebSocket-Key",
                fastwebsockets::handshake::generate_key(),
            )
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
        let mut conn = WsConn { ws };
        conn.write_frame(OpCode::Binary, Bytes::from(ctx.id.into_bytes()))
            .await?;
        Ok(conn)
    }
}
