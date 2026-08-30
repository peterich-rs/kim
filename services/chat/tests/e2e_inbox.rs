mod harness;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    ConversationReadReq, HistoryReq, HistoryResp, InboxReq, InboxResp, MessageReq, Status,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_CHAT_USER_TALK, CMD_HISTORY, CMD_INBOX_LIST,
    CMD_INBOX_READ, INBOX_KIND_USER, MESSAGE_TYPE_TEXT,
};

fn talk(seq: u32, dest: &str, body: &str) -> LogicPkt {
    let mut pkt = LogicPkt::new(CMD_CHAT_USER_TALK, seq, Bytes::new());
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
async fn inbox_history_and_read_cursor() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    alice
        .send(marshal(&Packet::Logic(talk(2, "bob", "hello"))))
        .await
        .expect("talk");
    let frame = timeout_read(&alice).await;
    let message_id = match read(&frame.payload).expect("talk resp") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            p.read_body::<kim_protocol::pkt::MessageResp>()
                .expect("resp")
                .message_id
        }
        _ => panic!("expected talk resp"),
    };
    let _ = timeout_read(&bob).await;

    let mut inbox = LogicPkt::new(CMD_INBOX_LIST, 3, Bytes::new());
    inbox.write_body(&InboxReq { limit: 20 });
    bob.send(marshal(&Packet::Logic(inbox)))
        .await
        .expect("inbox");
    let frame = timeout_read(&bob).await;
    match read(&frame.payload).expect("inbox") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let resp: InboxResp = p.read_body().expect("inbox");
            assert_eq!(resp.items.len(), 1);
            assert_eq!(resp.items[0].dest, "alice");
            assert_eq!(resp.items[0].unread, 1);
            assert_eq!(resp.items[0].last_body, "hello");
        }
        _ => panic!("expected inbox"),
    }

    let mut hist = LogicPkt::new(CMD_HISTORY, 4, Bytes::new());
    hist.set_dest("alice");
    hist.write_body(&HistoryReq {
        before_id: 0,
        limit: 20,
        kind: INBOX_KIND_USER,
    });
    bob.send(marshal(&Packet::Logic(hist))).await.expect("hist");
    let frame = timeout_read(&bob).await;
    match read(&frame.payload).expect("hist") {
        Packet::Logic(p) => {
            let resp: HistoryResp = p.read_body().expect("hist");
            assert_eq!(resp.messages.len(), 1);
            assert_eq!(resp.messages[0].body, "hello");
            assert_eq!(resp.messages[0].sender, "alice");
        }
        _ => panic!("expected hist"),
    }

    let mut read_pkt = LogicPkt::new(CMD_INBOX_READ, 5, Bytes::new());
    read_pkt.set_dest("alice");
    read_pkt.write_body(&ConversationReadReq {
        message_id,
        kind: INBOX_KIND_USER,
    });
    bob.send(marshal(&Packet::Logic(read_pkt)))
        .await
        .expect("read");
    let frame = timeout_read(&bob).await;
    match read(&frame.payload).expect("read") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected read"),
    }

    let mut inbox = LogicPkt::new(CMD_INBOX_LIST, 6, Bytes::new());
    inbox.write_body(&InboxReq { limit: 20 });
    bob.send(marshal(&Packet::Logic(inbox)))
        .await
        .expect("inbox2");
    let frame = timeout_read(&bob).await;
    match read(&frame.payload).expect("inbox2") {
        Packet::Logic(p) => {
            let resp: InboxResp = p.read_body().expect("inbox2");
            assert_eq!(resp.items[0].unread, 0);
        }
        _ => panic!("expected inbox2"),
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
