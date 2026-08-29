mod harness;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chat::ChatHandler;
use gateway::{GatewayHandler, KickHook};
use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer, ADULT};
use kim_core::Server;
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_protocol::{generate, DEMO_DEFAULT_SECRET};
use kim_session::open_session_store;
use kim_tcp::{TcpConn, TcpServer};
use pkt_client::perform_login;
use tokio::net::TcpStream;

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
async fn tcp_gateway_login() {
    let cache = open_session_store(None).await.expect("memory");
    let mut chat_server = TcpServer::bind("127.0.0.1:0").await.expect("chat bind");
    let chat_addr = chat_server.local_addr();
    let chat_c = Container::new(ContainerOpts {
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
    let chat_h = Arc::new(ChatHandler::new(chat_c.clone(), cache.clone()));
    chat_server.set_acceptor(chat_h.clone());
    chat_server.set_message_listener(chat_h.clone());
    chat_server.set_state_listener(chat_h);
    chat_c.attach_server(Arc::new(chat_server));
    let chat_run = chat_c.clone();
    tokio::spawn(async move {
        let _ = chat_run.start().await;
    });

    let mut gw_server = TcpServer::bind("127.0.0.1:0").await.expect("tgw bind");
    let gw_addr = gw_server.local_addr();
    let hook = Arc::new(KickHook::new());
    let naming = Arc::new(StaticNaming::from_slice(vec![DefaultRegistration {
        service_id: "chat-1".into(),
        service_name: "chat".into(),
        protocol: "tcp".into(),
        public_address: "127.0.0.1".into(),
        public_port: chat_addr.port(),
        tags: vec![],
        meta: HashMap::new(),
    }]));
    let gw_c = Container::new(ContainerOpts {
        naming,
        identity: ident("tg-1", "tgateway"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: "tg-1".into(),
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
        after_downlink: vec![hook.clone()],
    });
    let gw_h = Arc::new(GatewayHandler::new(
        gw_c.clone(),
        "tg-1",
        DEMO_DEFAULT_SECRET,
    ));
    gw_server.set_acceptor(gw_h.clone());
    gw_server.set_message_listener(gw_h.clone());
    gw_server.set_state_listener(gw_h.clone());
    let gw_server = Arc::new(gw_server);
    hook.attach(gw_server.clone());
    gw_h.attach_server(gw_server.clone());
    gw_c.attach_server(gw_server);
    let gw_run = gw_c.clone();
    tokio::spawn(async move {
        let _ = gw_run.start().await;
    });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if gw_c.slot_state("chat", "chat-1").await == Some(ADULT) {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("chat not adult");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let stream = TcpStream::connect(gw_addr).await.expect("dial tgw");
    let mut conn = TcpConn::new(stream);
    let token = generate(DEMO_DEFAULT_SECRET, "tg-alice", "kim", i64::MAX / 4).expect("jwt");
    let channel_id = perform_login(&mut conn, token).await.expect("login");
    assert!(channel_id.starts_with("tg-1_tg-alice_"), "{channel_id}");

    let _ = gw_c.shutdown().await;
    let _ = chat_c.shutdown().await;
}
