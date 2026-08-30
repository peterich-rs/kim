mod harness;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    Status, UserListResp, UserProfile, UserProfileUpdate, UserSearchReq, UserSearchResp,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_BLOCK_ADD, CMD_CHAT_USER_TALK, CMD_FRIEND_INCOMING,
    CMD_FRIEND_LIST, CMD_FRIEND_REMOVE, CMD_FRIEND_REQUEST, CMD_USER_PROFILE, CMD_USER_SEARCH,
    CMD_USER_UPDATE,
};

fn dest_pkt(command: &str, seq: u32, dest: &str) -> LogicPkt {
    let mut pkt = LogicPkt::new(command, seq, Bytes::new());
    pkt.set_dest(dest);
    pkt
}

#[tokio::test]
async fn profile_search_friends_and_talk_gate() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;

    let mut upd = LogicPkt::new(CMD_USER_UPDATE, 2, Bytes::new());
    upd.write_body(&UserProfileUpdate {
        nickname: "Ali".into(),
        avatar: String::new(),
        bio: "hi".into(),
    });
    alice
        .send(marshal(&Packet::Logic(upd)))
        .await
        .expect("update");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("upd") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let got: UserProfile = p.read_body().expect("profile");
            assert_eq!(got.nickname, "Ali");
        }
        _ => panic!("expected profile"),
    }

    let mut search = LogicPkt::new(CMD_USER_SEARCH, 3, Bytes::new());
    search.write_body(&UserSearchReq {
        query: "ali".into(),
    });
    bob.send(marshal(&Packet::Logic(search)))
        .await
        .expect("search");
    let frame = timeout_read(&bob).await;
    match read(&frame.payload).expect("search") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let resp: UserSearchResp = p.read_body().expect("search resp");
            assert!(resp.users.iter().any(|u| u.account == "alice"));
        }
        _ => panic!("expected search"),
    }

    let mut talk = dest_pkt(CMD_CHAT_USER_TALK, 4, "bob");
    talk.write_body(&kim_protocol::pkt::MessageReq {
        r#type: 1,
        body: "nope".into(),
        extra: String::new(),
        client_id: String::new(),
    });
    alice
        .send(marshal(&Packet::Logic(talk)))
        .await
        .expect("talk");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("talk") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::NotFriends as i32),
        _ => panic!("expected not friends"),
    }

    become_friends(&alice, &bob, "bob", "alice").await;

    let list = dest_pkt(CMD_FRIEND_LIST, 5, "");
    alice
        .send(marshal(&Packet::Logic(list)))
        .await
        .expect("list");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("list") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let resp: UserListResp = p.read_body().expect("friends");
            assert!(resp.users.iter().any(|u| u.account == "bob"));
        }
        _ => panic!("expected list"),
    }

    let incoming = dest_pkt(CMD_FRIEND_INCOMING, 6, "");
    alice
        .send(marshal(&Packet::Logic(incoming)))
        .await
        .expect("incoming");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("incoming") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let resp: UserListResp = p.read_body().expect("incoming");
            assert!(resp.users.is_empty());
        }
        _ => panic!("expected incoming"),
    }

    alice
        .send(marshal(&Packet::Logic(dest_pkt(
            CMD_FRIEND_REMOVE,
            7,
            "bob",
        ))))
        .await
        .expect("remove");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("remove") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected remove"),
    }

    alice
        .send(marshal(&Packet::Logic(dest_pkt(CMD_BLOCK_ADD, 8, "bob"))))
        .await
        .expect("block");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("block") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected block"),
    }
    bob.send(marshal(&Packet::Logic(dest_pkt(
        CMD_FRIEND_REQUEST,
        9,
        "alice",
    ))))
    .await
    .expect("req blocked");
    let frame = timeout_read(&bob).await;
    match read(&frame.payload).expect("blocked") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Blocked as i32),
        _ => panic!("expected blocked"),
    }

    let me = dest_pkt(CMD_USER_PROFILE, 10, "");
    alice.send(marshal(&Packet::Logic(me))).await.expect("me");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("me") {
        Packet::Logic(p) => {
            let got: UserProfile = p.read_body().expect("me");
            assert_eq!(got.account, "alice");
            assert_eq!(got.nickname, "Ali");
        }
        _ => panic!("expected me"),
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
