//! In-process Royal: protobuf HTTP over axum. Chat talks to this via `Http*` adapters.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use fake_chat::directory::{CreateGroup, GroupDirectory, MemoryGroupDirectory};
use fake_chat::idgen::{IdGenerator, SequenceIdGen, SnowflakeGen};
use fake_chat::store::{InsertMessage, MemoryMessageStore, MessageStore};
use kim_protocol::pkt::{
    AckMessageReq, GroupCreateReq, GroupCreateResp, GroupDetail, GroupJoinReq, GroupMembersResp,
    GroupQuitReq, InsertMessageReq, InsertMessageResp, MessageContentReq, MessageContentResp,
    MessageIndex, MessageIndexResp, OfflineIndexReq,
};
use prost::Message;

#[derive(Clone)]
pub struct RoyalState {
    store: Arc<dyn MessageStore>,
    groups: Arc<dyn GroupDirectory>,
}

impl RoyalState {
    pub fn memory(idgen: Arc<dyn IdGenerator>) -> Self {
        Self {
            store: Arc::new(MemoryMessageStore::new(idgen.clone())),
            groups: Arc::new(MemoryGroupDirectory::new(idgen)),
        }
    }

    pub fn with_snowflake(node: u16) -> Self {
        let idgen: Arc<dyn IdGenerator> = match SnowflakeGen::try_new(node) {
            Ok(g) => Arc::new(g),
            Err(err) => {
                tracing::error!(%err, node, "snowflake init failed; using SequenceIdGen");
                Arc::new(SequenceIdGen::new(10_001))
            }
        };
        Self::memory(idgen)
    }
}

pub fn router(state: RoyalState) -> Router {
    Router::new()
        .route("/api/{app}/message/user", post(insert_user))
        .route("/api/{app}/message/group", post(insert_group))
        .route("/api/{app}/message/ack", post(ack))
        .route("/api/{app}/offline/index", post(offline_index))
        .route("/api/{app}/offline/content", post(offline_content))
        .route("/api/{app}/group", post(group_create))
        .route(
            "/api/{app}/group/member",
            post(group_join).delete(group_quit),
        )
        .route("/api/{app}/group/members/{group}", get(group_members))
        .route("/api/{app}/group/{group}", get(group_detail))
        .with_state(state)
}

fn decode<T: Message + Default>(body: &Bytes) -> Result<T, (StatusCode, String)> {
    T::decode(body.as_ref()).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

fn encode(msg: &impl Message) -> Bytes {
    Bytes::from(msg.encode_to_vec())
}

fn backend(err: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

async fn insert_user(
    State(st): State<RoyalState>,
    Path(app): Path<String>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InsertMessageReq>(&body)?;
    let msg = req.message.unwrap_or_default();
    let inserted = st
        .store
        .insert_user(
            &app,
            &InsertMessage {
                sender: req.sender,
                dest: req.dest,
                send_time: req.send_time,
                msg_type: msg.r#type,
                body: msg.body,
                extra: msg.extra,
            },
        )
        .await
        .map_err(backend)?;
    Ok(encode(&InsertMessageResp {
        message_id: inserted.message_id,
    }))
}

async fn insert_group(
    State(st): State<RoyalState>,
    Path(app): Path<String>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InsertMessageReq>(&body)?;
    let msg = req.message.unwrap_or_default();
    let members = if req.members.is_empty() {
        st.groups.members(&app, &req.dest).await.map_err(backend)?
    } else {
        req.members
    };
    let inserted = st
        .store
        .insert_group(
            &app,
            &InsertMessage {
                sender: req.sender,
                dest: req.dest,
                send_time: req.send_time,
                msg_type: msg.r#type,
                body: msg.body,
                extra: msg.extra,
            },
            &members,
        )
        .await
        .map_err(backend)?;
    Ok(encode(&InsertMessageResp {
        message_id: inserted.message_id,
    }))
}

async fn ack(
    State(st): State<RoyalState>,
    Path(app): Path<String>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AckMessageReq>(&body)?;
    st.store
        .ack(&app, &req.account, req.message_id)
        .await
        .map_err(backend)?;
    Ok(Bytes::new())
}

async fn offline_index(
    State(st): State<RoyalState>,
    Path(app): Path<String>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<OfflineIndexReq>(&body)?;
    let rows = st
        .store
        .offline_index(&app, &req.account, req.message_id)
        .await
        .map_err(backend)?;
    let resp = MessageIndexResp {
        indexes: rows
            .into_iter()
            .map(|r| MessageIndex {
                message_id: r.message_id,
                direction: r.direction,
                send_time: r.send_time,
                account_b: r.account_b,
                group: r.group,
            })
            .collect(),
    };
    Ok(encode(&resp))
}

async fn offline_content(
    State(st): State<RoyalState>,
    Path(app): Path<String>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<MessageContentReq>(&body)?;
    let rows = st
        .store
        .offline_content(&app, &req.message_ids)
        .await
        .map_err(backend)?;
    let resp = MessageContentResp {
        messages: rows
            .into_iter()
            .map(|r| kim_protocol::pkt::Message {
                message_id: r.message_id,
                r#type: r.msg_type,
                body: r.body,
                extra: r.extra,
            })
            .collect(),
    };
    Ok(encode(&resp))
}

async fn group_create(
    State(st): State<RoyalState>,
    Path(app): Path<String>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<GroupCreateReq>(&body)?;
    let group_id = st
        .groups
        .create(
            &app,
            &CreateGroup {
                name: req.name,
                avatar: req.avatar,
                introduction: req.introduction,
                owner: req.owner,
                members: req.members,
            },
        )
        .await
        .map_err(backend)?;
    Ok(encode(&GroupCreateResp { group_id }))
}

async fn group_join(
    State(st): State<RoyalState>,
    Path(app): Path<String>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<GroupJoinReq>(&body)?;
    st.groups
        .join(&app, &req.group_id, &req.account)
        .await
        .map_err(backend)?;
    Ok(Bytes::new())
}

async fn group_quit(
    State(st): State<RoyalState>,
    Path(app): Path<String>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<GroupQuitReq>(&body)?;
    st.groups
        .quit(&app, &req.group_id, &req.account)
        .await
        .map_err(backend)?;
    Ok(Bytes::new())
}

async fn group_members(
    State(st): State<RoyalState>,
    Path((app, group)): Path<(String, String)>,
) -> Result<Bytes, (StatusCode, String)> {
    let members = st.groups.members(&app, &group).await.map_err(backend)?;
    Ok(encode(&GroupMembersResp { members }))
}

async fn group_detail(
    State(st): State<RoyalState>,
    Path((app, group)): Path<(String, String)>,
) -> Result<Bytes, (StatusCode, String)> {
    let info = st.groups.detail(&app, &group).await.map_err(backend)?;
    Ok(encode(&GroupDetail {
        group_id: info.id,
        name: info.name,
        avatar: info.avatar,
        introduction: info.introduction,
        owner: info.owner,
        members: info.members,
    }))
}

pub async fn serve(listener: tokio::net::TcpListener, state: RoyalState) -> std::io::Result<()> {
    let addr = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    tracing::info!(%addr, "royal listening");
    axum::serve(listener, router(state))
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_protocol::MESSAGE_TYPE_TEXT;

    #[tokio::test]
    async fn http_create_join_detail() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        let base = format!("http://{addr}");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let (store, groups) = fake_chat::http_backends(&base).unwrap();
        let gid = groups
            .create(
                "kim",
                &CreateGroup {
                    name: "g".into(),
                    avatar: String::new(),
                    introduction: String::new(),
                    owner: "alice".into(),
                    members: vec!["bob".into()],
                },
            )
            .await
            .unwrap();
        groups.join("kim", &gid, "carol").await.unwrap();
        let d = groups.detail("kim", &gid).await.unwrap();
        assert!(d.members.contains(&"carol".to_string()));
        let inserted = store
            .insert_user(
                "kim",
                &InsertMessage {
                    sender: "alice".into(),
                    dest: "bob".into(),
                    send_time: 1,
                    msg_type: MESSAGE_TYPE_TEXT,
                    body: "hi".into(),
                    extra: String::new(),
                },
            )
            .await
            .unwrap();
        assert!(inserted.message_id > 10_000);
    }
}
