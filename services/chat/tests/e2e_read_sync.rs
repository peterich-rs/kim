//! Self-account inbox read sync across two devices.

#![allow(clippy::unwrap_used)]
mod harness;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    ConversationReadReq, ConversationReadState, ConversationReadSyncPush, Flag, MessageReq,
    MessageResp, Status,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_CHAT_USER_TALK, CMD_INBOX_READ, CMD_INBOX_READ_SYNC,
    INBOX_KIND_USER, MESSAGE_TYPE_TEXT,
};

#[tokio::test]
async fn inbox_read_notifies_other_device_of_same_account() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice_phone, _) = login("alice", &url).await;
    let (alice_desktop, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;
    become_friends(&alice_phone, &bob, "bob", "alice").await;

    let mut talk = LogicPkt::new(CMD_CHAT_USER_TALK, 2, Bytes::new());
    talk.set_dest("alice");
    talk.write_body(&MessageReq {
        r#type: MESSAGE_TYPE_TEXT,
        body: "hello".into(),
        extra: String::new(),
        client_id: String::new(),
    });
    bob.send(marshal(&Packet::Logic(talk))).await.expect("talk");
    let frame = timeout_read(&bob).await;
    let message_id = match read(&frame.payload).expect("talk resp") {
        Packet::Logic(p) => {
            let resp: MessageResp = p.read_body().expect("talk body");
            resp.message_id
        }
        _ => panic!("expected talk resp"),
    };
    let _ = timeout_read(&alice_phone).await;
    let _ = timeout_read(&alice_desktop).await;

    let mut read_pkt = LogicPkt::new(CMD_INBOX_READ, 5, Bytes::new());
    read_pkt.set_dest("bob");
    read_pkt.write_body(&ConversationReadReq {
        message_id,
        kind: INBOX_KIND_USER,
    });
    alice_phone
        .send(marshal(&Packet::Logic(read_pkt)))
        .await
        .expect("read");
    let frame = timeout_read(&alice_phone).await;
    match read(&frame.payload).expect("read resp") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            if !p.body.is_empty() {
                let state: ConversationReadState = p.read_body().expect("read state");
                assert_eq!(state.unread, 0);
                assert!(state.last_read_message_id >= message_id);
            }
        }
        _ => panic!("expected read resp"),
    }

    let push = loop {
        let frame = timeout_read(&alice_desktop).await;
        match read(&frame.payload).expect("sync push") {
            Packet::Logic(p) if p.header.command == CMD_INBOX_READ_SYNC => break p,
            Packet::Logic(_) => continue,
            _ => panic!("expected logic"),
        }
    };
    assert_eq!(push.header.flag, Flag::Push as i32);
    let body: ConversationReadSyncPush = push.read_body().expect("sync body");
    assert_eq!(body.account, "alice");
    let state = body.state.expect("state");
    assert_eq!(state.dest, "bob");
    assert_eq!(state.unread, 0);

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
