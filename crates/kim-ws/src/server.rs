use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::Empty;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Notify};
use tracing::{info, warn};

use kim_core::{
    Acceptor, Agent, Channel, ChannelMap, ChannelOpts, Conn, Error, MessageListener, OpCode,
    Server, StateListener, DEFAULT_LOGIN_WAIT, DEFAULT_READ_WAIT, DEFAULT_WRITE_WAIT,
};

use crate::conn::WsConn;

pub struct WsServer {
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

impl WsServer {
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
impl Server for WsServer {
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
            return Ok(());
        }
        info!(local = %self.local_addr, "ws server listening");

        loop {
            tokio::select! {
                _ = self.shutdown.notified() => {
                    info!("ws server shutting down");
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
                    let ctx = HttpCtx {
                        acceptor: self.acceptor.clone(),
                        messages: self.messages.clone(),
                        states: self.states.clone(),
                        channels: self.channels.clone(),
                        login_wait: self.login_wait,
                        read_wait: self.read_wait,
                        write_wait: self.write_wait,
                    };
                    tokio::spawn(async move {
                        let io = TokioIo::new(stream);
                        let svc = service_fn(move |req| {
                            let ctx = ctx.clone();
                            async move { handle_http(req, ctx).await }
                        });
                        if let Err(err) = http1::Builder::new()
                            .serve_connection(io, svc)
                            .with_upgrades()
                            .await
                        {
                            warn!(%peer, %err, "http connection");
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

#[derive(Clone)]
struct HttpCtx {
    acceptor: Arc<dyn Acceptor>,
    messages: Option<Arc<dyn MessageListener>>,
    states: Option<Arc<dyn StateListener>>,
    channels: ChannelMap,
    login_wait: Duration,
    read_wait: Duration,
    write_wait: Duration,
}

async fn handle_http(
    mut req: Request<Incoming>,
    ctx: HttpCtx,
) -> Result<Response<Empty<Bytes>>, Infallible> {
    let path = req.uri().path();
    if path != "/" && path != "/ws" {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Empty::new())
            .unwrap());
    }
    if !fastwebsockets::upgrade::is_upgrade_request(&req) {
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Empty::new())
            .unwrap());
    }
    let (response, fut) = match fastwebsockets::upgrade::upgrade(&mut req) {
        Ok(v) => v,
        Err(_) => {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Empty::new())
                .unwrap());
        }
    };
    tokio::spawn(async move {
        match fut.await {
            Ok(mut ws) => {
                ws.set_auto_pong(false);
                ws.set_auto_close(false);
                ws.set_writev(true);
                ws.set_max_message_size(1024 * 1024);
                let conn = WsConn { ws };
                if let Err(err) = handle_ws(conn, ctx).await {
                    warn!(%err, "ws session ended");
                }
            }
            Err(err) => warn!(%err, "ws upgrade failed"),
        }
    });
    Ok(response)
}

async fn handle_ws<S>(mut conn: WsConn<S>, ctx: HttpCtx) -> Result<(), Error>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let id = match tokio::time::timeout(ctx.login_wait, ctx.acceptor.accept(&mut conn, ctx.login_wait))
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
        return Err(Error::ChannelExists(id));
    }
    let (reader, writer) = conn.into_split();
    let opts = ChannelOpts {
        read_wait: ctx.read_wait,
        write_wait: ctx.write_wait,
        write_queue: 64,
    };
    let (channel, read_loop) = Channel::pair(id.clone(), reader, writer, opts);
    ctx.channels.add(channel).await;
    let Some(messages) = ctx.messages else {
        ctx.channels.remove(&id).await;
        return Err(Error::other("MessageListener is not set"));
    };
    let read_result = read_loop.run(messages).await;
    ctx.channels.remove(&id).await;
    if let Some(states) = ctx.states {
        let _ = states.disconnect(&id).await;
    }
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
