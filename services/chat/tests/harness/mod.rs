//! Shared process stack and WS helpers for chat e2e tests.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use chat::directory::GroupDirectory;
use chat::store::MessageStore;
use chat::ChatHandler;
use gateway::{GatewayHandler, KickHook};
use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer, ADULT};
use kim_core::{Conn, OpCode, Server};
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_protocol::pkt::Flag;
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_FRIEND_ACCEPT, CMD_FRIEND_REQUEST, CMD_GROUP_CREATE,
    DEMO_DEFAULT_SECRET,
};
use kim_router::SessionStorage;
use kim_session::open_session_store;
use kim_tcp::TcpServer;
use kim_ws::{connect_ws, ClientOptions, WsClient, WsServer};
use pkt_client::LoginDialer;

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

pub struct Stack {
    pub gw: Arc<Container>,
    pub chat: Arc<Container>,
    pub gw_addr: std::net::SocketAddr,
    pub cache: Arc<dyn SessionStorage>,
    pub gw_server: Arc<WsServer>,
}

fn attach_chat_handler(server: &mut TcpServer, handler: Arc<ChatHandler>) {
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
}

pub async fn spawn_stack() -> Stack {
    spawn_stack_with_chat(ChatHandler::new).await
}

pub async fn spawn_stack_seams(
    store: Arc<dyn MessageStore>,
    groups: Arc<dyn GroupDirectory>,
) -> Stack {
    spawn_stack_with_chat(move |c, cache| ChatHandler::with_seams(c, cache, store, groups)).await
}

pub async fn spawn_stack_pending(
    store: Arc<dyn MessageStore>,
    groups: Arc<dyn GroupDirectory>,
) -> Stack {
    spawn_stack_with_chat(move |c, cache| {
        ChatHandler::with_seams_pending(c, cache, store, groups, true)
    })
    .await
}

async fn spawn_stack_with_chat<F>(make_chat: F) -> Stack
where
    F: FnOnce(Arc<Container>, Arc<dyn SessionStorage>) -> ChatHandler,
{
    let cache = open_session_store(None).await.expect("memory store");

    let mut chat_server = TcpServer::bind("127.0.0.1:0").await.expect("chat bind");
    chat_server.set_drain_wait(Duration::from_millis(50));
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
    let chat_h = Arc::new(make_chat(chat_c.clone(), cache.clone()));
    attach_chat_handler(&mut chat_server, chat_h);
    chat_c.attach_server(Arc::new(chat_server));
    let chat_run = chat_c.clone();
    tokio::spawn(async move {
        let _ = chat_run.start().await;
    });

    let mut gw_server = WsServer::bind("127.0.0.1:0").await.expect("gw bind");
    gw_server.set_drain_wait(Duration::from_millis(50));
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
        identity: ident("wg-1", "wgateway"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: "wg-1".into(),
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
        after_downlink: vec![hook.clone()],
    });
    let gw_h = Arc::new(GatewayHandler::new(
        gw_c.clone(),
        "wg-1",
        DEMO_DEFAULT_SECRET,
    ));
    gw_h.set_revoke(Arc::new(gateway::AllowAllRevoke));
    gw_server.set_acceptor(gw_h.clone());
    gw_server.set_message_listener(gw_h.clone());
    gw_server.set_state_listener(gw_h.clone());
    let gw_server = Arc::new(gw_server);
    hook.attach(gw_server.clone());
    gw_h.attach_server(gw_server.clone());
    gw_c.attach_server(gw_server.clone());
    let gw_run = gw_c.clone();
    tokio::spawn(async move {
        let _ = gw_run.start().await;
    });
    wait_adult(&gw_c).await;
    wait_ws(&ws_url(gw_addr)).await;

    Stack {
        gw: gw_c,
        chat: chat_c,
        gw_addr,
        cache,
        gw_server,
    }
}

pub async fn spawn_gateway_only() -> (Arc<Container>, std::net::SocketAddr) {
    let mut gw_server = WsServer::bind("127.0.0.1:0").await.expect("gw bind");
    gw_server.set_drain_wait(Duration::from_millis(50));
    let gw_addr = gw_server.local_addr();
    let hook = Arc::new(KickHook::new());
    let naming = Arc::new(StaticNaming::from_slice(vec![DefaultRegistration {
        service_id: "chat-1".into(),
        service_name: "chat".into(),
        protocol: "tcp".into(),
        public_address: "127.0.0.1".into(),
        public_port: 1,
        tags: vec![],
        meta: HashMap::new(),
    }]));
    let gw_c = Container::new(ContainerOpts {
        naming,
        identity: ident("wg-1", "wgateway"),
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: "wg-1".into(),
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
        after_downlink: vec![hook.clone()],
    });
    let gw_h = Arc::new(GatewayHandler::new(
        gw_c.clone(),
        "wg-1",
        DEMO_DEFAULT_SECRET,
    ));
    gw_h.set_revoke(Arc::new(gateway::AllowAllRevoke));
    gw_server.set_acceptor(gw_h.clone());
    gw_server.set_message_listener(gw_h.clone());
    gw_server.set_state_listener(gw_h.clone());
    let gw_server = Arc::new(gw_server);
    hook.attach(gw_server.clone());
    gw_h.attach_server(gw_server.clone());
    gw_c.attach_server(gw_server.clone());
    let gw_run = gw_c.clone();
    tokio::spawn(async move {
        let _ = gw_run.start().await;
    });
    wait_ws(&ws_url(gw_addr)).await;
    (gw_c, gw_addr)
}

async fn wait_adult(gw: &Container) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if gw.slot_state("chat", "chat-1").await == Some(ADULT) {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("chat slot did not become Adult");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

async fn wait_ws(url: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        match connect_ws(url).await {
            Ok(mut conn) => {
                let _ = conn.write_frame(OpCode::Close, Bytes::new()).await;
                let _ = conn.shutdown().await;
                return;
            }
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Err(err) => panic!("ws not ready: {err}"),
        }
    }
}

pub fn ws_url(addr: std::net::SocketAddr) -> String {
    format!("ws://{addr}/")
}

pub async fn become_friends(from: &WsClient, to: &WsClient, to_acc: &str, from_acc: &str) {
    let mut req = LogicPkt::new(CMD_FRIEND_REQUEST, 90, Bytes::new());
    req.set_dest(to_acc);
    from.send(marshal(&Packet::Logic(req)))
        .await
        .expect("friend request");
    let frame = timeout_read(from).await;
    match read(&frame.payload).expect("req decode") {
        Packet::Logic(p) => assert_eq!(p.header.status, kim_protocol::pkt::Status::Success as i32),
        _ => panic!("expected friend request resp"),
    }
    let mut acc = LogicPkt::new(CMD_FRIEND_ACCEPT, 91, Bytes::new());
    acc.set_dest(from_acc);
    to.send(marshal(&Packet::Logic(acc)))
        .await
        .expect("friend accept");
    drain_friend_packets(from).await;
    drain_friend_packets(to).await;
}

async fn drain_friend_packets(client: &WsClient) {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(200);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(50), client.read()).await {
            Ok(Ok(frame)) => match read(&frame.payload) {
                Ok(Packet::Logic(p))
                    if p.header.command.starts_with("chat.friend")
                        || p.header.command.starts_with("chat.block") => {}
                _ => return,
            },
            _ => return,
        }
    }
}

pub async fn login(account: &str, url: &str) -> (WsClient, Arc<LoginDialer>) {
    login_with_device(account, url, "").await
}

pub async fn login_with_device(
    account: &str,
    url: &str,
    device: &str,
) -> (WsClient, Arc<LoginDialer>) {
    let dialer = Arc::new(LoginDialer::new(DEMO_DEFAULT_SECRET).with_device(device));
    login_with_dialer(account, url, dialer).await
}

pub async fn login_with_token(
    account: &str,
    url: &str,
    token: String,
) -> (WsClient, Arc<LoginDialer>) {
    let dialer = Arc::new(LoginDialer::new(DEMO_DEFAULT_SECRET).with_token(token));
    login_with_dialer(account, url, dialer).await
}

async fn login_with_dialer(
    account: &str,
    url: &str,
    dialer: Arc<LoginDialer>,
) -> (WsClient, Arc<LoginDialer>) {
    let mut client = WsClient::new(
        account,
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(dialer.clone());
    client.connect(url).await.expect("login connect");
    (client, dialer)
}

pub async fn timeout_read_skip_group_notify(client: &WsClient) -> kim_core::Frame {
    loop {
        let frame = timeout_read(client).await;
        match read(&frame.payload) {
            Ok(Packet::Logic(p))
                if p.header.flag == Flag::Push as i32 && p.header.command == CMD_GROUP_CREATE =>
            {
                continue;
            }
            _ => return frame,
        }
    }
}

pub async fn timeout_read(client: &WsClient) -> kim_core::Frame {
    tokio::time::timeout(Duration::from_secs(2), client.read())
        .await
        .expect("read timeout")
        .expect("read")
}

pub async fn timeout_no_packet(client: &WsClient, dur: Duration) {
    match tokio::time::timeout(dur, client.read()).await {
        Err(_) => {}
        Ok(Ok(_)) => panic!("unexpected packet"),
        Ok(Err(err)) => panic!("read error: {err}"),
    }
}

pub fn assert_channel_id(id: &str, account: &str) {
    assert_ne!(id, "alice");
    assert_ne!(id, account);
    let prefix = format!("wg-1_{account}_");
    assert!(
        id.starts_with(&prefix),
        "channel_id {id} should start with {prefix}"
    );
    let seq = &id[prefix.len()..];
    let n: u64 = seq.parse().unwrap_or(0);
    assert!(n >= 1, "seq in {id}");
}
