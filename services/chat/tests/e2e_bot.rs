//! Bot identity e2e: create, talk gates, reply, pending, search.

mod harness;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    BotCreateReq, BotCreateResp, BotReplyReq, Flag, InboxReq, MessagePush, MessageReq, MessageResp,
    Status, UserListResp, UserSearchReq, UserSearchResp,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_BOT_CREATE, CMD_BOT_PENDING, CMD_BOT_REPLY,
    CMD_CHAT_USER_TALK, CMD_FRIEND_LIST, CMD_FRIEND_REMOVE, CMD_FRIEND_REQUEST, CMD_USER_SEARCH,
    MESSAGE_TYPE_TEXT, PROFILE_KIND_BOT,
};

fn dest_pkt(command: &str, seq: u32, dest: &str) -> LogicPkt {
    let mut pkt = LogicPkt::new(command, seq, Bytes::new());
    pkt.set_dest(dest);
    pkt
}

fn talk_pkt(seq: u32, dest: &str, body: &str) -> LogicPkt {
    let mut pkt = dest_pkt(CMD_CHAT_USER_TALK, seq, dest);
    pkt.write_body(&MessageReq {
        r#type: MESSAGE_TYPE_TEXT,
        body: body.to_string(),
        extra: String::new(),
        client_id: String::new(),
    });
    pkt
}

fn status_of(frame: &kim_core::Frame) -> i32 {
    match read(&frame.payload).expect("pkt") {
        Packet::Logic(p) => p.header.status,
        _ => panic!("expected logic"),
    }
}

async fn read_logic(client: &kim_ws::WsClient) -> LogicPkt {
    match read(&timeout_read(client).await.payload).expect("decode") {
        Packet::Logic(p) => p,
        _ => panic!("expected logic"),
    }
}

async fn wait_resp(client: &kim_ws::WsClient, command: &str) -> LogicPkt {
    loop {
        let p = read_logic(client).await;
        if p.header.flag == Flag::Response as i32 && p.header.command == command {
            return p;
        }
    }
}

async fn wait_push(client: &kim_ws::WsClient, command: &str) -> LogicPkt {
    loop {
        let p = read_logic(client).await;
        if p.header.flag == Flag::Push as i32 && p.header.command == command {
            return p;
        }
    }
}

#[tokio::test]
async fn bot_create_talk_reply_and_gates() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login_with_device("alice", &url, "web").await;
    let (alice2, _) = login_with_device("alice", &url, "web").await;
    let (bob, _) = login("bob", &url).await;

    let mut create = LogicPkt::new(CMD_BOT_CREATE, 2, Bytes::new());
    create.write_body(&BotCreateReq {
        client_profile_id: "goose".into(),
        nickname: "助手".into(),
        avatar: String::new(),
        bio: String::new(),
    });
    alice
        .send(marshal(&Packet::Logic(create)))
        .await
        .expect("create");
    let p = wait_resp(&alice, CMD_BOT_CREATE).await;
    assert_eq!(p.header.status, Status::Success as i32);
    let resp: BotCreateResp = p.read_body().expect("BotCreateResp");
    let bot = resp.profile.expect("profile");
    assert_eq!(bot.kind, PROFILE_KIND_BOT);
    assert!(bot.account.starts_with("b_"));
    let bot_acc = bot.account.clone();

    let list = dest_pkt(CMD_FRIEND_LIST, 3, "");
    alice
        .send(marshal(&Packet::Logic(list)))
        .await
        .expect("list");
    let p = wait_resp(&alice, CMD_FRIEND_LIST).await;
    assert_eq!(p.header.status, Status::Success as i32);
    let friends: UserListResp = p.read_body().expect("friends");
    assert!(friends
        .users
        .iter()
        .any(|u| u.account == bot_acc && u.kind == PROFILE_KIND_BOT));

    alice
        .send(marshal(&Packet::Logic(talk_pkt(4, &bot_acc, "hello bot"))))
        .await
        .expect("talk");
    let p = wait_resp(&alice, CMD_CHAT_USER_TALK).await;
    assert_eq!(p.header.status, Status::Success as i32);
    let m1: MessageResp = p.read_body().expect("talk resp");
    assert!(m1.message_id > 0);

    let echo = wait_push(&alice2, CMD_CHAT_USER_TALK).await;
    assert_eq!(echo.header.dest, bot_acc);
    let echo_push: MessagePush = echo.read_body().expect("echo");
    assert_eq!(echo_push.sender, "alice");
    assert_eq!(echo_push.body, "hello bot");

    bob.send(marshal(&Packet::Logic(talk_pkt(5, &bot_acc, "nope"))))
        .await
        .expect("bob talk");
    assert_eq!(
        status_of(&timeout_read(&bob).await),
        Status::UserNotFound as i32
    );

    bob.send(marshal(&Packet::Logic(dest_pkt(
        CMD_FRIEND_REQUEST,
        6,
        &bot_acc,
    ))))
    .await
    .expect("bob request");
    assert_eq!(
        status_of(&timeout_read(&bob).await),
        Status::UserNotFound as i32
    );

    alice
        .send(marshal(&Packet::Logic(dest_pkt(
            CMD_FRIEND_REMOVE,
            7,
            &bot_acc,
        ))))
        .await
        .expect("alice remove");
    assert_eq!(
        status_of(&timeout_read(&alice).await),
        Status::BotSocialDenied as i32
    );

    let mut reply = dest_pkt(CMD_BOT_REPLY, 8, &bot_acc);
    reply.write_body(&BotReplyReq {
        message: Some(MessageReq {
            r#type: MESSAGE_TYPE_TEXT,
            body: "hi alice".into(),
            extra: String::new(),
            client_id: "r1".into(),
        }),
        in_reply_to: m1.message_id,
    });
    alice
        .send(marshal(&Packet::Logic(reply)))
        .await
        .expect("reply");
    let p = wait_resp(&alice, CMD_BOT_REPLY).await;
    assert_eq!(p.header.status, Status::Success as i32);
    let m2: MessageResp = p.read_body().expect("reply resp");
    assert!(m2.message_id > 0);

    let push = wait_push(&alice2, CMD_CHAT_USER_TALK).await;
    let bot_push: MessagePush = push.read_body().expect("bot push");
    assert_eq!(bot_push.sender, bot_acc);
    assert_eq!(bot_push.body, "hi alice");

    let mut reply2 = dest_pkt(CMD_BOT_REPLY, 9, &bot_acc);
    reply2.write_body(&BotReplyReq {
        message: Some(MessageReq {
            r#type: MESSAGE_TYPE_TEXT,
            body: "other".into(),
            extra: String::new(),
            client_id: "r2".into(),
        }),
        in_reply_to: m1.message_id,
    });
    alice
        .send(marshal(&Packet::Logic(reply2)))
        .await
        .expect("dup reply");
    let p = wait_resp(&alice, CMD_BOT_REPLY).await;
    assert_eq!(p.header.status, Status::Success as i32);
    let dup: MessageResp = p.read_body().expect("dup resp");
    assert_eq!(dup.message_id, m2.message_id);

    let mut bad = dest_pkt(CMD_BOT_REPLY, 10, &bot_acc);
    bad.write_body(&BotReplyReq {
        message: Some(MessageReq {
            r#type: MESSAGE_TYPE_TEXT,
            body: "x".into(),
            extra: String::new(),
            client_id: "r3".into(),
        }),
        in_reply_to: 0,
    });
    alice.send(marshal(&Packet::Logic(bad))).await.expect("bad");
    assert_eq!(
        status_of(&timeout_read(&alice).await),
        Status::InvalidPacketBody as i32
    );

    let mut bob_reply = dest_pkt(CMD_BOT_REPLY, 11, &bot_acc);
    bob_reply.write_body(&BotReplyReq {
        message: Some(MessageReq {
            r#type: MESSAGE_TYPE_TEXT,
            body: "steal".into(),
            extra: String::new(),
            client_id: "r4".into(),
        }),
        in_reply_to: m1.message_id,
    });
    bob.send(marshal(&Packet::Logic(bob_reply)))
        .await
        .expect("bob reply");
    assert_eq!(
        status_of(&timeout_read(&bob).await),
        Status::NotBotOwner as i32
    );

    let mut search = LogicPkt::new(CMD_USER_SEARCH, 12, Bytes::new());
    search.write_body(&UserSearchReq {
        query: "助手".into(),
    });
    bob.send(marshal(&Packet::Logic(search)))
        .await
        .expect("search");
    let p = wait_resp(&bob, CMD_USER_SEARCH).await;
    assert_eq!(p.header.status, Status::Success as i32);
    let hits: UserSearchResp = p.read_body().expect("search");
    assert!(hits.users.iter().all(|u| u.account != bot_acc));

    let mut pending = dest_pkt(CMD_BOT_PENDING, 13, &bot_acc);
    pending.write_body(&InboxReq { limit: 20 });
    alice
        .send(marshal(&Packet::Logic(pending)))
        .await
        .expect("pending");
    let p = wait_resp(&alice, CMD_BOT_PENDING).await;
    assert_eq!(p.header.status, Status::Success as i32);
}
