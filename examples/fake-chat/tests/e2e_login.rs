//! Login e2e: JWT handshake, echo after login, kickout, bad token, identity, unavailable.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use fake_chat::ChatHandler;
use fake_gateway::{GatewayHandler, KickHook};
use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer, ADULT};
use kim_core::{Conn, OpCode, Server};
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_protocol::pkt::{Flag, Status};
use kim_protocol::{
    marshal, read, read_logic, BasicPkt, LogicPkt, Packet, CMD_DEMO_ECHO, CODE_PONG,
    DEMO_DEFAULT_SECRET,
};
use kim_router::SessionStorage;
use kim_session::open_session_store;
use kim_tcp::TcpServer;
use kim_ws::{connect_ws, ClientOptions, WsClient, WsIdentityDialer, WsServer};
use pkt_client::{is_kickout, LoginDialer};

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

struct Stack {
    gw: Arc<Container>,
    chat: Arc<Container>,
    gw_addr: std::net::SocketAddr,
    cache: Arc<dyn SessionStorage>,
    gw_server: Arc<WsServer>,
}

async fn spawn_stack() -> Stack {
    let cache = open_session_store(None).await.expect("memory store");

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
        after_downlink: None,
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

    let mut gw_server = WsServer::bind("127.0.0.1:0").await.expect("gw bind");
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
        after_downlink: Some(hook.clone()),
    });
    let gw_h = Arc::new(GatewayHandler::new(
        gw_c.clone(),
        "wg-1",
        DEMO_DEFAULT_SECRET,
    ));
    gw_server.set_acceptor(gw_h.clone());
    gw_server.set_message_listener(gw_h.clone());
    gw_server.set_state_listener(gw_h);
    let gw_server = Arc::new(gw_server);
    hook.attach(gw_server.clone());
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

async fn spawn_gateway_only() -> (Arc<Container>, std::net::SocketAddr) {
    let mut gw_server = WsServer::bind("127.0.0.1:0").await.expect("gw bind");
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
        after_downlink: Some(hook.clone()),
    });
    let gw_h = Arc::new(GatewayHandler::new(
        gw_c.clone(),
        "wg-1",
        DEMO_DEFAULT_SECRET,
    ));
    gw_server.set_acceptor(gw_h.clone());
    gw_server.set_message_listener(gw_h.clone());
    gw_server.set_state_listener(gw_h);
    let gw_server = Arc::new(gw_server);
    hook.attach(gw_server.clone());
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

fn ws_url(addr: std::net::SocketAddr) -> String {
    format!("ws://{addr}/")
}

async fn login(account: &str, url: &str) -> (WsClient, Arc<LoginDialer>) {
    let dialer = Arc::new(LoginDialer::new(DEMO_DEFAULT_SECRET));
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

async fn timeout_read(client: &WsClient) -> kim_core::Frame {
    tokio::time::timeout(Duration::from_secs(2), client.read())
        .await
        .expect("read timeout")
        .expect("read")
}

fn assert_channel_id(id: &str, account: &str) {
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

#[tokio::test]
async fn login_channel_id_echo_and_login_resp_does_not_close() {
    // #2 channel_id form, #4 echo after login, #15 LoginResp must not close.
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (client, dialer) = login("alice", &url).await;
    let channel_id = dialer.channel_id().expect("channel_id");
    assert_channel_id(&channel_id, "alice");

    client
        .send(marshal(&Packet::Basic(BasicPkt::ping())))
        .await
        .expect("ping");
    let pong = timeout_read(&client).await;
    match read(&pong.payload).expect("pong decode") {
        Packet::Basic(p) => assert_eq!(p.code, CODE_PONG),
        _ => panic!("expected pong after LoginResp"),
    }

    let seq = 2u32;
    let req = LogicPkt::new(CMD_DEMO_ECHO, seq, Bytes::from_static(b"hello pkt"));
    client
        .send(marshal(&Packet::Logic(req)))
        .await
        .expect("echo send");
    let resp = timeout_read(&client).await;
    match read(&resp.payload).expect("echo decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.sequence, seq);
            assert_eq!(p.header.flag, Flag::Response as i32);
            assert_eq!(p.header.status, Status::Success as i32);
            assert_eq!(&p.body[..], b"hello pkt");
        }
        _ => panic!("expected echo logic"),
    }

    assert!(
        stack.gw_server.channel_map().contains(&channel_id).await,
        "LoginResp must not close_channel"
    );

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn second_login_kickout_then_logout_keeps_new_location() {
    // #6 kickout + close; #7 logout << 60s; second still echoes.
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);

    let (mut first, d1) = login("alice", &url).await;
    let id1 = d1.channel_id().expect("id1");
    assert_channel_id(&id1, "alice");

    let (second, d2) = login("alice", &url).await;
    let id2 = d2.channel_id().expect("id2");
    assert_channel_id(&id2, "alice");
    assert_ne!(id1, id2);

    let kick = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let frame = first.read().await.expect("first read");
            if let Packet::Logic(p) = read(&frame.payload).expect("kick decode") {
                if let Some(notify) = is_kickout(&p) {
                    return notify;
                }
            }
        }
    })
    .await
    .expect("kickout timeout");
    assert_eq!(kick.channel_id, id1);
    first.close().await.expect("first close");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        match stack.cache.get(&id1).await {
            Err(kim_router::SessionError::NotFound) => break,
            _ if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            _ => panic!("id1 session still present after 2s (logout too slow)"),
        }
    }
    let loc = stack
        .cache
        .get_location("alice", "")
        .await
        .expect("location");
    assert_eq!(loc.channel_id, id2);

    let req = LogicPkt::new(CMD_DEMO_ECHO, 2, Bytes::from_static(b"hello pkt"));
    second
        .send(marshal(&Packet::Logic(req)))
        .await
        .expect("second echo");
    let resp = timeout_read(&second).await;
    match read(&resp.payload).expect("second echo decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            assert_eq!(&p.body[..], b"hello pkt");
        }
        _ => panic!("expected echo"),
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn bad_token_unauthorized_or_close_not_added() {
    // #8
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let dialer = Arc::new(LoginDialer::new(DEMO_DEFAULT_SECRET).with_bad_token());
    let mut client = WsClient::new(
        "alice",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(dialer);
    let err = client.connect(&url).await.expect_err("bad token must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("closed") || msg.contains("status=105"),
        "expected closed or Unauthorized status=105, got: {msg}"
    );
    assert!(
        stack.gw_server.channel_map().is_empty().await,
        "bad token must not add channel"
    );

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn identity_first_frame_is_close_without_logic_pkt() {
    // #9a
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let mut conn = connect_ws(&url).await.expect("upgrade");
    conn.write_frame(OpCode::Binary, Bytes::from_static(b"alice"))
        .await
        .expect("write identity");
    let outcome = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let frame = match conn.read_frame().await {
                Ok(f) => f,
                Err(kim_core::Error::Closed) => return Err("closed"),
                Err(e) => panic!("read: {e}"),
            };
            match frame.opcode {
                OpCode::Close => return Err("closed"),
                OpCode::Ping => {
                    let _ = conn.write_frame(OpCode::Pong, Bytes::new()).await;
                }
                OpCode::Pong | OpCode::Continuation => {}
                OpCode::Binary | OpCode::Text => {
                    if read_logic(&frame.payload).is_ok() {
                        return Ok(());
                    }
                    return Err("non-logic binary");
                }
            }
        }
    })
    .await
    .expect("identity handshake timeout");
    assert!(outcome.is_err(), "identity must not yield LogicPkt");

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn ws_identity_dialer_cannot_login() {
    // #9a via WsIdentityDialer
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let mut client = WsClient::new(
        "alice",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(WsIdentityDialer));
    if let Ok(()) = client.connect(&url).await {
        let res = tokio::time::timeout(Duration::from_secs(2), client.read()).await;
        match res {
            Ok(Err(_)) => {}
            Ok(Ok(frame)) => {
                assert!(
                    read_logic(&frame.payload).is_err(),
                    "identity must not produce LogicPkt"
                );
            }
            Err(_) => panic!("expected close after identity"),
        }
    }
    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn wrong_command_first_frame_is_invalid_command() {
    // #9b
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let mut conn = connect_ws(&url).await.expect("upgrade");
    let pkt = LogicPkt::new(CMD_DEMO_ECHO, 1, Bytes::from_static(b"nope"));
    conn.write_frame(OpCode::Binary, marshal(&Packet::Logic(pkt)))
        .await
        .expect("write wrong cmd");
    let mut saw_invalid = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        let frame = match tokio::time::timeout(Duration::from_millis(500), conn.read_frame()).await
        {
            Ok(Ok(f)) => f,
            Ok(Err(kim_core::Error::Closed)) => break,
            Ok(Err(e)) => panic!("read: {e}"),
            Err(_) => break,
        };
        match frame.opcode {
            OpCode::Close => break,
            OpCode::Binary | OpCode::Text => match read(&frame.payload) {
                Ok(Packet::Logic(p)) => {
                    assert_eq!(p.header.status, Status::InvalidCommand as i32);
                    saw_invalid = true;
                }
                _ => panic!("expected logic InvalidCommand"),
            },
            OpCode::Ping => {
                let _ = conn.write_frame(OpCode::Pong, Bytes::new()).await;
            }
            _ => {}
        }
    }
    assert!(saw_invalid, "expected Binary InvalidCommand=103 then Close");

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn unavailable_without_chat_fails_handshake() {
    // #11 handshake fails; not login-then-echo.
    let (gw, addr) = spawn_gateway_only().await;
    let url = ws_url(addr);
    let dialer = Arc::new(LoginDialer::new(DEMO_DEFAULT_SECRET));
    let mut client = WsClient::new(
        "alice",
        "test",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(dialer.clone());
    let err = client
        .connect(&url)
        .await
        .expect_err("handshake must fail without chat");
    let msg = err.to_string();
    assert!(
        msg.contains("closed") || msg.contains("status=3"),
        "expected closed or ServiceUnavailable status=3, got: {msg}"
    );
    assert!(dialer.channel_id().is_none());

    let _ = gw.shutdown().await;
}

#[tokio::test]
async fn echo_after_session_delete_is_session_not_found() {
    // #10: dest.channels must be set or the client never sees 404.
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (client, dialer) = login("alice", &url).await;
    let id = dialer.channel_id().expect("channel_id");
    stack
        .cache
        .delete("alice", &id)
        .await
        .expect("delete session");

    let req = LogicPkt::new(CMD_DEMO_ECHO, 2, Bytes::from_static(b"hello pkt"));
    client
        .send(marshal(&Packet::Logic(req)))
        .await
        .expect("echo send");
    let resp = timeout_read(&client).await;
    match read(&resp.payload).expect("decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::SessionNotFound as i32);
            assert_eq!(p.header.flag, Flag::Response as i32);
            assert_eq!(p.header.sequence, 2);
        }
        _ => panic!("expected SessionNotFound logic"),
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
