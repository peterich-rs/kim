//! Chat demo: session lookup, Router, login / logout / echo handlers.

mod ack;
pub mod admin;
pub mod directory;
mod echo;
pub mod filter;
mod friends;
mod group;
mod hmac_nonce;
pub mod idgen;
mod inbox;
mod login;
mod offline;
mod profile;
pub mod royal;
pub mod social;
pub mod store;
mod talk;
pub mod users;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_container::Container;
use kim_core::{Acceptor, Agent, Conn, Error, MessageListener, StateListener};
use kim_metrics::KimMetrics;
use kim_protocol::pkt::{Flag, InnerHandshakeReq, Session, Status};
use kim_protocol::{
    read_logic, ALLOWED_APP, CMD_BLOCK_ADD, CMD_BLOCK_LIST, CMD_BLOCK_REMOVE, CMD_CHAT_GROUP_TALK,
    CMD_CHAT_TALK_ACK, CMD_CHAT_USER_TALK, CMD_DEMO_ECHO, CMD_FRIEND_ACCEPT, CMD_FRIEND_INCOMING,
    CMD_FRIEND_LIST, CMD_FRIEND_REJECT, CMD_FRIEND_REMOVE, CMD_FRIEND_REQUEST, CMD_GROUP_CREATE,
    CMD_GROUP_DETAIL, CMD_GROUP_JOIN, CMD_GROUP_MEMBERS, CMD_GROUP_QUIT, CMD_HISTORY,
    CMD_INBOX_LIST, CMD_INBOX_READ, CMD_LOGIN_SIGN_IN, CMD_LOGIN_SIGN_OUT, CMD_OFFLINE_CONTENT,
    CMD_OFFLINE_INDEX, CMD_USER_PROFILE, CMD_USER_SEARCH, CMD_USER_UPDATE, META_DEST_CHANNELS,
    META_DEST_SERVER,
};
use kim_router::{Dispatcher, Router, RouterError, SessionError, SessionStorage};
use prost::Message;
use std::sync::Mutex;
use tracing::{info, warn};

use crate::directory::{GroupDirectory, MemoryGroupDirectory};
use crate::idgen::{resolve_snowflake_node, IdGenerator, SequenceIdGen, SnowflakeGen};
use crate::social::{MemorySocialDirectory, SocialDirectory};
use crate::store::{pending_receipt_enabled, MemoryMessageStore, MessageStore};
use crate::users::{MemoryUserDirectory, UserDirectory};

pub use ack::do_talk_ack;
pub use admin::{router as admin_router, serve as serve_admin, ChatAdmin};
pub use echo::do_echo;
pub use filter::{
    builtin_talk_filter, ContentFilter, FilterChain, ImageFilter, NoopFilter, TextWordFilter,
};
pub use friends::{
    do_block_add, do_block_list, do_block_remove, do_friend_accept, do_friend_incoming,
    do_friend_list, do_friend_reject, do_friend_remove, do_friend_request,
};
pub use group::{do_group_create, do_group_detail, do_group_join, do_group_members, do_group_quit};
#[cfg(feature = "redis")]
pub use hmac_nonce::RedisHmacNonceGuard;
pub use hmac_nonce::{HmacNonceGuard, MemoryHmacNonceGuard};
pub use inbox::{do_history, do_inbox_list, do_inbox_read, parse_kind};
pub use kim_session::open_uncached_session_store;
pub use login::{do_sys_login, do_sys_login_with_zone, do_sys_logout};
pub use offline::{do_offline_content, do_offline_index};
pub use profile::{do_user_profile, do_user_search, do_user_update};
pub use royal::{http_backends, http_backends_with_hmac, http_backends_with_hmac_receipt};
pub use talk::{do_group_talk, do_user_talk};

#[derive(Clone)]
pub(crate) struct ChatSvc {
    store: Arc<dyn MessageStore>,
    groups: Arc<dyn GroupDirectory>,
    filter: Arc<dyn ContentFilter>,
    users: Arc<dyn UserDirectory>,
    social: Arc<dyn SocialDirectory>,
    metrics: Arc<Mutex<Option<Arc<KimMetrics>>>>,
    pending_receipt: bool,
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
    svc: ChatSvc,
}

impl ChatHandler {
    /// Two-arg signature unchanged for `e2e_login.rs`: `resolve_snowflake_node(None)`.
    pub fn new(container: Arc<Container>, cache: Arc<dyn SessionStorage>) -> Self {
        Self::new_with_node(container, cache, None)
    }

    /// `services/chat/src/main.rs` must pass `Some(cfg.this.snowflake_node)`.
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
        Self::with_seams_and_zone(
            container,
            cache,
            Arc::new(MemoryMessageStore::new(idgen.clone())),
            Arc::new(MemoryGroupDirectory::new(idgen)),
            String::new(),
        )
    }

    pub fn with_seams(
        container: Arc<Container>,
        cache: Arc<dyn SessionStorage>,
        store: Arc<dyn MessageStore>,
        groups: Arc<dyn GroupDirectory>,
    ) -> Self {
        Self::with_seams_and_zone(container, cache, store, groups, String::new())
    }

    pub fn with_seams_pending(
        container: Arc<Container>,
        cache: Arc<dyn SessionStorage>,
        store: Arc<dyn MessageStore>,
        groups: Arc<dyn GroupDirectory>,
        pending_receipt: bool,
    ) -> Self {
        Self::with_social(
            container,
            cache,
            store,
            groups,
            String::new(),
            Arc::new(NoopFilter),
            Arc::new(MemoryUserDirectory::new()),
            Arc::new(MemorySocialDirectory::new()),
            pending_receipt,
        )
    }

    pub fn with_seams_and_zone(
        container: Arc<Container>,
        cache: Arc<dyn SessionStorage>,
        store: Arc<dyn MessageStore>,
        groups: Arc<dyn GroupDirectory>,
        zone: String,
    ) -> Self {
        Self::with_filter(container, cache, store, groups, zone, Arc::new(NoopFilter))
    }

    pub fn with_filter(
        container: Arc<Container>,
        cache: Arc<dyn SessionStorage>,
        store: Arc<dyn MessageStore>,
        groups: Arc<dyn GroupDirectory>,
        zone: String,
        filter: Arc<dyn ContentFilter>,
    ) -> Self {
        Self::with_users(
            container,
            cache,
            store,
            groups,
            zone,
            filter,
            Arc::new(MemoryUserDirectory::new()),
        )
    }

    pub fn with_users(
        container: Arc<Container>,
        cache: Arc<dyn SessionStorage>,
        store: Arc<dyn MessageStore>,
        groups: Arc<dyn GroupDirectory>,
        zone: String,
        filter: Arc<dyn ContentFilter>,
        users: Arc<dyn UserDirectory>,
    ) -> Self {
        Self::with_social(
            container,
            cache,
            store,
            groups,
            zone,
            filter,
            users,
            Arc::new(MemorySocialDirectory::new()),
            pending_receipt_enabled(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_social(
        container: Arc<Container>,
        cache: Arc<dyn SessionStorage>,
        store: Arc<dyn MessageStore>,
        groups: Arc<dyn GroupDirectory>,
        zone: String,
        filter: Arc<dyn ContentFilter>,
        users: Arc<dyn UserDirectory>,
        social: Arc<dyn SocialDirectory>,
        pending_receipt: bool,
    ) -> Self {
        let dispatcher: Arc<dyn Dispatcher> = Arc::new(ContainerDispatcher(container.clone()));
        let mut router = Router::new();
        {
            let zone = zone.clone();
            let users = users.clone();
            let store = store.clone();
            router.handle(CMD_LOGIN_SIGN_IN, move |ctx| {
                let zone = zone.clone();
                let users = users.clone();
                let store = store.clone();
                async move {
                    do_sys_login_with_zone(
                        ctx,
                        &zone,
                        users.as_ref(),
                        Some(store.as_ref()),
                        pending_receipt,
                    )
                    .await
                }
            });
        }
        router.handle(CMD_LOGIN_SIGN_OUT, do_sys_logout);
        router.handle(CMD_DEMO_ECHO, do_echo);
        let svc = ChatSvc {
            store,
            groups,
            filter,
            users,
            social,
            metrics: Arc::new(Mutex::new(None)),
            pending_receipt,
        };
        {
            let svc = svc.clone();
            router.handle(CMD_CHAT_USER_TALK, move |ctx| {
                let svc = svc.clone();
                async move {
                    let metrics = svc
                        .metrics
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    do_user_talk(
                        ctx,
                        svc.store.as_ref(),
                        svc.filter.as_ref(),
                        svc.users.as_ref(),
                        svc.social.as_ref(),
                        metrics.as_deref(),
                        talk::TALK_PUSH_BUDGET,
                    )
                    .await
                }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_CHAT_GROUP_TALK, move |ctx| {
                let svc = svc.clone();
                async move {
                    let metrics = svc
                        .metrics
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    do_group_talk(
                        ctx,
                        svc.store.as_ref(),
                        svc.groups.as_ref(),
                        svc.filter.as_ref(),
                        metrics.as_deref(),
                        talk::TALK_PUSH_BUDGET,
                    )
                    .await
                }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_GROUP_CREATE, move |ctx| {
                let svc = svc.clone();
                async move { do_group_create(ctx, svc.groups.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_GROUP_JOIN, move |ctx| {
                let svc = svc.clone();
                async move { do_group_join(ctx, svc.groups.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_GROUP_QUIT, move |ctx| {
                let svc = svc.clone();
                async move { do_group_quit(ctx, svc.groups.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_GROUP_DETAIL, move |ctx| {
                let svc = svc.clone();
                async move { do_group_detail(ctx, svc.groups.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_GROUP_MEMBERS, move |ctx| {
                let svc = svc.clone();
                async move { do_group_members(ctx, svc.groups.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_CHAT_TALK_ACK, move |ctx| {
                let svc = svc.clone();
                async move { do_talk_ack(ctx, svc.store.as_ref(), svc.pending_receipt).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_OFFLINE_INDEX, move |ctx| {
                let svc = svc.clone();
                async move { do_offline_index(ctx, svc.store.as_ref(), svc.pending_receipt).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_OFFLINE_CONTENT, move |ctx| {
                let svc = svc.clone();
                async move { do_offline_content(ctx, svc.store.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_USER_PROFILE, move |ctx| {
                let svc = svc.clone();
                async move { do_user_profile(ctx, svc.users.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_USER_UPDATE, move |ctx| {
                let svc = svc.clone();
                async move { do_user_update(ctx, svc.users.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_USER_SEARCH, move |ctx| {
                let svc = svc.clone();
                async move { do_user_search(ctx, svc.users.as_ref(), svc.social.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_FRIEND_REQUEST, move |ctx| {
                let svc = svc.clone();
                async move { do_friend_request(ctx, svc.social.as_ref(), svc.users.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_FRIEND_ACCEPT, move |ctx| {
                let svc = svc.clone();
                async move { do_friend_accept(ctx, svc.social.as_ref(), svc.users.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_FRIEND_REJECT, move |ctx| {
                let svc = svc.clone();
                async move { do_friend_reject(ctx, svc.social.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_FRIEND_REMOVE, move |ctx| {
                let svc = svc.clone();
                async move { do_friend_remove(ctx, svc.social.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_FRIEND_LIST, move |ctx| {
                let svc = svc.clone();
                async move { do_friend_list(ctx, svc.social.as_ref(), svc.users.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_FRIEND_INCOMING, move |ctx| {
                let svc = svc.clone();
                async move { do_friend_incoming(ctx, svc.social.as_ref(), svc.users.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_BLOCK_ADD, move |ctx| {
                let svc = svc.clone();
                async move { do_block_add(ctx, svc.social.as_ref(), svc.users.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_BLOCK_REMOVE, move |ctx| {
                let svc = svc.clone();
                async move { do_block_remove(ctx, svc.social.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_BLOCK_LIST, move |ctx| {
                let svc = svc.clone();
                async move { do_block_list(ctx, svc.social.as_ref(), svc.users.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_INBOX_LIST, move |ctx| {
                let svc = svc.clone();
                async move {
                    do_inbox_list(
                        ctx,
                        svc.store.as_ref(),
                        svc.users.as_ref(),
                        svc.groups.as_ref(),
                    )
                    .await
                }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_INBOX_READ, move |ctx| {
                let svc = svc.clone();
                async move { do_inbox_read(ctx, svc.store.as_ref()).await }
            });
        }
        {
            let svc = svc.clone();
            router.handle(CMD_HISTORY, move |ctx| {
                let svc = svc.clone();
                async move { do_history(ctx, svc.store.as_ref()).await }
            });
        }
        Self {
            container,
            router,
            cache,
            dispatcher,
            svc,
        }
    }

    pub fn with_metrics(&self, m: Arc<KimMetrics>) {
        *self.svc.metrics.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
    }

    pub fn admin(
        &self,
        hmac_secret: impl Into<String>,
        nonce: Arc<dyn HmacNonceGuard>,
    ) -> ChatAdmin {
        ChatAdmin::new(
            self.cache.clone(),
            self.dispatcher.clone(),
            hmac_secret,
            nonce,
        )
    }

    fn metrics(&self) -> Option<Arc<KimMetrics>> {
        self.svc
            .metrics
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
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
                    if let Some(m) = self.metrics() {
                        m.on_session_not_found();
                    }
                    resp_err(&self.container, pkt, Status::SessionNotFound).await;
                    return;
                }
                Err(_) => {
                    resp_err(&self.container, pkt, Status::SystemException).await;
                    return;
                }
            }
        };
        if pkt.header.command != CMD_LOGIN_SIGN_IN && session.app != ALLOWED_APP {
            resp_err(&self.container, pkt, Status::Unauthorized).await;
            return;
        }
        if let Some(m) = self.metrics() {
            m.on_message_in(payload.len() as u64);
            if pkt.header.command == CMD_CHAT_USER_TALK {
                m.on_talk("user");
            } else if pkt.header.command == CMD_CHAT_GROUP_TALK {
                m.on_talk("group");
            }
        }
        let started = std::time::Instant::now();
        let cmd = pkt.header.command.clone();
        if let Err(err) = self
            .router
            .serve(pkt, self.dispatcher.clone(), self.cache.clone(), session)
            .await
        {
            warn!(%err, "router serve failed");
        }
        if let Some(m) = self.metrics() {
            m.observe_handler(&cmd, started.elapsed());
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer};
    use kim_naming::{DefaultRegistration, StaticNaming};
    use kim_protocol::pkt::MessageReq;
    use kim_protocol::{marshal, LogicPkt, Packet, MESSAGE_TYPE_TEXT};
    use kim_session::MemorySessionStore;

    use super::*;
    use crate::directory::MemoryGroupDirectory;
    use crate::idgen::SequenceIdGen;
    use crate::social::{MemorySocialDirectory, SocialDirectory};
    use crate::store::MemoryMessageStore;
    use crate::users::{MemoryUserDirectory, UserDirectory};

    struct NoopAgent;

    #[async_trait]
    impl Agent for NoopAgent {
        fn id(&self) -> &str {
            "noop"
        }

        async fn push(&self, _payload: Bytes) -> Result<(), Error> {
            Ok(())
        }
    }

    fn ident() -> DefaultRegistration {
        DefaultRegistration {
            service_id: "chat-1".into(),
            service_name: "chat".into(),
            protocol: "tcp".into(),
            public_address: String::new(),
            public_port: 0,
            tags: vec![],
            meta: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn receive_rejects_non_kim_session_before_store() {
        let cache: Arc<dyn SessionStorage> = Arc::new(MemorySessionStore::new());
        let store = Arc::new(MemoryMessageStore::new(Arc::new(SequenceIdGen::new(1))));
        let groups = Arc::new(MemoryGroupDirectory::new(Arc::new(SequenceIdGen::new(2))));
        let users = Arc::new(MemoryUserDirectory::new());
        let social = Arc::new(MemorySocialDirectory::new());
        users.upsert("kim-gray", "alice").await.unwrap();
        users.upsert("kim-gray", "bob").await.unwrap();
        social.request("kim-gray", "alice", "bob").await.unwrap();
        social.accept("kim-gray", "bob", "alice").await.unwrap();

        let container = Container::new(ContainerOpts {
            naming: Arc::new(StaticNaming::from_slice(vec![])),
            identity: ident(),
            dialer: Arc::new(InnerTcpDialer {
                local_service_id: "chat-1".into(),
            }),
            deps: vec![],
            adult_delay: Duration::from_millis(0),
            selector: Arc::new(HashSelector),
            after_downlink: vec![],
        });
        let handler = ChatHandler::with_social(
            container,
            cache.clone(),
            store.clone(),
            groups,
            String::new(),
            Arc::new(NoopFilter),
            users,
            social,
            false,
        );

        cache
            .add(&Session {
                channel_id: "ch-gray".into(),
                gate_id: "wg-1".into(),
                account: "alice".into(),
                app: "kim-gray".into(),
                ..Session::default()
            })
            .await
            .unwrap();

        let mut pkt = LogicPkt::new(CMD_CHAT_USER_TALK, 1, Bytes::new());
        pkt.header.channel_id = "ch-gray".into();
        pkt.set_dest("bob");
        pkt.write_body(&MessageReq {
            r#type: MESSAGE_TYPE_TEXT,
            body: "hi".into(),
            extra: String::new(),
            client_id: String::new(),
        });
        handler
            .receive(&NoopAgent, marshal(&Packet::Logic(pkt)))
            .await;
        assert!(
            store.recorded().is_empty(),
            "kim-gray session must not reach the message store"
        );
    }
}
