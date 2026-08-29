//! Login e2e: JWT handshake, echo after login, kickout, bad token, identity, unavailable.

mod harness;

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use harness::*;
use kim_core::{Conn, OpCode};
use kim_protocol::pkt::{Flag, Status};
use kim_protocol::{
    marshal, read, read_logic, BasicPkt, LogicPkt, Packet, CMD_DEMO_ECHO, CODE_PONG,
    DEMO_DEFAULT_SECRET,
};
use kim_ws::{connect_ws, ClientOptions, WsClient, WsIdentityDialer};
use pkt_client::{is_kickout, LoginDialer};

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
