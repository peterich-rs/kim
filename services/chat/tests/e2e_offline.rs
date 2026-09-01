//! Offline pull e2e: ACK skips replay; no ACK then reconnect fetches content.

mod harness;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    Flag, MessageAckReq, MessageContentReq, MessageContentResp, MessageIndexReq, MessageIndexResp,
    MessagePush, MessageReq, MessageResp, Status,
};
use kim_protocol::{
    generate_with_jti, marshal, read, LogicPkt, Packet, CMD_CHAT_TALK_ACK, CMD_CHAT_USER_TALK,
    CMD_OFFLINE_CONTENT, CMD_OFFLINE_INDEX, DEMO_DEFAULT_SECRET, MESSAGE_TYPE_TEXT,
};
use kim_ws::WsClient;

fn talk_pkt(command: &str, seq: u32, dest: &str, body: &str) -> LogicPkt {
    let mut pkt = LogicPkt::new(command, seq, Bytes::new());
    pkt.set_dest(dest);
    pkt.write_body(&MessageReq {
        r#type: MESSAGE_TYPE_TEXT,
        body: body.to_string(),
        extra: String::new(),
        client_id: String::new(),
    });
    pkt
}

async fn send_ack(client: &WsClient, seq: u32, message_id: i64) {
    let mut pkt = LogicPkt::new(CMD_CHAT_TALK_ACK, seq, Bytes::new());
    pkt.write_body(&MessageAckReq {
        message_id,
        ..Default::default()
    });
    client
        .send(marshal(&Packet::Logic(pkt)))
        .await
        .expect("ack");
    let frame = timeout_read(client).await;
    match read(&frame.payload).expect("ack decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            assert_eq!(p.header.command, CMD_CHAT_TALK_ACK);
        }
        _ => panic!("expected ack resp"),
    }
}

async fn pull_index(client: &WsClient, seq: u32, message_id: i64) -> Vec<i64> {
    let mut pkt = LogicPkt::new(CMD_OFFLINE_INDEX, seq, Bytes::new());
    pkt.write_body(&MessageIndexReq {
        message_id,
        ..Default::default()
    });
    client
        .send(marshal(&Packet::Logic(pkt)))
        .await
        .expect("index");
    let frame = timeout_read(client).await;
    match read(&frame.payload).expect("index decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let resp: MessageIndexResp = p.read_body().expect("MessageIndexResp");
            resp.indexes.into_iter().map(|i| i.message_id).collect()
        }
        _ => panic!("expected index resp"),
    }
}

async fn pull_content(client: &WsClient, seq: u32, ids: &[i64]) -> Vec<String> {
    let mut pkt = LogicPkt::new(CMD_OFFLINE_CONTENT, seq, Bytes::new());
    pkt.write_body(&MessageContentReq {
        message_ids: ids.to_vec(),
        ..Default::default()
    });
    client
        .send(marshal(&Packet::Logic(pkt)))
        .await
        .expect("content");
    let frame = timeout_read(client).await;
    match read(&frame.payload).expect("content decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let resp: MessageContentResp = p.read_body().expect("MessageContentResp");
            resp.messages.into_iter().map(|m| m.body).collect()
        }
        _ => panic!("expected content resp"),
    }
}

#[tokio::test]
async fn ack_then_reconnect_has_empty_index() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    alice
        .send(marshal(&Packet::Logic(talk_pkt(
            CMD_CHAT_USER_TALK,
            2,
            "bob",
            "hello world",
        ))))
        .await
        .expect("talk");
    let resp_frame = timeout_read(&alice).await;
    let message_id = match read(&resp_frame.payload).expect("resp") {
        Packet::Logic(p) => {
            let resp: MessageResp = p.read_body().expect("MessageResp");
            resp.message_id
        }
        _ => panic!("expected MessageResp"),
    };
    let push_frame = timeout_read(&bob).await;
    match read(&push_frame.payload).expect("push") {
        Packet::Logic(p) => {
            assert_eq!(p.header.flag, Flag::Push as i32);
            let push: MessagePush = p.read_body().expect("MessagePush");
            assert_eq!(push.message_id, message_id);
        }
        _ => panic!("expected push"),
    }
    send_ack(&bob, 3, message_id).await;
    bob.close().await.expect("bob close");

    let (bob2, _) = login("bob", &url).await;
    let ids = pull_index(&bob2, 2, 0).await;
    assert!(
        !ids.contains(&message_id),
        "acked message {message_id} still in {ids:?}"
    );

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn no_ack_then_reconnect_pulls_content() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    alice
        .send(marshal(&Packet::Logic(talk_pkt(
            CMD_CHAT_USER_TALK,
            2,
            "bob",
            "hello world",
        ))))
        .await
        .expect("talk");
    let resp_frame = timeout_read(&alice).await;
    let message_id = match read(&resp_frame.payload).expect("resp") {
        Packet::Logic(p) => {
            p.read_body::<MessageResp>()
                .expect("MessageResp")
                .message_id
        }
        _ => panic!("expected MessageResp"),
    };
    let _ = timeout_read(&bob).await;
    bob.close().await.expect("bob close");

    let (bob2, _) = login("bob", &url).await;
    let ids = pull_index(&bob2, 2, 0).await;
    assert!(ids.contains(&message_id), "missing {message_id} in {ids:?}");
    let bodies = pull_content(&bob2, 3, &[message_id]).await;
    assert_eq!(bodies, vec!["hello world".to_string()]);

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn carol_cannot_pull_alice_bob_content() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    alice
        .send(marshal(&Packet::Logic(talk_pkt(
            CMD_CHAT_USER_TALK,
            2,
            "bob",
            "private",
        ))))
        .await
        .expect("talk");
    let resp_frame = timeout_read(&alice).await;
    let message_id = match read(&resp_frame.payload).expect("resp") {
        Packet::Logic(p) => {
            p.read_body::<MessageResp>()
                .expect("MessageResp")
                .message_id
        }
        _ => panic!("expected MessageResp"),
    };
    let _ = timeout_read(&bob).await;
    bob.close().await.expect("bob close");

    let (carol, _) = login("carol", &url).await;
    let bodies = pull_content(&carol, 2, &[message_id]).await;
    assert!(bodies.is_empty(), "carol saw {bodies:?}");

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

async fn pull_index_resume(client: &WsClient, seq: u32) -> (Vec<i64>, bool) {
    let mut pkt = LogicPkt::new(CMD_OFFLINE_INDEX, seq, Bytes::new());
    pkt.write_body(&MessageIndexReq {
        message_id: 0,
        resume: true,
    });
    client
        .send(marshal(&Packet::Logic(pkt)))
        .await
        .expect("index");
    let frame = timeout_read(client).await;
    match read(&frame.payload).expect("index decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let resp: MessageIndexResp = p.read_body().expect("MessageIndexResp");
            (
                resp.indexes.into_iter().map(|i| i.message_id).collect(),
                resp.has_more,
            )
        }
        _ => panic!("expected index resp"),
    }
}

async fn send_ack_ids(client: &WsClient, seq: u32, ids: &[i64]) -> i32 {
    let mut pkt = LogicPkt::new(CMD_CHAT_TALK_ACK, seq, Bytes::new());
    pkt.write_body(&MessageAckReq {
        message_ids: ids.to_vec(),
        ..Default::default()
    });
    client
        .send(marshal(&Packet::Logic(pkt)))
        .await
        .expect("ack");
    let frame = timeout_read(client).await;
    match read(&frame.payload).expect("ack decode") {
        Packet::Logic(p) => p.header.status,
        _ => panic!("expected ack resp"),
    }
}

async fn pending_stack() -> Stack {
    let idgen: std::sync::Arc<dyn chat::idgen::IdGenerator> =
        std::sync::Arc::new(chat::idgen::SequenceIdGen::default());
    let store = std::sync::Arc::new(chat::store::MemoryMessageStore::with_pending_receipt(
        idgen.clone(),
    ));
    let groups = std::sync::Arc::new(chat::directory::MemoryGroupDirectory::new(idgen));
    spawn_stack_pending(store, groups).await
}

async fn talk_and_push(alice: &WsClient, bob: &mut WsClient, body: &str, seq: u32) -> i64 {
    alice
        .send(marshal(&Packet::Logic(talk_pkt(
            CMD_CHAT_USER_TALK,
            seq,
            "bob",
            body,
        ))))
        .await
        .expect("talk");
    let resp_frame = timeout_read(alice).await;
    let message_id = match read(&resp_frame.payload).expect("resp") {
        Packet::Logic(p) => {
            p.read_body::<MessageResp>()
                .expect("MessageResp")
                .message_id
        }
        _ => panic!("expected MessageResp"),
    };
    let _ = timeout_read(bob).await;
    message_id
}

#[tokio::test]
async fn pending_ack_one_id_keeps_earlier_hole() {
    let stack = pending_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    let first = talk_and_push(&alice, &mut bob, "one", 2).await;
    let second = talk_and_push(&alice, &mut bob, "two", 3).await;
    send_ack(&bob, 4, second).await;
    let (ids, _) = pull_index_resume(&bob, 5).await;
    assert!(ids.contains(&first), "hole {first} missing in {ids:?}");
    assert!(!ids.contains(&second), "acked {second} still in {ids:?}");

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn leftover_index_without_resume_stops() {
    let stack = pending_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;
    let _ = talk_and_push(&alice, &mut bob, "keep", 2).await;
    bob.close().await.expect("close");

    let (bob2, _) = login("bob", &url).await;
    let first = pull_index(&bob2, 2, 0).await;
    assert!(!first.is_empty());
    let second = pull_index(&bob2, 3, first[0]).await;
    assert!(second.is_empty());

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn ack_201_ids_is_invalid_packet() {
    let stack = pending_stack().await;
    let url = ws_url(stack.gw_addr);
    let (bob, _) = login("bob", &url).await;
    let ids: Vec<i64> = (1..=201).collect();
    let status = send_ack_ids(&bob, 2, &ids).await;
    assert_eq!(status, Status::InvalidPacketBody as i32);

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

fn jwt_with_jti(account: &str, jti: &str) -> String {
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64
        + 3600;
    generate_with_jti(DEMO_DEFAULT_SECRET, account, "kim", exp, jti).expect("jwt")
}

#[tokio::test]
async fn same_jti_reconnect_does_not_resurrect_acked() {
    let stack = pending_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let bob_token = jwt_with_jti("bob", "bob-jti");
    let (mut bob, _) = login_with_token("bob", &url, bob_token.clone()).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    let id = talk_and_push(&alice, &mut bob, "keep-ack", 2).await;
    send_ack(&bob, 4, id).await;
    bob.close().await.expect("bob close");

    let (bob2, _) = login_with_token("bob", &url, bob_token).await;
    let (ids, _) = pull_index_resume(&bob2, 2).await;
    assert!(
        !ids.contains(&id),
        "acked {id} resurfaced after same-jti reconnect: {ids:?}"
    );

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
