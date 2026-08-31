//! Private-group command authz over the login stack (Memory directory).

mod harness;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    GroupCreateReq, GroupCreateResp, GroupDetail, GroupJoinReq, GroupQuitReq, Status,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_GROUP_CREATE, CMD_GROUP_DETAIL, CMD_GROUP_JOIN,
    CMD_GROUP_MEMBERS, CMD_GROUP_QUIT,
};

#[tokio::test]
async fn create_is_private_join_disabled_quit_membership() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;
    let (carol, _) = login("carol", &url).await;

    let mut create = LogicPkt::new(CMD_GROUP_CREATE, 2, Bytes::new());
    create.write_body(&GroupCreateReq {
        name: "group1".into(),
        owner: "eve".into(),
        members: vec!["alice".into(), "bob".into()],
        avatar: String::new(),
        introduction: "hi".into(),
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

    let mut detail = LogicPkt::new(CMD_GROUP_DETAIL, 3, Bytes::new());
    detail.set_dest(&group_id);
    alice
        .send(marshal(&Packet::Logic(detail)))
        .await
        .expect("detail");
    let detail_frame = timeout_read(&alice).await;
    match read(&detail_frame.payload).expect("detail resp") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let d: GroupDetail = p.read_body().expect("GroupDetail");
            assert_eq!(d.owner, "alice");
            assert_eq!(d.members, vec!["alice".to_string()]);
        }
        _ => panic!("expected detail"),
    }

    let mut join = LogicPkt::new(CMD_GROUP_JOIN, 2, Bytes::new());
    join.set_dest(&group_id);
    join.write_body(&GroupJoinReq {
        account: "carol".into(),
        group_id: group_id.clone(),
    });
    carol
        .send(marshal(&Packet::Logic(join)))
        .await
        .expect("join");
    let join_frame = timeout_read(&carol).await;
    match read(&join_frame.payload).expect("join resp") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Unauthorized as i32),
        _ => panic!("expected join resp"),
    }

    let mut join_self = LogicPkt::new(CMD_GROUP_JOIN, 3, Bytes::new());
    join_self.set_dest(&group_id);
    join_self.write_body(&GroupJoinReq {
        account: String::new(),
        group_id: group_id.clone(),
    });
    bob.send(marshal(&Packet::Logic(join_self)))
        .await
        .expect("bob join");
    let bob_join = timeout_read(&bob).await;
    match read(&bob_join.payload).expect("bob join resp") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Unauthorized as i32),
        _ => panic!("expected bob join resp"),
    }

    let mut bob_detail = LogicPkt::new(CMD_GROUP_DETAIL, 4, Bytes::new());
    bob_detail.set_dest(&group_id);
    bob.send(marshal(&Packet::Logic(bob_detail)))
        .await
        .expect("bob detail");
    let bob_detail_frame = timeout_read(&bob).await;
    match read(&bob_detail_frame.payload).expect("bob detail") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::NotGroupMember as i32);
            assert!(p.body.is_empty());
        }
        _ => panic!("expected bob detail"),
    }

    let mut bob_members = LogicPkt::new(CMD_GROUP_MEMBERS, 5, Bytes::new());
    bob_members.set_dest(&group_id);
    bob.send(marshal(&Packet::Logic(bob_members)))
        .await
        .expect("bob members");
    let bob_members_frame = timeout_read(&bob).await;
    match read(&bob_members_frame.payload).expect("bob members") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::NotGroupMember as i32);
            assert!(p.body.is_empty());
        }
        _ => panic!("expected bob members"),
    }

    let mut bob_quit = LogicPkt::new(CMD_GROUP_QUIT, 6, Bytes::new());
    bob_quit.set_dest(&group_id);
    bob_quit.write_body(&GroupQuitReq {
        account: String::new(),
        group_id: group_id.clone(),
    });
    bob.send(marshal(&Packet::Logic(bob_quit)))
        .await
        .expect("bob quit");
    let bob_quit_frame = timeout_read(&bob).await;
    match read(&bob_quit_frame.payload).expect("bob quit") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::NotGroupMember as i32),
        _ => panic!("expected bob quit"),
    }

    let mut unknown_quit = LogicPkt::new(CMD_GROUP_QUIT, 7, Bytes::new());
    unknown_quit.set_dest("NOPE");
    unknown_quit.write_body(&GroupQuitReq {
        account: String::new(),
        group_id: "NOPE".into(),
    });
    alice
        .send(marshal(&Packet::Logic(unknown_quit)))
        .await
        .expect("unknown quit");
    let unknown_frame = timeout_read(&alice).await;
    match read(&unknown_frame.payload).expect("unknown quit") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::NotGroupMember as i32),
        _ => panic!("expected unknown quit"),
    }

    let mut quit = LogicPkt::new(CMD_GROUP_QUIT, 8, Bytes::new());
    quit.set_dest(&group_id);
    quit.write_body(&GroupQuitReq {
        account: String::new(),
        group_id: group_id.clone(),
    });
    alice
        .send(marshal(&Packet::Logic(quit)))
        .await
        .expect("quit");
    let quit_frame = timeout_read(&alice).await;
    match read(&quit_frame.payload).expect("quit resp") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected quit resp"),
    }

    let mut after = LogicPkt::new(CMD_GROUP_DETAIL, 9, Bytes::new());
    after.set_dest(&group_id);
    alice
        .send(marshal(&Packet::Logic(after)))
        .await
        .expect("after detail");
    let after_frame = timeout_read(&alice).await;
    match read(&after_frame.payload).expect("after detail") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::NotGroupMember as i32),
        _ => panic!("expected after detail"),
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
