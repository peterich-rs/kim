mod harness;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use chat::ChatHandler;
use gateway::{GatewayHandler, KickHook, Route, RouteFile, RouteSelector, ZoneFile};
use harness::*;
use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer, ADULT};
use kim_core::{Conn, OpCode, Server};
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_protocol::pkt::{Flag, MessageReq, Status};
use kim_protocol::{
    generate, marshal, read, LogicPkt, Packet, CMD_CHAT_USER_TALK, CMD_FRIEND_ACCEPT,
    CMD_FRIEND_REQUEST, DEMO_DEFAULT_SECRET, MESSAGE_TYPE_TEXT,
};
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

#[tokio::test]
async fn whitelist_app_hits_chat2_then_fallback() {
    let cache = open_session_store(None).await.expect("memory");

    let mut chat1 = TcpServer::bind("127.0.0.1:0").await.expect("c1");
    let addr1 = chat1.local_addr();
    let c1 = Container::new(ContainerOpts {
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
    let h1 = Arc::new(ChatHandler::with_seams_and_zone(
        c1.clone(),
        cache.clone(),
        Arc::new(chat::store::MemoryMessageStore::new(Arc::new(
            chat::idgen::SequenceIdGen::new(10_001),
        ))),
        Arc::new(chat::directory::MemoryGroupDirectory::new(Arc::new(
            chat::idgen::SequenceIdGen::new(20_001),
        ))),
        "zone_local".into(),
    ));
    chat1.set_acceptor(h1.clone());
    chat1.set_message_listener(h1.clone());
    chat1.set_state_listener(h1);
    c1.attach_server(Arc::new(chat1));
    let run1 = c1.clone();
    tokio::spawn(async move {
        let _ = run1.start().await;
    });

    let mut chat2 = TcpServer::bind("127.0.0.1:0").await.expect("c2");
    let addr2 = chat2.local_addr();
    let c2 = Container::new(ContainerOpts {
        naming: Arc::new(StaticNaming::from_slice(vec![])),
        identity: ident("chat-2", "chat"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: "chat-2".into(),
        }),
        deps: vec![],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
        after_downlink: vec![],
    });
    let h2 = Arc::new(ChatHandler::with_seams_and_zone(
        c2.clone(),
        cache.clone(),
        Arc::new(chat::store::MemoryMessageStore::new(Arc::new(
            chat::idgen::SequenceIdGen::new(11_001),
        ))),
        Arc::new(chat::directory::MemoryGroupDirectory::new(Arc::new(
            chat::idgen::SequenceIdGen::new(21_001),
        ))),
        "zone_gray".into(),
    ));
    chat2.set_acceptor(h2.clone());
    chat2.set_message_listener(h2.clone());
    chat2.set_state_listener(h2);
    c2.attach_server(Arc::new(chat2));
    let run2 = c2.clone();
    tokio::spawn(async move {
        let _ = run2.start().await;
    });

    let mut whitelist = HashMap::new();
    whitelist.insert("kim-gray".into(), "zone_gray".into());
    let route = Route::from_config(RouteFile {
        route_by: "app".into(),
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
    });
    let sel = Arc::new(RouteSelector::new(route));

    let mut gw_server = WsServer::bind("127.0.0.1:0").await.expect("gw");
    let gw_addr = gw_server.local_addr();
    let hook = Arc::new(KickHook::new());
    let naming = Arc::new(StaticNaming::from_slice(vec![
        DefaultRegistration {
            service_id: "chat-1".into(),
            service_name: "chat".into(),
            protocol: "tcp".into(),
            public_address: "127.0.0.1".into(),
            public_port: addr1.port(),
            tags: vec!["zone:zone_local".into()],
            meta: {
                let mut m = HashMap::new();
                m.insert("zone".into(), "zone_local".into());
                m
            },
        },
        DefaultRegistration {
            service_id: "chat-2".into(),
            service_name: "chat".into(),
            protocol: "tcp".into(),
            public_address: "127.0.0.1".into(),
            public_port: addr2.port(),
            tags: vec!["zone:zone_gray".into()],
            meta: {
                let mut m = HashMap::new();
                m.insert("zone".into(), "zone_gray".into());
                m
            },
        },
    ]));
    let gw = Container::new(ContainerOpts {
        naming,
        identity: ident("wg-1", "wgateway"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: "wg-1".into(),
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(0),
        selector: sel,
        after_downlink: vec![hook.clone()],
    });
    let gw_h = Arc::new(GatewayHandler::new(gw.clone(), "wg-1", DEMO_DEFAULT_SECRET));
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
        if gw.slot_state("chat", "chat-1").await == Some(ADULT)
            && gw.slot_state("chat", "chat-2").await == Some(ADULT)
        {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("chats not adult");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let url = ws_url(gw_addr);
    let bob_token = generate(DEMO_DEFAULT_SECRET, "bob", "kim-gray", i64::MAX / 4).expect("jwt");
    let mut bob_conn = connect_ws(&url).await.expect("bob ws");
    let _ = perform_login(&mut bob_conn, bob_token)
        .await
        .expect("bob login");

    let token = generate(DEMO_DEFAULT_SECRET, "alice", "kim-gray", i64::MAX / 4).expect("jwt");
    let mut conn = connect_ws(&url).await.expect("ws");
    let ch = perform_login(&mut conn, token).await.expect("login");
    assert!(ch.contains("alice"));
    let sess = cache.get(&ch).await.expect("session");
    assert_eq!(sess.zone, "zone_gray");
    assert_eq!(sess.app, "kim-gray");

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

    let _ = gw.shutdown().await;
    let _ = c1.shutdown().await;
    let _ = c2.shutdown().await;
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
