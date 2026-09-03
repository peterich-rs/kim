use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use socket2::SockRef;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Notify, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;
use tracing::{info, warn};

use kim_core::{
    Acceptor, Channel, ChannelHandle, ChannelMap, ChannelOpts, Conn, Error, LaneKeyFn,
    MailboxFullHook, MessageListener, OpCode, Server, StateListener, WriteFullPolicy,
    DEFAULT_DRAIN_WAIT, DEFAULT_LOGIN_WAIT, DEFAULT_MAX_IN_FLIGHT,
};

use crate::conn::TcpConn;
use crate::opts::SocketOpts;

fn lock_inner<T>(m: &StdMutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// 明文 `TcpServer::start` 与 TLS 前端共用的可变状态。
pub struct FrontendState {
    listener: Mutex<Option<TcpListener>>,
    acceptor: StdMutex<Arc<dyn Acceptor>>,
    messages: StdMutex<Option<Arc<dyn MessageListener>>>,
    states: StdMutex<Option<Arc<dyn StateListener>>>,
    channels: ChannelMap,
    login_wait: StdMutex<Duration>,
    opts: StdMutex<ChannelOpts>,
    drain_wait: StdMutex<Duration>,
    socket_opts: StdMutex<SocketOpts>,
    max_connections: StdMutex<Option<Arc<Semaphore>>>,
    shutdown: Notify,
    closed: AtomicBool,
    tasks: Mutex<JoinSet<()>>,
}

impl FrontendState {
    fn new(listener: TcpListener) -> Self {
        Self {
            listener: Mutex::new(Some(listener)),
            acceptor: StdMutex::new(Arc::new(DefaultAcceptor)),
            messages: StdMutex::new(None),
            states: StdMutex::new(None),
            channels: ChannelMap::new(),
            login_wait: StdMutex::new(DEFAULT_LOGIN_WAIT),
            opts: StdMutex::new(ChannelOpts {
                in_flight: Some(Arc::new(Semaphore::new(DEFAULT_MAX_IN_FLIGHT))),
                ..ChannelOpts::default()
            }),
            drain_wait: StdMutex::new(DEFAULT_DRAIN_WAIT),
            socket_opts: StdMutex::new(SocketOpts { keepalive: None }),
            max_connections: StdMutex::new(None),
            shutdown: Notify::new(),
            closed: AtomicBool::new(false),
            tasks: Mutex::new(JoinSet::new()),
        }
    }

    pub fn serve_ctx(&self) -> ServeConnCtx {
        ServeConnCtx {
            acceptor: lock_inner(&self.acceptor).clone(),
            messages: lock_inner(&self.messages).clone(),
            states: lock_inner(&self.states).clone(),
            channels: self.channels.clone(),
            login_wait: *lock_inner(&self.login_wait),
            opts: lock_inner(&self.opts).clone(),
        }
    }

    pub async fn take_listener(&self) -> Option<TcpListener> {
        self.listener.lock().await.take()
    }

    pub fn socket_opts(&self) -> SocketOpts {
        lock_inner(&self.socket_opts).clone()
    }

    pub fn connection_limit(&self) -> Option<Arc<Semaphore>> {
        lock_inner(&self.max_connections).clone()
    }

    pub fn set_acceptor(&self, acceptor: Arc<dyn Acceptor>) {
        *lock_inner(&self.acceptor) = acceptor;
    }

    pub fn set_message_listener(&self, listener: Arc<dyn MessageListener>) {
        *lock_inner(&self.messages) = Some(listener);
    }

    pub fn set_state_listener(&self, listener: Arc<dyn StateListener>) {
        *lock_inner(&self.states) = Some(listener);
    }

    pub fn set_read_wait(&self, wait: Duration) {
        lock_inner(&self.opts).read_wait = wait;
    }

    pub fn set_write_full(&self, policy: WriteFullPolicy) {
        lock_inner(&self.opts).write_full = policy;
    }

    pub fn set_on_mailbox_full(&self, hook: MailboxFullHook) {
        lock_inner(&self.opts).on_mailbox_full = Some(hook);
    }

    pub fn set_max_connections(&self, max: Option<usize>) {
        let sem = max.map(|n| Arc::new(Semaphore::new(n.min(Semaphore::MAX_PERMITS))));
        *lock_inner(&self.max_connections) = sem;
    }

    pub fn set_socket_opts(&self, opts: SocketOpts) {
        *lock_inner(&self.socket_opts) = opts;
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub fn shutdown_notify(&self) -> &Notify {
        &self.shutdown
    }

    pub async fn spawn_conn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut tasks = self.tasks.lock().await;
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        tasks.spawn(fut);
    }

    pub async fn push(&self, channel_id: &str, payload: Bytes) -> Result<(), Error> {
        let Some(ch) = self.channels.get(channel_id).await else {
            return Err(Error::ChannelNotFound(channel_id.to_string()));
        };
        ch.push(payload).await
    }

    pub async fn close_channel(&self, channel_id: &str) -> Result<(), Error> {
        let Some(ch) = self.channels.get(channel_id).await else {
            return Err(Error::ChannelNotFound(channel_id.to_string()));
        };
        ch.close().await;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), Error> {
        self.closed.store(true, Ordering::SeqCst);
        self.shutdown.notify_waiters();
        self.shutdown.notify_one();

        let drain = *lock_inner(&self.drain_wait);
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

pub struct ServeConnCtx {
    pub acceptor: Arc<dyn Acceptor>,
    pub messages: Option<Arc<dyn MessageListener>>,
    pub states: Option<Arc<dyn StateListener>>,
    pub channels: ChannelMap,
    pub login_wait: Duration,
    pub opts: ChannelOpts,
}

/// 明文入口（等价旧私有 handle_conn）。
pub async fn serve_tcp_conn(
    stream: TcpStream,
    peer: SocketAddr,
    ctx: ServeConnCtx,
) -> Result<(), Error> {
    let _ = peer;
    serve_conn(TcpConn::new(stream), ctx).await
}

/// 泛型入口：调用方先 `TcpConn::with_peer`。
pub async fn serve_conn<S>(mut conn: TcpConn<S>, ctx: ServeConnCtx) -> Result<(), Error>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let peer = conn.peer_addr().unwrap_or_default();
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

pub struct TcpServer {
    local_addr: SocketAddr,
    state: Arc<FrontendState>,
}

impl TcpServer {
    pub async fn bind(listen: impl tokio::net::ToSocketAddrs) -> Result<Self, Error> {
        let listener = TcpListener::bind(listen).await?;
        let local_addr = listener.local_addr()?;
        Ok(Self {
            local_addr,
            state: Arc::new(FrontendState::new(listener)),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn channel_map(&self) -> ChannelMap {
        self.state.channels.clone()
    }

    pub fn frontend_state(&self) -> Arc<FrontendState> {
        self.state.clone()
    }

    pub fn into_frontend_state(self) -> Arc<FrontendState> {
        self.state
    }

    pub async fn take_listener(&self) -> Option<TcpListener> {
        self.state.take_listener().await
    }

    pub fn set_drain_wait(&mut self, wait: Duration) {
        *lock_inner(&self.state.drain_wait) = wait;
    }

    pub fn set_lane_key(&mut self, key: LaneKeyFn) {
        lock_inner(&self.state.opts).lane_key = Some(key);
    }

    pub fn set_max_in_flight(&mut self, n: usize) {
        let n = n.max(1);
        let mut opts = lock_inner(&self.state.opts);
        opts.max_in_flight = n;
        opts.in_flight = Some(Arc::new(Semaphore::new(n)));
    }

    /// `None` = 不限制。禁止传入会溢出 `Semaphore::MAX_PERMITS` 的值。
    pub fn set_max_connections(&self, max: Option<usize>) {
        self.state.set_max_connections(max);
    }

    pub fn set_socket_opts(&self, opts: SocketOpts) {
        self.state.set_socket_opts(opts);
    }
}

#[async_trait]
impl Server for TcpServer {
    fn set_acceptor(&mut self, acceptor: Arc<dyn Acceptor>) {
        self.state.set_acceptor(acceptor);
    }

    fn set_message_listener(&mut self, listener: Arc<dyn MessageListener>) {
        self.state.set_message_listener(listener);
    }

    fn set_state_listener(&mut self, listener: Arc<dyn StateListener>) {
        self.state.set_state_listener(listener);
    }

    fn set_read_wait(&mut self, wait: Duration) {
        self.state.set_read_wait(wait);
    }

    fn set_write_full(&mut self, policy: WriteFullPolicy) {
        self.state.set_write_full(policy);
    }

    fn set_on_mailbox_full(&mut self, hook: MailboxFullHook) {
        self.state.set_on_mailbox_full(hook);
    }

    async fn start(&self) -> Result<(), Error> {
        let listener = self
            .state
            .take_listener()
            .await
            .ok_or_else(|| Error::other("server already started"))?;
        if self.state.is_closed() {
            info!("tcp server already shut down");
            return Ok(());
        }
        info!(local = %self.local_addr, "tcp server listening");

        loop {
            tokio::select! {
                _ = self.state.shutdown_notify().notified() => {
                    info!("tcp server shutting down");
                    break;
                }
                accepted = listener.accept() => {
                    let (mut stream, peer) = match accepted {
                        Ok(v) => v,
                        Err(err) => {
                            warn!(%err, "accept failed");
                            continue;
                        }
                    };
                    if self.state.is_closed() {
                        continue;
                    }
                    if let Err(err) = apply_socket_opts(&stream, &self.state.socket_opts()) {
                        warn!(%err, %peer, "socket opts failed");
                    }
                    let permit = match acquire_permit(self.state.connection_limit().as_ref(), &mut stream).await {
                        Ok(p) => p,
                        Err(()) => continue,
                    };
                    let ctx = self.state.serve_ctx();
                    self.state.spawn_conn(async move {
                        let _permit = permit;
                        if let Err(err) = serve_tcp_conn(stream, peer, ctx).await {
                            warn!(%peer, %err, "connection ended");
                        }
                    }).await;
                }
            }
        }
        Ok(())
    }

    async fn push(&self, channel_id: &str, payload: Bytes) -> Result<(), Error> {
        self.state.push(channel_id, payload).await
    }

    async fn close_channel(&self, channel_id: &str) -> Result<(), Error> {
        self.state.close_channel(channel_id).await
    }

    async fn shutdown(&self) -> Result<(), Error> {
        self.state.shutdown().await
    }
}

pub fn apply_socket_opts(stream: &TcpStream, opts: &SocketOpts) -> io::Result<()> {
    opts.apply(&SockRef::from(stream))
}

/// 拿不到 permit 时立刻关流。`Ok(None)` = 不限制。
pub async fn acquire_permit(
    limit: Option<&Arc<Semaphore>>,
    stream: &mut TcpStream,
) -> Result<Option<OwnedSemaphorePermit>, ()> {
    let Some(sem) = limit else {
        return Ok(None);
    };
    match sem.clone().try_acquire_owned() {
        Ok(p) => Ok(Some(p)),
        Err(_) => {
            warn!("max connections reached");
            let _ = stream.shutdown().await;
            Err(())
        }
    }
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
