//! Owner-only AgentSpec sync/upsert LWW e2e.

#![allow(clippy::unwrap_used)]
mod harness;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    AgentSpecRecord, AgentSpecSyncResp, AgentSpecUpsertReq, AgentSpecUpsertResp, Flag, Status,
};
use kim_protocol::{marshal, read, LogicPkt, Packet, CMD_AGENT_SPEC_SYNC, CMD_AGENT_SPEC_UPSERT};

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

fn spec_record(id: &str, spec: &[u8], updated_at: i64) -> AgentSpecRecord {
    AgentSpecRecord {
        profile_id: id.into(),
        nickname: "助手".into(),
        server_account: String::new(),
        spec: spec.to_vec(),
        key_ciphertext: Vec::new(),
        updated_at,
        deleted_at: 0,
    }
}

#[tokio::test]
async fn agent_spec_upsert_sync_lww_and_owner_gate() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;

    let mut upsert = LogicPkt::new(CMD_AGENT_SPEC_UPSERT, 2, Bytes::new());
    upsert.write_body(&AgentSpecUpsertReq {
        record: Some(spec_record("goose", b"\x0a\x01\x01", 100)),
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
        record: Some(spec_record("goose", b"old", 50)),
        account: None,
    });
    alice.send(marshal(&Packet::Logic(older))).await.unwrap();
    let p = wait_resp(&alice, CMD_AGENT_SPEC_UPSERT).await;
    assert_eq!(p.header.status, Status::Success as i32);
    let resp: AgentSpecUpsertResp = p.read_body().unwrap();
    assert_eq!(resp.record.unwrap().spec, b"\x0a\x01\x01");

    let equal = LogicPkt::new(CMD_AGENT_SPEC_UPSERT, 4, Bytes::new());
    let mut equal = equal;
    equal.write_body(&AgentSpecUpsertReq {
        record: Some(spec_record("goose", b"eq", 100)),
        account: None,
    });
    alice.send(marshal(&Packet::Logic(equal))).await.unwrap();
    let p = wait_resp(&alice, CMD_AGENT_SPEC_UPSERT).await;
    let resp: AgentSpecUpsertResp = p.read_body().unwrap();
    assert_eq!(resp.record.unwrap().spec, b"eq");

    let tomb = LogicPkt::new(CMD_AGENT_SPEC_UPSERT, 5, Bytes::new());
    let mut tomb = tomb;
    let mut rec = spec_record("goose", b"eq", 200);
    rec.deleted_at = 200;
    tomb.write_body(&AgentSpecUpsertReq {
        record: Some(rec),
        account: None,
    });
    alice.send(marshal(&Packet::Logic(tomb))).await.unwrap();
    let _ = wait_resp(&alice, CMD_AGENT_SPEC_UPSERT).await;

    let sync = LogicPkt::new(CMD_AGENT_SPEC_SYNC, 6, Bytes::new());
    alice.send(marshal(&Packet::Logic(sync))).await.unwrap();
    let p = wait_resp(&alice, CMD_AGENT_SPEC_SYNC).await;
    let resp: AgentSpecSyncResp = p.read_body().unwrap();
    assert_eq!(resp.records.len(), 1);
    assert_eq!(resp.records[0].deleted_at, 200);

    let mut steal = LogicPkt::new(CMD_AGENT_SPEC_UPSERT, 7, Bytes::new());
    steal.set_dest("alice");
    steal.write_body(&AgentSpecUpsertReq {
        record: Some(spec_record("goose", b"hack", 300)),
        account: None,
    });
    bob.send(marshal(&Packet::Logic(steal))).await.unwrap();
    let p = wait_resp(&bob, CMD_AGENT_SPEC_UPSERT).await;
    assert_eq!(p.header.status, Status::NotBotOwner as i32);

    let empty = LogicPkt::new(CMD_AGENT_SPEC_UPSERT, 8, Bytes::new());
    let mut empty = empty;
    empty.write_body(&AgentSpecUpsertReq {
        record: Some(spec_record("goose", b"", 400)),
        account: None,
    });
    alice.send(marshal(&Packet::Logic(empty))).await.unwrap();
    let p = wait_resp(&alice, CMD_AGENT_SPEC_UPSERT).await;
    assert_eq!(p.header.status, Status::InvalidPacketBody as i32);

    let mut acct = LogicPkt::new(CMD_AGENT_SPEC_UPSERT, 9, Bytes::new());
    acct.write_body(&AgentSpecUpsertReq {
        record: None,
        account: Some(kim_protocol::pkt::AgentProviderAccount {
            id: "acct-goose".into(),
            vendor_id: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            key_ref: "agent.api_key.acct.acct-goose".into(),
            display_name: "openai".into(),
            models: vec!["gpt-4o".into()],
            updated_at: 10,
            deleted_at: 0,
        }),
    });
    alice.send(marshal(&Packet::Logic(acct))).await.unwrap();
    let p = wait_resp(&alice, CMD_AGENT_SPEC_UPSERT).await;
    assert_eq!(p.header.status, Status::Success as i32);
    let sync2 = LogicPkt::new(CMD_AGENT_SPEC_SYNC, 10, Bytes::new());
    alice.send(marshal(&Packet::Logic(sync2))).await.unwrap();
    let p = wait_resp(&alice, CMD_AGENT_SPEC_SYNC).await;
    let resp: AgentSpecSyncResp = p.read_body().unwrap();
    assert_eq!(resp.accounts.len(), 1);
    assert_eq!(resp.accounts[0].id, "acct-goose");
    assert!(resp.accounts[0].key_ref.contains("acct-goose"));
}
