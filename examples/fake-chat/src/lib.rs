//! Chat demo: session lookup, Router, login / logout / echo handlers.

pub mod directory;
mod echo;
pub mod idgen;
mod login;
pub mod store;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_container::Container;
use kim_core::{Acceptor, Agent, Conn, Error, MessageListener, StateListener};
use kim_protocol::pkt::{Flag, InnerHandshakeReq, Session, Status};
use kim_protocol::{
    read_logic, CMD_DEMO_ECHO, CMD_LOGIN_SIGN_IN, CMD_LOGIN_SIGN_OUT, META_DEST_CHANNELS,
    META_DEST_SERVER,
};
use kim_router::{Dispatcher, Router, RouterError, SessionError, SessionStorage};
use prost::Message;
use tracing::{info, warn};

use crate::directory::{GroupDirectory, MemoryGroupDirectory};
use crate::idgen::{resolve_snowflake_node, IdGenerator, SequenceIdGen, SnowflakeGen};
use crate::store::{MemoryMessageStore, MessageStore};

pub use echo::do_echo;
pub use login::{do_sys_login, do_sys_logout};

#[derive(Clone)]
#[allow(dead_code)] // PR3/4 capture svc in talk/create handlers
pub(crate) struct ChatSvc {
    store: Arc<dyn MessageStore>,
    groups: Arc<dyn GroupDirectory>,
}

struct ContainerDispatcher(Arc<Container>);

#[async_trait]
impl Dispatcher for ContainerDispatcher {
    async fn push(
        &self,
        gateway: &str,
        channels: &[String],
        mut pkt: kim_protocol::LogicPkt,
    ) -> Result<(), RouterError> {
        pkt.set_meta(META_DEST_SERVER, gateway);
        pkt.set_meta(META_DEST_CHANNELS, &channels.join(","));
        self.0
            .push(gateway, pkt)
            .await
            .map_err(|e| RouterError::Dispatcher(e.to_string()))
    }
}

pub struct ChatHandler {
    container: Arc<Container>,
    router: Router,
    cache: Arc<dyn SessionStorage>,
    dispatcher: Arc<dyn Dispatcher>,
    #[allow(dead_code)]
    svc: ChatSvc,
}

impl ChatHandler {
    /// Two-arg signature unchanged for `e2e_login.rs`: `resolve_snowflake_node(None)`.
    pub fn new(container: Arc<Container>, cache: Arc<dyn SessionStorage>) -> Self {
        Self::new_with_node(container, cache, None)
    }

    /// `examples/fake-chat/src/main.rs` must pass `Some(cfg.this.snowflake_node)`.
    pub fn new_with_node(
        container: Arc<Container>,
        cache: Arc<dyn SessionStorage>,
        cfg_node: Option<u16>,
    ) -> Self {
        let node = resolve_snowflake_node(cfg_node);
        let idgen: Arc<dyn IdGenerator> = match SnowflakeGen::try_new(node) {
            Ok(g) => Arc::new(g),
            Err(err) => {
                tracing::error!(%err, node, "snowflake init failed; using SequenceIdGen");
                Arc::new(SequenceIdGen::new(10_001))
            }
        };
        Self::with_seams(
            container,
            cache,
            Arc::new(MemoryMessageStore::new(idgen.clone())),
            Arc::new(MemoryGroupDirectory::new(idgen)),
        )
    }

    pub fn with_seams(
        container: Arc<Container>,
        cache: Arc<dyn SessionStorage>,
        store: Arc<dyn MessageStore>,
        groups: Arc<dyn GroupDirectory>,
    ) -> Self {
        let dispatcher: Arc<dyn Dispatcher> = Arc::new(ContainerDispatcher(container.clone()));
        let mut router = Router::new();
        router.handle(CMD_LOGIN_SIGN_IN, do_sys_login);
        router.handle(CMD_LOGIN_SIGN_OUT, do_sys_logout);
        router.handle(CMD_DEMO_ECHO, do_echo);
        Self {
            container,
            router,
            cache,
            dispatcher,
            svc: ChatSvc { store, groups },
        }
    }
}

async fn resp_err(container: &Container, mut pkt: kim_protocol::LogicPkt, status: Status) {
    let gw = pkt.get_meta(META_DEST_SERVER).unwrap_or("").to_string();
    let ch = pkt.header.channel_id.clone();
    pkt.header.flag = Flag::Response as i32;
    pkt.header.status = status as i32;
    pkt.set_meta(META_DEST_SERVER, &gw);
    pkt.set_meta(META_DEST_CHANNELS, &ch);
    if let Err(err) = container.push(&gw, pkt).await {
        warn!(%err, "push err resp failed");
    }
}

#[async_trait]
impl Acceptor for ChatHandler {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| Error::HandshakeTimeout(timeout))??;
        let req = InnerHandshakeReq::decode(frame.payload.as_ref())
            .map_err(|e| Error::Handshake(e.to_string()))?;
        if req.service_id.is_empty() {
            return Err(Error::Handshake("empty service id".into()));
        }
        Ok(req.service_id)
    }
}

#[async_trait]
impl MessageListener for ChatHandler {
    async fn receive(&self, _agent: &dyn Agent, payload: Bytes) {
        let pkt = match read_logic(&payload) {
            Ok(p) => p,
            Err(err) => {
                warn!(%err, "unexpected basic pkt or bad logic");
                return;
            }
        };
        info!(
            command = %pkt.header.command,
            sequence = pkt.header.sequence,
            channel_id = %pkt.header.channel_id,
            "chat recv logic"
        );
        let session = if pkt.header.command == CMD_LOGIN_SIGN_IN {
            let gate = pkt.get_meta(META_DEST_SERVER).unwrap_or("").to_string();
            Session {
                channel_id: pkt.header.channel_id.clone(),
                gate_id: gate,
                tags: vec!["AutoGenerated".into()],
                ..Session::default()
            }
        } else {
            match self.cache.get(&pkt.header.channel_id).await {
                Ok(s) => s,
                Err(SessionError::NotFound) => {
                    resp_err(&self.container, pkt, Status::SessionNotFound).await;
                    return;
                }
                Err(_) => {
                    resp_err(&self.container, pkt, Status::SystemException).await;
                    return;
                }
            }
        };
        if let Err(err) = self
            .router
            .serve(pkt, self.dispatcher.clone(), self.cache.clone(), session)
            .await
        {
            warn!(%err, "router serve failed");
        }
    }
}

#[async_trait]
impl StateListener for ChatHandler {
    async fn disconnect(&self, channel_id: &str) -> Result<(), Error> {
        info!(channel = channel_id, "gateway disconnected");
        Ok(())
    }
}
