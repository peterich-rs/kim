//! Chat store/directory over royal HTTP.

#![allow(clippy::unwrap_used)]
mod harness;

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use chat::idgen::SequenceIdGen;
use chat::royal::{http_backends, HttpAgentSpecStore};
use harness::*;
use kim_protocol::pkt::{
    AgentSpecRecord, AgentSpecSyncResp, AgentSpecUpsertReq, AgentSpecUpsertResp, Flag,
    GroupCreateReq, GroupCreateResp, GroupDetail, Status,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_AGENT_SPEC_SYNC, CMD_AGENT_SPEC_UPSERT, CMD_GROUP_CREATE,
    CMD_GROUP_DETAIL,
};
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

    let (store, groups, _, _) = http_backends(&format!("http://{addr}")).expect("http backends");
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

#[tokio::test]
async fn agent_spec_via_royal_http() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("royal bind");
    let addr = listener.local_addr().expect("royal addr");
    let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
    tokio::spawn(async move {
        let _ = serve(listener, state).await;
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    let base = format!("http://{addr}");

    let (store, groups, _, _) = http_backends(&base).expect("http backends");
    let specs = Arc::new(HttpAgentSpecStore::new(&base).expect("spec store"));
    let stack = spawn_stack_agent_specs(store, groups, specs).await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;

    let mut upsert = LogicPkt::new(CMD_AGENT_SPEC_UPSERT, 2, Bytes::new());
    upsert.write_body(&AgentSpecUpsertReq {
        record: Some(AgentSpecRecord {
            profile_id: "goose".into(),
            nickname: "助手".into(),
            server_account: String::new(),
            spec: b"\x0a\x01\x01".to_vec(),
            key_ciphertext: Vec::new(),
            updated_at: 100,
            deleted_at: 0,
        }),
        account: None,
    });
    alice.send(marshal(&Packet::Logic(upsert))).await.unwrap();
    let p = wait_resp(&alice, CMD_AGENT_SPEC_UPSERT).await;
    assert_eq!(p.header.status, Status::Success as i32);
    let resp: AgentSpecUpsertResp = p.read_body().unwrap();
    assert_eq!(resp.record.unwrap().profile_id, "goose");

    let older = LogicPkt::new(CMD_AGENT_SPEC_UPSERT, 3, Bytes::new());
    let mut older = older;
    older.write_body(&AgentSpecUpsertReq {
        record: Some(AgentSpecRecord {
            profile_id: "goose".into(),
            nickname: "助手".into(),
            server_account: String::new(),
            spec: b"old".to_vec(),
            key_ciphertext: Vec::new(),
            updated_at: 50,
            deleted_at: 0,
        }),
        account: None,
    });
    alice.send(marshal(&Packet::Logic(older))).await.unwrap();
    let p = wait_resp(&alice, CMD_AGENT_SPEC_UPSERT).await;
    let resp: AgentSpecUpsertResp = p.read_body().unwrap();
    assert_eq!(resp.record.unwrap().spec, b"\x0a\x01\x01");

    let sync = LogicPkt::new(CMD_AGENT_SPEC_SYNC, 4, Bytes::new());
    alice.send(marshal(&Packet::Logic(sync))).await.unwrap();
    let p = wait_resp(&alice, CMD_AGENT_SPEC_SYNC).await;
    let resp: AgentSpecSyncResp = p.read_body().unwrap();
    assert_eq!(resp.records.len(), 1);
    assert_eq!(resp.records[0].spec, b"\x0a\x01\x01");

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
