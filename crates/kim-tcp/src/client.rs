use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::Mutex;
use tracing::debug;

use kim_core::{
    Conn, DialerContext, Error, Frame, OpCode, DEFAULT_HEARTBEAT, DEFAULT_LOGIN_WAIT,
    DEFAULT_WRITE_WAIT,
};

use crate::conn::{TcpConn, TcpReadHalf, TcpWriteHalf};

/// TCP 客户端专用拨号器：必须返回可拆分的 [`TcpConn`]。
#[async_trait]
pub trait TcpDialer: Send + Sync {
    async fn dial_and_handshake(&self, ctx: DialerContext) -> Result<TcpConn, Error>;
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

pub struct TcpClient {
    id: String,
    name: String,
    dialer: Option<Arc<dyn TcpDialer>>,
    reader: Option<Mutex<TcpReadHalf>>,
    writer: Option<Arc<Mutex<TcpWriteHalf>>>,
    connected: AtomicBool,
    options: ClientOptions,
}

impl TcpClient {
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

    pub fn set_dialer(&mut self, dialer: Arc<dyn TcpDialer>) {
        self.dialer = Some(dialer);
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
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
            Ok(Ok(conn)) => conn,
            Ok(Err(err)) => {
                self.connected.store(false, Ordering::SeqCst);
                return Err(err);
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
            let id = self.id.clone();
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(interval);
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    tick.tick().await;
                    debug!(client = %id, "send ping");
                    let mut guard = writer.lock().await;
                    let ping = tokio::time::timeout(
                        write_wait,
                        guard.write_frame(OpCode::Ping, Bytes::new()),
                    )
                    .await;
                    if ping.is_err() || matches!(ping, Ok(Err(_))) {
                        break;
                    }
                    if guard.flush().await.is_err() {
                        break;
                    }
                }
            });
        }
        Ok(())
    }

    pub async fn send(&self, payload: Bytes) -> Result<(), Error> {
        let writer = self.writer.as_ref().ok_or(Error::NotConnected)?;
        let mut guard = writer.lock().await;
        tokio::time::timeout(
            self.options.write_wait,
            guard.write_frame(OpCode::Binary, payload),
        )
        .await
        .map_err(|_| Error::other("write timeout"))??;
        guard.flush().await
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
                    if let Some(writer) = &self.writer {
                        let mut guard = writer.lock().await;
                        let _ = guard.write_frame(OpCode::Pong, Bytes::new()).await;
                        let _ = guard.flush().await;
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
        self.shutdown().await?;
        self.reader.take();
        Ok(())
    }

    /// Close the write half without `&mut self` so `Arc<TcpClient>` can drop a slot.
    pub async fn shutdown(&self) -> Result<(), Error> {
        self.connected.store(false, Ordering::SeqCst);
        if let Some(writer) = &self.writer {
            let mut guard = writer.lock().await;
            let _ = guard.write_frame(OpCode::Close, Bytes::new()).await;
            let _ = guard.shutdown().await;
        }
        Ok(())
    }
}

/// 最简单的拨号：TCP 连上后，把 client id 当第一帧发过去（echo / mock 握手）。
pub struct IdentityDialer;

#[async_trait]
impl TcpDialer for IdentityDialer {
    async fn dial_and_handshake(&self, ctx: DialerContext) -> Result<TcpConn, Error> {
        let stream = tokio::net::TcpStream::connect(&ctx.address).await?;
        let mut conn = TcpConn::new(stream);
        conn.write_frame(OpCode::Binary, Bytes::from(ctx.id.into_bytes()))
            .await?;
        conn.flush().await?;
        Ok(conn)
    }
}
