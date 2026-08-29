//! Talk e2e: 1:1 online/offline and three-person group with one offline.

mod harness;

use std::time::Duration;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    Flag, GroupCreateReq, GroupCreateResp, MessagePush, MessageReq, MessageResp, Status,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_CHAT_GROUP_TALK, CMD_CHAT_USER_TALK, CMD_GROUP_CREATE,
    MESSAGE_TYPE_TEXT,
};

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

#[tokio::test]
async fn alice_to_bob_one_to_one() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;

    let req = talk_pkt(CMD_CHAT_USER_TALK, 2, "bob", "hello world");
    alice
        .send(marshal(&Packet::Logic(req)))
        .await
        .expect("talk send");

    let resp_frame = timeout_read(&alice).await;
    let (message_id, send_time) = match read(&resp_frame.payload).expect("resp decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            assert_eq!(p.header.flag, Flag::Response as i32);
            assert_eq!(p.header.command, CMD_CHAT_USER_TALK);
            let resp: MessageResp = p.read_body().expect("MessageResp");
            assert!(resp.message_id > 10000);
            assert!(resp.send_time > 1000);
            (resp.message_id, resp.send_time)
        }
        _ => panic!("expected MessageResp"),
    };

    let push_frame = timeout_read(&bob).await;
    match read(&push_frame.payload).expect("push decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_CHAT_USER_TALK);
            assert_eq!(p.header.flag, Flag::Push as i32);
            let push: MessagePush = p.read_body().expect("MessagePush");
            assert_eq!(push.message_id, message_id);
            assert_eq!(push.send_time, send_time);
            assert_eq!(push.body, "hello world");
            assert_eq!(push.r#type, 1);
            assert_eq!(push.sender, "alice");
            assert!(push.extra.is_empty());
        }
        _ => panic!("expected MessagePush"),
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn offline_one_to_one_success_without_push() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut carol, _) = login("carol", &url).await;
    carol.close().await.expect("carol close");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let req = talk_pkt(CMD_CHAT_USER_TALK, 2, "carol", "hello world");
    alice
        .send(marshal(&Packet::Logic(req)))
        .await
        .expect("talk send");

    let resp_frame = timeout_read(&alice).await;
    match read(&resp_frame.payload).expect("resp decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let resp: MessageResp = p.read_body().expect("MessageResp");
            assert!(resp.message_id > 10000);
        }
        _ => panic!("expected MessageResp"),
    }
    timeout_no_packet(&alice, Duration::from_secs(2)).await;

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn unknown_dest_is_user_not_found() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let req = talk_pkt(CMD_CHAT_USER_TALK, 2, "zoe", "hello");
    alice
        .send(marshal(&Packet::Logic(req)))
        .await
        .expect("talk send");
    let resp_frame = timeout_read(&alice).await;
    match read(&resp_frame.payload).expect("resp decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::UserNotFound as i32);
        }
        _ => panic!("expected logic"),
    }
    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn three_person_group_one_offline() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;
    let (carol, _) = login("carol", &url).await;

    let mut create = LogicPkt::new(CMD_GROUP_CREATE, 2, Bytes::new());
    create.write_body(&GroupCreateReq {
        name: "group1".into(),
        owner: "alice".into(),
        members: vec!["alice".into(), "bob".into(), "carol".into(), "dave".into()],
        avatar: String::new(),
        introduction: String::new(),
    });
    alice
        .send(marshal(&Packet::Logic(create)))
        .await
        .expect("create send");
    let create_frame = timeout_read(&alice).await;
    let group_id = match read(&create_frame.payload).expect("create decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let resp: GroupCreateResp = p.read_body().expect("GroupCreateResp");
            assert!(!resp.group_id.is_empty());
            resp.group_id
        }
        _ => panic!("expected GroupCreateResp"),
    };

    let talk = talk_pkt(CMD_CHAT_GROUP_TALK, 3, &group_id, "hellogroup");
    alice
        .send(marshal(&Packet::Logic(talk)))
        .await
        .expect("group talk");

    let resp_frame = timeout_read(&alice).await;
    match read(&resp_frame.payload).expect("resp decode") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let resp: MessageResp = p.read_body().expect("MessageResp");
            assert!(resp.message_id > 10000);
            assert!(resp.send_time > 1000);
        }
        _ => panic!("expected MessageResp"),
    }
    timeout_no_packet(&alice, Duration::from_millis(500)).await;

    for (client, name) in [(&bob, "bob"), (&carol, "carol")] {
        let push_frame = timeout_read_skip_group_notify(client).await;
        match read(&push_frame.payload).expect("push decode") {
            Packet::Logic(p) => {
                assert_eq!(p.header.flag, Flag::Push as i32, "{name}");
                assert_eq!(p.header.command, CMD_CHAT_GROUP_TALK, "{name}");
                let push: MessagePush = p.read_body().expect("MessagePush");
                assert_eq!(push.sender, "alice");
                assert_eq!(push.body, "hellogroup");
                assert_eq!(push.r#type, 1);
                assert!(push.extra.is_empty());
                assert!(push.message_id > 10000);
                assert!(push.send_time > 1000);
            }
            _ => panic!("expected MessagePush for {name}"),
        }
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
