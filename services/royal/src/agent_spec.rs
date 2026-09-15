//! HMAC AgentSpec blob store. Chat does not parse `spec` bytes.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use chat::{account_from_pb, account_to_pb, record_from_pb, record_to_pb, AgentSpecError};
use kim_protocol::pkt::{
    AgentSpecSyncResp, AgentSpecUpsertResp, InternalAgentSpecQuery, InternalAgentSpecUpsert,
};

use crate::{backend, decode, encode, RoyalState};

fn spec_http(err: AgentSpecError) -> (StatusCode, String) {
    match err {
        AgentSpecError::Invalid => (StatusCode::BAD_REQUEST, "invalid".into()),
        AgentSpecError::NotOwner => (StatusCode::FORBIDDEN, "not bot owner".into()),
        AgentSpecError::Backend(e) => backend(e),
    }
}

fn require_owner(owner: &str) -> Result<(), (StatusCode, String)> {
    if owner.is_empty() {
        Err((StatusCode::BAD_REQUEST, "empty owner".into()))
    } else {
        Ok(())
    }
}

pub async fn spec_sync(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InternalAgentSpecQuery>(&body)?;
    require_owner(&req.owner)?;
    let records = st
        .agent_specs
        .list(&st.app, &req.owner)
        .await
        .map_err(spec_http)?;
    let accounts = st
        .agent_specs
        .list_accounts(&st.app, &req.owner)
        .await
        .map_err(spec_http)?;
    Ok(encode(&AgentSpecSyncResp {
        records: records.iter().map(record_to_pb).collect(),
        accounts: accounts.iter().map(account_to_pb).collect(),
    }))
}

pub async fn spec_upsert(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InternalAgentSpecUpsert>(&body)?;
    require_owner(&req.owner)?;
    if req.record.is_none() && req.account.is_none() {
        return Err((StatusCode::BAD_REQUEST, "empty upsert".into()));
    }
    if let Some(acc) = req.account {
        st.agent_specs
            .upsert_account(&st.app, &req.owner, account_from_pb(acc))
            .await
            .map_err(spec_http)?;
    }
    let Some(pb) = req.record else {
        return Ok(encode(&AgentSpecUpsertResp { record: None }));
    };
    let stored = st
        .agent_specs
        .upsert(&st.app, &req.owner, record_from_pb(pb))
        .await
        .map_err(spec_http)?;
    Ok(encode(&AgentSpecUpsertResp {
        record: Some(record_to_pb(&stored)),
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Arc;
    use std::time::Duration;

    use chat::idgen::SequenceIdGen;
    use chat::{AgentSpecRecord, AgentSpecStore, HttpAgentSpecStore};

    use super::*;
    use crate::{serve, RoyalState};

    #[tokio::test]
    async fn http_store_lww_and_empty_owner() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let store = HttpAgentSpecStore::new(&format!("http://{addr}")).unwrap();

        let rec = AgentSpecRecord {
            profile_id: "goose".into(),
            nickname: "助手".into(),
            spec: b"new".to_vec(),
            updated_at: 100,
            ..AgentSpecRecord::default()
        };
        store.upsert("kim", "alice", rec).await.unwrap();
        let older = AgentSpecRecord {
            profile_id: "goose".into(),
            nickname: "助手".into(),
            spec: b"old".to_vec(),
            updated_at: 50,
            ..AgentSpecRecord::default()
        };
        let kept = store.upsert("kim", "alice", older).await.unwrap();
        assert_eq!(kept.spec, b"new");
        let rows = store.list("kim", "alice").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].spec, b"new");
        assert!(store.list("kim", "bob").await.unwrap().is_empty());

        let err = store
            .upsert(
                "kim",
                "",
                AgentSpecRecord {
                    profile_id: "x".into(),
                    spec: b"z".to_vec(),
                    updated_at: 1,
                    ..AgentSpecRecord::default()
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AgentSpecError::Invalid));
    }
}
