//! Chat store/directory over royal HTTP.

mod harness;

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use chat::idgen::SequenceIdGen;
use chat::royal::http_backends;
use harness::*;
use kim_protocol::pkt::{GroupCreateReq, GroupCreateResp, GroupDetail, Status};
use kim_protocol::{marshal, read, LogicPkt, Packet, CMD_GROUP_CREATE, CMD_GROUP_DETAIL};
use royal::{serve, RoyalState};

#[tokio::test]
async fn create_and_detail_via_royal_http() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("royal bind");
    let addr = listener.local_addr().expect("royal addr");
    let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
    tokio::spawn(async move {
        let _ = serve(listener, state).await;
    });
    tokio::time::sleep(Duration::from_millis(30)).await;

    let (store, groups) = http_backends(&format!("http://{addr}")).expect("http backends");
    let stack = spawn_stack_seams(store, groups).await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;

    let mut create = LogicPkt::new(CMD_GROUP_CREATE, 2, Bytes::new());
    create.write_body(&GroupCreateReq {
        name: "royal-g".into(),
        owner: "alice".into(),
        members: vec!["alice".into(), "bob".into()],
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
            assert_eq!(p.header.status, Status::Success as i32, "create failed");
            p.read_body::<GroupCreateResp>()
                .expect("GroupCreateResp")
                .group_id
        }
        _ => panic!("expected create"),
    };
    assert!(!group_id.is_empty());

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
            assert_eq!(d.name, "royal-g");
            assert!(d.members.contains(&"alice".to_string()));
        }
        _ => panic!("expected detail"),
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
