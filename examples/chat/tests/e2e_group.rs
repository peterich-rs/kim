//! Group join / detail / notify / members over the login stack (Memory directory).

mod harness;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    Flag, GroupCreateNotify, GroupCreateReq, GroupCreateResp, GroupDetail, GroupJoinReq,
    GroupMembersResp, GroupQuitReq, Status,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_GROUP_CREATE, CMD_GROUP_DETAIL, CMD_GROUP_JOIN,
    CMD_GROUP_MEMBERS, CMD_GROUP_QUIT,
};

#[tokio::test]
async fn create_join_detail_quit_and_notify() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;

    let mut create = LogicPkt::new(CMD_GROUP_CREATE, 2, Bytes::new());
    create.write_body(&GroupCreateReq {
        name: "group1".into(),
        owner: "alice".into(),
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

    let notify_frame = timeout_read(&bob).await;
    match read(&notify_frame.payload).expect("notify") {
        Packet::Logic(p) => {
            assert_eq!(p.header.command, CMD_GROUP_CREATE);
            assert_eq!(p.header.flag, Flag::Push as i32);
            let n: GroupCreateNotify = p.read_body().expect("notify body");
            assert_eq!(n.group_id, group_id);
            assert!(n.members.contains(&"bob".to_string()));
        }
        _ => panic!("expected notify"),
    }

    let (carol, _) = login("carol", &url).await;
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
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected join resp"),
    }

    let mut detail = LogicPkt::new(CMD_GROUP_DETAIL, 3, Bytes::new());
    detail.set_dest(&group_id);
    carol
        .send(marshal(&Packet::Logic(detail)))
        .await
        .expect("detail");
    let detail_frame = timeout_read(&carol).await;
    match read(&detail_frame.payload).expect("detail resp") {
        Packet::Logic(p) => {
            assert_eq!(p.header.status, Status::Success as i32);
            let d: GroupDetail = p.read_body().expect("GroupDetail");
            assert_eq!(d.group_id, group_id);
            assert_eq!(d.name, "group1");
            assert!(d.members.contains(&"carol".to_string()));
        }
        _ => panic!("expected detail"),
    }

    let mut members = LogicPkt::new(CMD_GROUP_MEMBERS, 4, Bytes::new());
    members.set_dest(&group_id);
    carol
        .send(marshal(&Packet::Logic(members)))
        .await
        .expect("members");
    let members_frame = timeout_read(&carol).await;
    match read(&members_frame.payload).expect("members resp") {
        Packet::Logic(p) => {
            let m: GroupMembersResp = p.read_body().expect("members");
            assert!(m.members.contains(&"carol".to_string()));
        }
        _ => panic!("expected members"),
    }

    let mut quit = LogicPkt::new(CMD_GROUP_QUIT, 5, Bytes::new());
    quit.set_dest(&group_id);
    quit.write_body(&GroupQuitReq {
        account: "carol".into(),
        group_id: group_id.clone(),
    });
    carol
        .send(marshal(&Packet::Logic(quit)))
        .await
        .expect("quit");
    let quit_frame = timeout_read(&carol).await;
    match read(&quit_frame.payload).expect("quit resp") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected quit"),
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
