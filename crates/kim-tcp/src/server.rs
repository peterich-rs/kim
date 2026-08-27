use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Notify};
use tracing::{info, warn};

use kim_core::{
    Acceptor, Agent, Channel, ChannelMap, ChannelOpts, Conn, Error, MessageListener, OpCode,
    Server, StateListener, DEFAULT_LOGIN_WAIT, DEFAULT_READ_WAIT, DEFAULT_WRITE_WAIT,
};

use crate::conn::TcpConn;

pub struct TcpServer {
    local_addr: SocketAddr,
    listener: Mutex<Option<TcpListener>>,
    acceptor: Arc<dyn Acceptor>,
    messages: Option<Arc<dyn MessageListener>>,
    states: Option<Arc<dyn StateListener>>,
    channels: ChannelMap,
    login_wait: Duration,
    read_wait: Duration,
    write_wait: Duration,
    shutdown: Notify,
    closed: AtomicBool,
}

impl TcpServer {
    pub async fn bind(listen: impl tokio::net::ToSocketAddrs) -> Result<Self, Error> {
        let listener = TcpListener::bind(listen).await?;
        let local_addr = listener.local_addr()?;
        Ok(Self {
            local_addr,
            listener: Mutex::new(Some(listener)),
            acceptor: Arc::new(DefaultAcceptor),
            messages: None,
            states: None,
            channels: ChannelMap::new(),
            login_wait: DEFAULT_LOGIN_WAIT,
            read_wait: DEFAULT_READ_WAIT,
            write_wait: DEFAULT_WRITE_WAIT,
            shutdown: Notify::new(),
            closed: AtomicBool::new(false),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn channel_map(&self) -> ChannelMap {
        self.channels.clone()
    }
}

#[async_trait]
impl Server for TcpServer {
    fn set_acceptor(&mut self, acceptor: Arc<dyn Acceptor>) {
        self.acceptor = acceptor;
    }

    fn set_message_listener(&mut self, listener: Arc<dyn MessageListener>) {
        self.messages = Some(listener);
    }

    fn set_state_listener(&mut self, listener: Arc<dyn StateListener>) {
        self.states = Some(listener);
    }

    fn set_read_wait(&mut self, wait: Duration) {
        self.read_wait = wait;
    }

    async fn start(&self) -> Result<(), Error> {
        let listener = self
            .listener
            .lock()
            .await
            .take()
            .ok_or_else(|| Error::other("server already started"))?;
        if self.closed.load(Ordering::SeqCst) {
            info!("tcp server already shut down");
            return Ok(());
        }
        info!(local = %self.local_addr, "tcp server listening");

        loop {
            tokio::select! {
                _ = self.shutdown.notified() => {
                    info!("tcp server shutting down");
                    break;
                }
                accepted = listener.accept() => {
                    let (stream, peer) = match accepted {
                        Ok(v) => v,
                        Err(err) => {
                            warn!(%err, "accept failed");
                            continue;
                        }
                    };
                    let acceptor = self.acceptor.clone();
                    let messages = self.messages.clone();
                    let states = self.states.clone();
                    let channels = self.channels.clone();
                    let login_wait = self.login_wait;
                    let opts = ChannelOpts {
                        read_wait: self.read_wait,
                        write_wait: self.write_wait,
                        write_queue: 64,
                    };
                    tokio::spawn(async move {
                        if let Err(err) = handle_conn(
                            stream, peer, acceptor, messages, states, channels, login_wait, opts,
                        )
                        .await
                        {
                            warn!(%peer, %err, "connection ended");
                        }
                    });
                }
            }
        }
        Ok(())
    }

    async fn push(&self, channel_id: &str, payload: Bytes) -> Result<(), Error> {
        let Some(ch) = self.channels.get(channel_id).await else {
            return Err(Error::ChannelNotFound(channel_id.to_string()));
        };
        ch.push(payload).await
    }

    async fn shutdown(&self) -> Result<(), Error> {
        self.closed.store(true, Ordering::SeqCst);
        self.shutdown.notify_waiters();
        self.shutdown.notify_one();
        Ok(())
    }
}

async fn handle_conn(
    stream: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
    acceptor: Arc<dyn Acceptor>,
    messages: Option<Arc<dyn MessageListener>>,
    states: Option<Arc<dyn StateListener>>,
    channels: ChannelMap,
    login_wait: Duration,
    opts: ChannelOpts,
) -> Result<(), Error> {
    let mut conn = TcpConn::new(stream);
    let id = match tokio::time::timeout(login_wait, acceptor.accept(&mut conn, login_wait)).await {
        Ok(Ok(id)) => id,
        Ok(Err(err)) => {
            let _ = conn
                .write_frame(OpCode::Close, Bytes::from(err.to_string()))
                .await;
            let _ = conn.shutdown().await;
            return Err(err);
        }
        Err(_) => {
            let _ = conn
                .write_frame(OpCode::Close, Bytes::from_static(b"handshake timeout"))
                .await;
            let _ = conn.shutdown().await;
            return Err(Error::HandshakeTimeout(login_wait));
        }
    };

    if channels.contains(&id).await {
        let _ = conn
            .write_frame(OpCode::Close, Bytes::from_static(b"channelId is repeated"))
            .await;
        let _ = conn.shutdown().await;
        return Err(Error::ChannelExists(id));
    }

    let (reader, writer) = conn.into_split();
    let (channel, read_loop) = Channel::pair(id.clone(), reader, writer, opts);
    channels.add(channel).await;
    info!(%peer, channel = %id, "accepted");

    let Some(messages) = messages else {
        channels.remove(&id).await;
        return Err(Error::other("MessageListener is not set"));
    };

    let read_result = read_loop.run(messages).await;
    channels.remove(&id).await;
    if let Some(states) = states {
        let _ = states.disconnect(&id).await;
    }
    info!(channel = %id, "disconnected");
    read_result
}

struct DefaultAcceptor;

#[async_trait]
impl Acceptor for DefaultAcceptor {
    async fn accept(&self, _conn: &mut dyn Conn, _timeout: Duration) -> Result<String, Error> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(1);
        Ok(format!("ch-{}", SEQ.fetch_add(1, Ordering::Relaxed)))
    }
}
