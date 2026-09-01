mod harness;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use chat::store::MessageStore;
use chat::ChatHandler;
use gateway::{GatewayHandler, KickHook, Route, RouteFile, RouteSelector, ZoneFile};
use harness::*;
use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer, ADULT};
use kim_core::{Conn, MessageListener, OpCode, Server};
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_protocol::pkt::{Flag, MessageReq, Session, Status};
use kim_protocol::{
    generate, marshal, read, LogicPkt, Packet, CMD_CHAT_USER_TALK, CMD_FRIEND_ACCEPT,
    CMD_FRIEND_REQUEST, DEMO_DEFAULT_SECRET, MESSAGE_TYPE_TEXT,
};
use kim_router::SessionStorage;
use kim_session::open_session_store;
use kim_tcp::TcpServer;
use kim_ws::{connect_ws, WsServer};
use pkt_client::perform_login;

fn ident(id: &str, name: &str) -> DefaultRegistration {
    DefaultRegistration {
        service_id: id.into(),
        service_name: name.into(),
        protocol: "tcp".into(),
        public_address: String::new(),
        public_port: 0,
        tags: vec![],
        meta: HashMap::new(),
    }
}

fn zone_reg(id: &str, zone: &str, port: u16) -> DefaultRegistration {
    DefaultRegistration {
        service_id: id.into(),
        service_name: "chat".into(),
        protocol: "tcp".into(),
        public_address: "127.0.0.1".into(),
        public_port: port,
        tags: vec![format!("zone:{zone}")],
        meta: {
            let mut m = HashMap::new();
            m.insert("zone".into(), zone.into());
            m
        },
    }
}

struct GrayStack {
    gw: Arc<Container>,
    c1: Arc<Container>,
    c2: Option<Arc<Container>>,
    gw_addr: std::net::SocketAddr,
    cache: Arc<dyn SessionStorage>,
}

impl GrayStack {
    async fn shutdown(self) {
        let _ = self.gw.shutdown().await;
        let _ = self.c1.shutdown().await;
        if let Some(c2) = self.c2 {
            let _ = c2.shutdown().await;
        }
    }
}

fn account_route(whitelist: HashMap<String, String>) -> Route {
    Route::from_config(RouteFile {
        route_by: "account".into(),
        zones: vec![
            ZoneFile {
                id: "zone_local".into(),
                weight: 100,
            },
            ZoneFile {
                id: "zone_gray".into(),
                weight: 0,
            },
        ],
        whitelist,
    })
}

async fn spawn_chat(
    id: &str,
    zone: &str,
    cache: Arc<dyn SessionStorage>,
    node: i64,
) -> (Arc<Container>, std::net::SocketAddr) {
    let mut chat = TcpServer::bind("127.0.0.1:0").await.expect("chat bind");
    let addr = chat.local_addr();
    let c = Container::new(ContainerOpts {
        naming: Arc::new(StaticNaming::from_slice(vec![])),
        identity: ident(id, "chat"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: id.into(),
        }),
        deps: vec![],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
        after_downlink: vec![],
    });
    let h = Arc::new(ChatHandler::with_seams_and_zone(
        c.clone(),
        cache,
        Arc::new(chat::store::MemoryMessageStore::new(Arc::new(
            chat::idgen::SequenceIdGen::new(node),
        ))),
        Arc::new(chat::directory::MemoryGroupDirectory::new(Arc::new(
            chat::idgen::SequenceIdGen::new(node + 10_000),
        ))),
        zone.into(),
    ));
    chat.set_acceptor(h.clone());
    chat.set_message_listener(h.clone());
    chat.set_state_listener(h);
    c.attach_server(Arc::new(chat));
    let run = c.clone();
    tokio::spawn(async move {
        let _ = run.start().await;
    });
    (c, addr)
}

async fn spawn_gray(whitelist: HashMap<String, String>, with_gray_chat: bool) -> GrayStack {
    let cache = open_session_store(None).await.expect("memory");
    let (c1, addr1) = spawn_chat("chat-1", "zone_local", cache.clone(), 10_001).await;
    let (c2, addr2) = if with_gray_chat {
        let pair = spawn_chat("chat-2", "zone_gray", cache.clone(), 11_001).await;
        (Some(pair.0), Some(pair.1))
    } else {
        (None, None)
    };

    let mut regs = vec![zone_reg("chat-1", "zone_local", addr1.port())];
    if let Some(a2) = addr2 {
        regs.push(zone_reg("chat-2", "zone_gray", a2.port()));
    }

    let mut gw_server = WsServer::bind("127.0.0.1:0").await.expect("gw");
    let gw_addr = gw_server.local_addr();
    let hook = Arc::new(KickHook::new());
    let naming = Arc::new(StaticNaming::from_slice(regs));
    let gw = Container::new(ContainerOpts {
        naming,
        identity: ident("wg-1", "wgateway"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: "wg-1".into(),
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(RouteSelector::new(account_route(whitelist))),
        after_downlink: vec![hook.clone()],
    });
    let gw_h = Arc::new(GatewayHandler::new(gw.clone(), "wg-1", DEMO_DEFAULT_SECRET));
    gw_h.set_revoke(Arc::new(gateway::AllowAllRevoke));
    gw_server.set_acceptor(gw_h.clone());
    gw_server.set_message_listener(gw_h.clone());
    gw_server.set_state_listener(gw_h.clone());
    let gw_server = Arc::new(gw_server);
    hook.attach(gw_server.clone());
    gw_h.attach_server(gw_server.clone());
    gw.attach_server(gw_server);
    let run = gw.clone();
    tokio::spawn(async move {
        let _ = run.start().await;
    });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let c1_ready = gw.slot_state("chat", "chat-1").await == Some(ADULT);
        let c2_ready = !with_gray_chat || gw.slot_state("chat", "chat-2").await == Some(ADULT);
        if c1_ready && c2_ready {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("chats not adult");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    GrayStack {
        gw,
        c1,
        c2,
        gw_addr,
        cache,
    }
}

#[tokio::test]
async fn whitelist_account_hits_zone_gray() {
    let mut whitelist = HashMap::new();
    whitelist.insert("alice".into(), "zone_gray".into());
    whitelist.insert("bob".into(), "zone_gray".into());
    let stack = spawn_gray(whitelist, true).await;
    let url = ws_url(stack.gw_addr);

    let bob_token = generate(DEMO_DEFAULT_SECRET, "bob", "kim", i64::MAX / 4).expect("jwt");
    let mut bob_conn = connect_ws(&url).await.expect("bob ws");
    let _ = perform_login(&mut bob_conn, bob_token)
        .await
        .expect("bob login");

    let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim", i64::MAX / 4).expect("jwt");
    let mut conn = connect_ws(&url).await.expect("ws");
    let ch = perform_login(&mut conn, token).await.expect("login");
    assert!(ch.contains("alice"));
    let sess = stack.cache.get(&ch).await.expect("session");
    assert_eq!(sess.zone, "zone_gray");
    assert_eq!(sess.app, "kim");

    let mut freq = LogicPkt::new(CMD_FRIEND_REQUEST, 2, Bytes::new());
    freq.set_dest("bob");
    conn.write_frame(OpCode::Binary, marshal(&Packet::Logic(freq)))
        .await
        .expect("friend req");
    let req_resp = timeout_read_response(&mut conn).await;
    assert_eq!(req_resp.header.status, Status::Success as i32);
    let mut facc = LogicPkt::new(CMD_FRIEND_ACCEPT, 2, Bytes::new());
    facc.set_dest("alice");
    bob_conn
        .write_frame(OpCode::Binary, marshal(&Packet::Logic(facc)))
        .await
        .expect("friend accept");
    let acc_resp = timeout_read_response(&mut bob_conn).await;
    assert_eq!(acc_resp.header.status, Status::Success as i32);
    bob_conn.shutdown().await.expect("bob close");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut pkt = LogicPkt::new(CMD_CHAT_USER_TALK, 3, Bytes::new());
    pkt.set_dest("bob");
    pkt.write_body(&MessageReq {
        r#type: MESSAGE_TYPE_TEXT,
        body: "hi".into(),
        extra: String::new(),
        client_id: String::new(),
    });
    conn.write_frame(OpCode::Binary, marshal(&Packet::Logic(pkt)))
        .await
        .expect("talk");
    let p = timeout_read_response(&mut conn).await;
    assert_eq!(p.header.status, Status::Success as i32);
    assert_eq!(p.header.flag, Flag::Response as i32);

    stack.shutdown().await;
}

#[tokio::test]
async fn kim_gray_jwt_login_unauthorized() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim-gray", i64::MAX / 4).expect("jwt");
    let mut conn = connect_ws(&url).await.expect("ws");
    let err = perform_login(&mut conn, token)
        .await
        .expect_err("kim-gray must fail");
    assert!(
        err.to_string().contains("status=105"),
        "expected Unauthorized 105, got {err}"
    );
    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn second_app_cannot_login_same_account() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let kim = generate(DEMO_DEFAULT_SECRET, "alice", "kim", i64::MAX / 4).expect("jwt");
    let mut conn = connect_ws(&url).await.expect("ws");
    let ch = perform_login(&mut conn, kim).await.expect("kim login");
    assert!(ch.contains("alice"));

    let gray = generate(DEMO_DEFAULT_SECRET, "alice", "kim-gray", i64::MAX / 4).expect("jwt");
    let mut gray_conn = connect_ws(&url).await.expect("ws");
    let err = perform_login(&mut gray_conn, gray)
        .await
        .expect_err("second app must fail");
    assert!(
        err.to_string().contains("status=105"),
        "expected Unauthorized 105, got {err}"
    );

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn whitelist_empty_gray_zone_does_not_fallback() {
    let mut whitelist = HashMap::new();
    whitelist.insert("alice".into(), "zone_gray".into());
    let stack = spawn_gray(whitelist, false).await;
    let url = ws_url(stack.gw_addr);
    let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim", i64::MAX / 4).expect("jwt");
    let mut conn = connect_ws(&url).await.expect("ws");
    let err = perform_login(&mut conn, token)
        .await
        .expect_err("empty gray zone must not land on zone_local");
    let msg = err.to_string();
    assert!(
        msg.contains("status=3") || msg.contains("closed") || msg.contains("status=105"),
        "expected unavailable/closed, got {msg}"
    );

    let bob = generate(DEMO_DEFAULT_SECRET, "bob", "kim", i64::MAX / 4).expect("jwt");
    let mut bob_conn = connect_ws(&url).await.expect("ws");
    let ch = perform_login(&mut bob_conn, bob)
        .await
        .expect("non-whitelist still routes");
    let sess = stack.cache.get(&ch).await.expect("session");
    assert_eq!(sess.zone, "zone_local");

    stack.shutdown().await;
}

#[tokio::test]
async fn non_whitelist_kim_account_hits_zone_local() {
    let mut whitelist = HashMap::new();
    whitelist.insert("alice".into(), "zone_gray".into());
    let stack = spawn_gray(whitelist, true).await;
    let url = ws_url(stack.gw_addr);
    let token = generate(DEMO_DEFAULT_SECRET, "carol", "kim", i64::MAX / 4).expect("jwt");
    let mut conn = connect_ws(&url).await.expect("ws");
    let ch = perform_login(&mut conn, token).await.expect("login");
    let sess = stack.cache.get(&ch).await.expect("session");
    assert_eq!(sess.zone, "zone_local");
    assert_eq!(sess.app, "kim");
    stack.shutdown().await;
}

struct NoopAgent;

#[async_trait::async_trait]
impl kim_core::Agent for NoopAgent {
    fn id(&self) -> &str {
        "noop"
    }

    async fn push(&self, _payload: Bytes) -> Result<(), kim_core::Error> {
        Ok(())
    }
}

#[tokio::test]
async fn legacy_kim_gray_session_talk_is_unauthorized() {
    let cache = open_session_store(None).await.expect("memory");
    let store = Arc::new(chat::store::MemoryMessageStore::new(Arc::new(
        chat::idgen::SequenceIdGen::new(1),
    )));
    let groups = Arc::new(chat::directory::MemoryGroupDirectory::new(Arc::new(
        chat::idgen::SequenceIdGen::new(2),
    )));
    let container = Container::new(ContainerOpts {
        naming: Arc::new(StaticNaming::from_slice(vec![])),
        identity: ident("chat-1", "chat"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: "chat-1".into(),
        }),
        deps: vec![],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
        after_downlink: vec![],
    });
    let handler = ChatHandler::with_seams(container, cache.clone(), store.clone(), groups);
    cache
        .add(&Session {
            channel_id: "ch-gray".into(),
            gate_id: "wg-1".into(),
            account: "alice".into(),
            app: "kim-gray".into(),
            ..Session::default()
        })
        .await
        .expect("add gray session");

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

    let (idx, _) = store
        .offline_index("kim-gray", "bob", "", 0, false)
        .await
        .expect("index");
    assert!(idx.is_empty(), "kim-gray talk must not persist");
    let (idx_kim, _) = store
        .offline_index("kim", "bob", "", 0, false)
        .await
        .expect("index");
    assert!(idx_kim.is_empty(), "kim-gray talk must not persist as kim");
}

async fn timeout_read_conn<C: Conn>(conn: &mut C) -> kim_core::Frame {
    tokio::time::timeout(Duration::from_secs(3), conn.read_frame())
        .await
        .expect("timeout")
        .expect("read")
}

async fn timeout_read_response<C: Conn>(conn: &mut C) -> LogicPkt {
    loop {
        let frame = timeout_read_conn(conn).await;
        match read(&frame.payload).expect("decode") {
            Packet::Logic(p) if p.header.flag == Flag::Response as i32 => return p,
            Packet::Logic(_) => continue,
            _ => panic!("expected logic"),
        }
    }
}
