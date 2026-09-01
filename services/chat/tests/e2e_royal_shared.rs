//! Two Chat processes share one in-process Royal directory.

mod harness;

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use chat::idgen::SequenceIdGen;
use chat::royal::http_backends;
use chat::ChatHandler;
use gateway::{GatewayHandler, KickHook};
use harness::*;
use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer, ADULT};
use kim_core::Server;
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_protocol::pkt::{GroupCreateReq, GroupCreateResp, GroupDetail, Status};
use kim_protocol::{marshal, read, LogicPkt, Packet, CMD_GROUP_CREATE, CMD_GROUP_DETAIL};
use kim_session::open_session_store;
use kim_tcp::TcpServer;
use kim_ws::WsServer;
use royal::{serve, RoyalState};

fn ident(id: &str, name: &str) -> DefaultRegistration {
    DefaultRegistration {
        service_id: id.into(),
        service_name: name.into(),
        protocol: "tcp".into(),
        public_address: String::new(),
        public_port: 0,
        tags: vec![],
        meta: std::collections::HashMap::new(),
    }
}

async fn spawn_chat_gw(
    cache: Arc<dyn kim_router::SessionStorage>,
    store: Arc<dyn chat::store::MessageStore>,
    groups: Arc<dyn chat::directory::GroupDirectory>,
    chat_id: &str,
    gw_id: &str,
) -> (Arc<Container>, Arc<Container>, std::net::SocketAddr) {
    let mut chat_server = TcpServer::bind("127.0.0.1:0").await.expect("chat bind");
    chat_server.set_drain_wait(Duration::from_millis(50));
    let chat_addr = chat_server.local_addr();
    let chat_c = Container::new(ContainerOpts {
        naming: Arc::new(StaticNaming::from_slice(vec![])),
        identity: ident(chat_id, "chat"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: chat_id.into(),
        }),
        deps: vec![],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
        after_downlink: vec![],
    });
    let h = Arc::new(ChatHandler::with_seams(
        chat_c.clone(),
        cache,
        store,
        groups,
    ));
    chat_server.set_acceptor(h.clone());
    chat_server.set_message_listener(h.clone());
    chat_server.set_state_listener(h);
    chat_c.attach_server(Arc::new(chat_server));
    let run = chat_c.clone();
    tokio::spawn(async move {
        let _ = run.start().await;
    });

    let mut gw_server = WsServer::bind("127.0.0.1:0").await.expect("gw bind");
    gw_server.set_drain_wait(Duration::from_millis(50));
    let gw_addr = gw_server.local_addr();
    let hook = Arc::new(KickHook::new());
    let naming = Arc::new(StaticNaming::from_slice(vec![DefaultRegistration {
        service_id: chat_id.into(),
        service_name: "chat".into(),
        protocol: "tcp".into(),
        public_address: "127.0.0.1".into(),
        public_port: chat_addr.port(),
        tags: vec![],
        meta: std::collections::HashMap::new(),
    }]));
    let gw_c = Container::new(ContainerOpts {
        naming,
        identity: ident(gw_id, "wgateway"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: gw_id.into(),
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
        after_downlink: vec![hook.clone()],
    });
    let gw_h = Arc::new(GatewayHandler::new(
        gw_c.clone(),
        gw_id,
        kim_protocol::DEMO_DEFAULT_SECRET,
    ));
    gw_h.set_revoke(Arc::new(gateway::AllowAllRevoke));
    gw_server.set_acceptor(gw_h.clone());
    gw_server.set_message_listener(gw_h.clone());
    gw_server.set_state_listener(gw_h.clone());
    let gw_server = Arc::new(gw_server);
    hook.attach(gw_server.clone());
    gw_h.attach_server(gw_server.clone());
    gw_c.attach_server(gw_server);
    let run_gw = gw_c.clone();
    tokio::spawn(async move {
        let _ = run_gw.start().await;
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if gw_c.slot_state("chat", chat_id).await == Some(ADULT) {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("{chat_id} slot did not become Adult");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    (chat_c, gw_c, gw_addr)
}

#[tokio::test]
async fn two_chats_see_the_same_group_via_royal() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("royal bind");
    let addr = listener.local_addr().expect("royal addr");
    let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
    tokio::spawn(async move {
        let _ = serve(listener, state).await;
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    let base = format!("http://{addr}");
    let (store1, groups1, _, _) = http_backends(&base).expect("b1");
    let (store2, groups2, _, _) = http_backends(&base).expect("b2");
    let cache = open_session_store(None).await.expect("cache");

    let (c1, g1, a1) = spawn_chat_gw(cache.clone(), store1, groups1, "chat-1", "wg-1").await;
    let (c2, g2, a2) = spawn_chat_gw(cache, store2, groups2, "chat-2", "wg-2").await;

    let (alice, _) = login("alice", &ws_url(a1)).await;
    let mut create = LogicPkt::new(CMD_GROUP_CREATE, 2, Bytes::new());
    create.write_body(&GroupCreateReq {
        name: "shared".into(),
        owner: "alice".into(),
        members: vec!["alice".into(), "bob".into()],
        avatar: String::new(),
        introduction: String::new(),
    });
    alice
        .send(marshal(&Packet::Logic(create)))
        .await
        .expect("create");
    let create_frame = timeout_read(&alice).await;
    let group_id = match read(&create_frame.payload).expect("create resp") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            p.read_body::<GroupCreateResp>().expect("resp").group_id
        }
        _ => panic!("expected create"),
    };

    let (alice2, _) = login("alice", &ws_url(a2)).await;
    let mut detail = LogicPkt::new(CMD_GROUP_DETAIL, 3, Bytes::new());
    detail.set_dest(&group_id);
    alice2
        .send(marshal(&Packet::Logic(detail)))
        .await
        .expect("detail");
    let detail_frame = timeout_read(&alice2).await;
    match read(&detail_frame.payload).expect("detail resp") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let d: GroupDetail = p.read_body().expect("GroupDetail");
            assert_eq!(d.name, "shared");
            assert_eq!(d.members, vec!["alice".to_string()]);
        }
        _ => panic!("expected detail"),
    }

    let _ = g1.shutdown().await;
    let _ = g2.shutdown().await;
    let _ = c1.shutdown().await;
    let _ = c2.shutdown().await;
}
