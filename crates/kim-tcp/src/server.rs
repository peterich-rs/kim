use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Notify, Semaphore};
use tokio::task::JoinSet;
use tracing::{info, warn};

use kim_core::{
    Acceptor, Channel, ChannelHandle, ChannelMap, ChannelOpts, Conn, Error, LaneKeyFn,
    MailboxFullHook, MessageListener, OpCode, Server, StateListener, WriteFullPolicy,
    DEFAULT_DRAIN_WAIT, DEFAULT_LOGIN_WAIT, DEFAULT_MAX_IN_FLIGHT,
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
    opts: ChannelOpts,
    drain_wait: Duration,
    shutdown: Notify,
    closed: AtomicBool,
    tasks: Mutex<JoinSet<()>>,
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
            opts: ChannelOpts {
                in_flight: Some(Arc::new(Semaphore::new(DEFAULT_MAX_IN_FLIGHT))),
                ..ChannelOpts::default()
            },
            drain_wait: DEFAULT_DRAIN_WAIT,
            shutdown: Notify::new(),
            closed: AtomicBool::new(false),
            tasks: Mutex::new(JoinSet::new()),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn channel_map(&self) -> ChannelMap {
        self.channels.clone()
    }

    pub fn set_drain_wait(&mut self, wait: Duration) {
        self.drain_wait = wait;
    }

    pub fn set_lane_key(&mut self, key: LaneKeyFn) {
        self.opts.lane_key = Some(key);
    }

    pub fn set_max_in_flight(&mut self, n: usize) {
        let n = n.max(1);
        self.opts.max_in_flight = n;
        self.opts.in_flight = Some(Arc::new(Semaphore::new(n)));
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
        self.opts.read_wait = wait;
    }

    fn set_write_full(&mut self, policy: WriteFullPolicy) {
        self.opts.write_full = policy;
    }

    fn set_on_mailbox_full(&mut self, hook: MailboxFullHook) {
        self.opts.on_mailbox_full = Some(hook);
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
                    if self.closed.load(Ordering::SeqCst) {
                        continue;
                    }
                    let ctx = ConnCtx {
                        acceptor: self.acceptor.clone(),
                        messages: self.messages.clone(),
                        states: self.states.clone(),
                        channels: self.channels.clone(),
                        login_wait: self.login_wait,
                        opts: self.opts.clone(),
                    };
                    let mut tasks = self.tasks.lock().await;
                    if self.closed.load(Ordering::SeqCst) {
                        continue;
                    }
                    tasks.spawn(async move {
                        if let Err(err) = handle_conn(stream, peer, ctx).await {
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

    async fn close_channel(&self, channel_id: &str) -> Result<(), Error> {
        let Some(ch) = self.channels.get(channel_id).await else {
            return Err(Error::ChannelNotFound(channel_id.to_string()));
        };
        ch.close().await;
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), Error> {
        self.closed.store(true, Ordering::SeqCst);
        self.shutdown.notify_waiters();
        self.shutdown.notify_one();

        let drain = self.drain_wait;
        {
            let mut tasks = self.tasks.lock().await;
            let timed_out =
                tokio::time::timeout(drain, async { while tasks.join_next().await.is_some() {} })
                    .await
                    .is_err();
            if timed_out {
                info!(?drain, "tcp drain timed out; closing connections");
            }
        }

        for ch in self.channels.all().await {
            ch.close().await;
        }

        let mut tasks = self.tasks.lock().await;
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        Ok(())
    }
}

struct ConnCtx {
    acceptor: Arc<dyn Acceptor>,
    messages: Option<Arc<dyn MessageListener>>,
    states: Option<Arc<dyn StateListener>>,
    channels: ChannelMap,
    login_wait: Duration,
    opts: ChannelOpts,
}

async fn handle_conn(
    stream: tokio::net::TcpStream,
    peer: SocketAddr,
    ctx: ConnCtx,
) -> Result<(), Error> {
    let mut conn = TcpConn::new(stream);
    let id = match tokio::time::timeout(
        ctx.login_wait,
        ctx.acceptor.accept(&mut conn, ctx.login_wait),
    )
    .await
    {
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
            return Err(Error::HandshakeTimeout(ctx.login_wait));
        }
    };

    if ctx.channels.contains(&id).await {
        let _ = conn
            .write_frame(OpCode::Close, Bytes::from_static(b"channelId is repeated"))
            .await;
        let _ = conn.shutdown().await;
        ctx.acceptor.on_accept_abandoned(&id).await;
        return Err(Error::ChannelExists(id));
    }

    let (reader, writer) = conn.into_split();
    let (channel, read_loop) = Channel::pair(id.clone(), reader, writer, ctx.opts);
    ctx.channels.add(channel).await;
    info!(%peer, channel = %id, "accepted");

    let Some(messages) = ctx.messages else {
        ctx.acceptor.on_accept_abandoned(&id).await;
        if let Some(ch) = ctx.channels.get(&id).await {
            ch.close().await;
        }
        ctx.channels.remove(&id).await;
        return Err(Error::other("MessageListener is not set"));
    };

    if let Err(err) = ctx.acceptor.on_channel_ready(&id).await {
        if let Some(ch) = ctx.channels.get(&id).await {
            ch.close().await;
        }
        ctx.channels.remove(&id).await;
        return Err(err);
    }

    let read_result = read_loop.run(messages).await;
    ctx.channels.remove(&id).await;
    if let Some(states) = ctx.states {
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
