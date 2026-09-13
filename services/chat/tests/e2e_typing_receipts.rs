//! Typing (room-interest scoped) + DM read receipts e2e.

#![allow(clippy::unwrap_used)]
mod harness;

use std::time::Duration;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    ConversationReadReq, Flag, MessageReq, MessageResp, ReadReceiptPush, RoomEnterReq, Status,
    TypingPush, TypingReq,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_CHAT_GROUP_TALK, CMD_CHAT_USER_TALK, CMD_GROUP_CREATE,
    CMD_INBOX_READ, CMD_RECEIPT_READ, CMD_ROOM_ENTER, CMD_TYPING, INBOX_KIND_GROUP,
    INBOX_KIND_USER, MESSAGE_TYPE_TEXT,
};

fn enter_pkt(seq: u32, dest: &str) -> LogicPkt {
    let mut pkt = LogicPkt::new(CMD_ROOM_ENTER, seq, Bytes::new());
    pkt.write_body(&RoomEnterReq {
        dest: dest.into(),
        kind: INBOX_KIND_USER,
    });
    pkt
}

fn typing_pkt(seq: u32, dest: &str, active: bool) -> LogicPkt {
    let mut pkt = LogicPkt::new(CMD_TYPING, seq, Bytes::new());
    pkt.write_body(&TypingReq {
        dest: dest.into(),
        kind: INBOX_KIND_USER,
        active,
    });
    pkt
}

fn talk_pkt(seq: u32, dest: &str, body: &str) -> LogicPkt {
    let mut pkt = LogicPkt::new(CMD_CHAT_USER_TALK, seq, Bytes::new());
    pkt.set_dest(dest);
    pkt.write_body(&MessageReq {
        r#type: MESSAGE_TYPE_TEXT,
        body: body.into(),
        extra: String::new(),
        client_id: String::new(),
    });
    pkt
}

fn read_pkt(seq: u32, dest: &str, message_id: i64, kind: i32) -> LogicPkt {
    let mut pkt = LogicPkt::new(CMD_INBOX_READ, seq, Bytes::new());
    pkt.set_dest(dest);
    pkt.write_body(&ConversationReadReq { message_id, kind });
    pkt
}

#[tokio::test]
async fn typing_only_reaches_interested_peer_in_that_dm() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;
    let (carol, _) = login("carol", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;
    become_friends(&alice, &carol, "carol", "alice").await;

    // Bob viewing Alice↔Bob; Carol viewing Alice↔Carol — different rooms.
    bob.send(marshal(&Packet::Logic(enter_pkt(2, "alice"))))
        .await
        .expect("bob enter");
    let _ = timeout_read(&bob).await;
    carol
        .send(marshal(&Packet::Logic(enter_pkt(2, "alice"))))
        .await
        .expect("carol enter");
    let _ = timeout_read(&carol).await;

    // Alice typing toward Bob only.
    alice
        .send(marshal(&Packet::Logic(typing_pkt(3, "bob", true))))
        .await
        .expect("typing");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("typing resp") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected typing resp"),
    }

    let frame = timeout_read(&bob).await;
    match read(&frame.payload).expect("typing push") {
        Packet::Logic(p) => {
            assert_eq!(p.header.flag, Flag::Push as i32);
            assert_eq!(p.header.command, CMD_TYPING);
            let push: TypingPush = p.read_body().expect("body");
            assert_eq!(push.typer, "alice");
            assert_eq!(push.dest, "bob");
            assert!(push.active);
        }
        _ => panic!("expected typing push"),
    }
    // Carol must not see Alice→Bob typing.
    timeout_no_packet(&carol, Duration::from_millis(400)).await;

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn typing_without_interest_is_silent() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    // Bob online but has not entered Alice's room.
    alice
        .send(marshal(&Packet::Logic(typing_pkt(2, "bob", true))))
        .await
        .expect("typing");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("resp") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected resp"),
    }
    timeout_no_packet(&bob, Duration::from_millis(400)).await;

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn dm_mark_read_pushes_receipt_to_peer() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    alice
        .send(marshal(&Packet::Logic(talk_pkt(2, "bob", "hi"))))
        .await
        .expect("talk");
    let frame = timeout_read(&alice).await;
    let message_id = match read(&frame.payload).expect("talk resp") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            p.read_body::<MessageResp>().expect("resp").message_id
        }
        _ => panic!("expected talk resp"),
    };
    let _ = timeout_read(&bob).await; // message push

    bob.send(marshal(&Packet::Logic(read_pkt(
        3,
        "alice",
        message_id,
        INBOX_KIND_USER,
    ))))
    .await
    .expect("mark read");
    let frame = timeout_read(&bob).await;
    match read(&frame.payload).expect("read resp") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected read resp"),
    }

    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("receipt") {
        Packet::Logic(p) => {
            assert_eq!(p.header.flag, Flag::Push as i32);
            assert_eq!(p.header.command, CMD_RECEIPT_READ);
            let push: ReadReceiptPush = p.read_body().expect("body");
            assert_eq!(push.reader, "bob");
            assert_eq!(push.dest, "alice");
            assert_eq!(push.message_id, message_id);
            assert_eq!(push.kind, INBOX_KIND_USER);
        }
        _ => panic!("expected receipt push"),
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn group_mark_read_does_not_push_receipt() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;

    let mut create = LogicPkt::new(CMD_GROUP_CREATE, 2, Bytes::new());
    create.write_body(&kim_protocol::pkt::GroupCreateReq {
        name: "g1".into(),
        owner: "alice".into(),
        members: vec!["alice".into()],
        avatar: String::new(),
        introduction: String::new(),
    });
    alice
        .send(marshal(&Packet::Logic(create)))
        .await
        .expect("create");
    let frame = timeout_read(&alice).await;
    let group_id = match read(&frame.payload).expect("create resp") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            p.read_body::<kim_protocol::pkt::GroupCreateResp>()
                .expect("body")
                .group_id
        }
        _ => panic!("expected create"),
    };

    let mut talk = LogicPkt::new(CMD_CHAT_GROUP_TALK, 3, Bytes::new());
    talk.set_dest(&group_id);
    talk.write_body(&MessageReq {
        r#type: MESSAGE_TYPE_TEXT,
        body: "hey".into(),
        extra: String::new(),
        client_id: String::new(),
    });
    alice
        .send(marshal(&Packet::Logic(talk)))
        .await
        .expect("gtalk");
    let frame = timeout_read(&alice).await;
    let message_id = match read(&frame.payload).expect("gtalk resp") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            p.read_body::<MessageResp>().expect("resp").message_id
        }
        _ => panic!("expected gtalk resp"),
    };

    alice
        .send(marshal(&Packet::Logic(read_pkt(
            4,
            &group_id,
            message_id,
            INBOX_KIND_GROUP,
        ))))
        .await
        .expect("group read");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("read resp") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected read resp"),
    }

    // Group receipts are disabled — peer must not see chat.receipt.read.
    timeout_no_packet(&bob, Duration::from_millis(400)).await;
    timeout_no_packet(&alice, Duration::from_millis(300)).await;

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
