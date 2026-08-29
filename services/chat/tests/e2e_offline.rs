//! Offline pull e2e: ACK skips replay; no ACK then reconnect fetches content.

mod harness;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    Flag, GroupCreateReq, GroupCreateResp, MessageAckReq, MessageContentReq, MessageContentResp,
    MessageIndexReq, MessageIndexResp, MessagePush, MessageReq, MessageResp, Status,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_CHAT_GROUP_TALK, CMD_CHAT_TALK_ACK, CMD_CHAT_USER_TALK,
    CMD_GROUP_CREATE, CMD_OFFLINE_CONTENT, CMD_OFFLINE_INDEX, MESSAGE_TYPE_TEXT,
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
    pkt.write_body(&MessageAckReq { message_id });
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
    pkt.write_body(&MessageIndexReq { message_id });
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
async fn group_offline_member_pulls_on_login() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;

    let mut create = LogicPkt::new(CMD_GROUP_CREATE, 2, Bytes::new());
    create.write_body(&GroupCreateReq {
        name: "group1".into(),
        owner: "alice".into(),
        members: vec!["alice".into(), "bob".into(), "carol".into()],
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
            p.read_body::<GroupCreateResp>()
                .expect("GroupCreateResp")
                .group_id
        }
        _ => panic!("expected create resp"),
    };

    alice
        .send(marshal(&Packet::Logic(talk_pkt(
            CMD_CHAT_GROUP_TALK,
            3,
            &group_id,
            "hellogroup",
        ))))
        .await
        .expect("gtalk");
    let resp_frame = timeout_read(&alice).await;
    let message_id = match read(&resp_frame.payload).expect("gtalk resp") {
        Packet::Logic(p) => {
            p.read_body::<MessageResp>()
                .expect("MessageResp")
                .message_id
        }
        _ => panic!("expected MessageResp"),
    };
    let push_frame = timeout_read_skip_group_notify(&bob).await;
    match read(&push_frame.payload).expect("bob push") {
        Packet::Logic(p) => {
            assert_eq!(p.header.flag, Flag::Push as i32);
            let push: MessagePush = p.read_body().expect("MessagePush");
            assert_eq!(push.message_id, message_id);
            assert_eq!(push.sender, "alice");
        }
        _ => panic!("expected push"),
    }

    let (carol, _) = login("carol", &url).await;
    let ids = pull_index(&carol, 2, 0).await;
    assert!(ids.contains(&message_id), "carol missing {message_id}");
    let bodies = pull_content(&carol, 3, &[message_id]).await;
    assert_eq!(bodies, vec!["hellogroup".to_string()]);

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
