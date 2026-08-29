//! In-process Royal: protobuf HTTP over axum. Chat talks to this via `Http*` adapters.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use chat::directory::{CreateGroup, GroupDirectory, MemoryGroupDirectory};
use chat::idgen::{IdGenerator, SequenceIdGen, SnowflakeGen};
use chat::store::{InsertMessage, MemoryMessageStore, MessageStore};
use chat::users::{MemoryUserDirectory, UserDirectory};
use kim_protocol::pkt::{
    AckMessageReq, GroupCreateReq, GroupCreateResp, GroupDetail, GroupJoinReq, GroupMembersResp,
    GroupQuitReq, InsertMessageReq, InsertMessageResp, MessageContentReq, MessageContentResp,
    MessageIndex, MessageIndexResp, OfflineIndexReq,
};
use kim_protocol::{generate, ProtocolError};
use prost::Message;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub ttl_secs: i64,
    pub issue_key: String,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: kim_protocol::DEMO_DEFAULT_SECRET.to_string(),
            ttl_secs: 86_400,
            issue_key: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct RoyalState {
    store: Arc<dyn MessageStore>,
    groups: Arc<dyn GroupDirectory>,
    users: Arc<dyn UserDirectory>,
    jwt: JwtConfig,
}

impl RoyalState {
    pub fn memory(idgen: Arc<dyn IdGenerator>) -> Self {
        Self::memory_with_jwt(idgen, JwtConfig::default())
    }

    pub fn memory_with_jwt(idgen: Arc<dyn IdGenerator>, jwt: JwtConfig) -> Self {
        Self {
            store: Arc::new(MemoryMessageStore::new(idgen.clone())),
            groups: Arc::new(MemoryGroupDirectory::new(idgen)),
            users: Arc::new(MemoryUserDirectory::new()),
            jwt,
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

    pub fn with_backends(
        store: Arc<dyn MessageStore>,
        groups: Arc<dyn GroupDirectory>,
        users: Arc<dyn UserDirectory>,
        jwt: JwtConfig,
    ) -> Self {
        Self {
            store,
            groups,
            users,
            jwt,
        }
    }
}

#[derive(Deserialize)]
struct TokenReq {
    account: String,
}

#[derive(Serialize)]
struct TokenResp {
    token: String,
    exp: i64,
}

pub fn router(state: RoyalState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/{app}/token", post(issue_token))
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

async fn health() -> &'static str {
    "ok"
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn issue_token(
    State(st): State<RoyalState>,
    Path(app): Path<String>,
    headers: HeaderMap,
    Json(req): Json<TokenReq>,
) -> Result<Json<TokenResp>, (StatusCode, String)> {
    if !st.jwt.issue_key.is_empty() {
        let got = headers
            .get("x-kim-issue-key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if got != st.jwt.issue_key {
            return Err((StatusCode::UNAUTHORIZED, "issue key".into()));
        }
    }
    let ttl = if st.jwt.ttl_secs > 0 {
        st.jwt.ttl_secs
    } else {
        86_400
    };
    let exp = now_ts().saturating_add(ttl);
    st.users.upsert(&app, &req.account).await.map_err(backend)?;
    let token = generate(&st.jwt.secret, &req.account, &app, exp).map_err(|e| match e {
        ProtocolError::InvalidAccount => (StatusCode::BAD_REQUEST, "invalid account".into()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "token".into()),
    })?;
    Ok(Json(TokenResp { token, exp }))
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

        let (store, groups) = chat::http_backends(&base).unwrap();
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

    #[tokio::test]
    async fn token_roundtrip_and_rejects() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let jwt = JwtConfig {
            secret: "test-secret".into(),
            ttl_secs: 60,
            issue_key: "ik".into(),
        };
        let state = RoyalState::memory_with_jwt(Arc::new(SequenceIdGen::default()), jwt);
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let http = reqwest::Client::new();
        let url = format!("http://{addr}/api/kim/token");
        let denied = http
            .post(&url)
            .json(&serde_json::json!({"account":"alice"}))
            .send()
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        let ok = http
            .post(&url)
            .header("X-KIM-Issue-Key", "ik")
            .json(&serde_json::json!({"account":"alice"}))
            .send()
            .await
            .unwrap();
        assert!(ok.status().is_success());
        let body: serde_json::Value = ok.json().await.unwrap();
        let token = body["token"].as_str().unwrap();
        let claims = kim_protocol::parse("test-secret", token).unwrap();
        assert_eq!(claims.account, "alice");
        assert_eq!(claims.app, "kim");
        let bad = http
            .post(&url)
            .header("X-KIM-Issue-Key", "ik")
            .json(&serde_json::json!({"account":""}))
            .send()
            .await
            .unwrap();
        assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
        let ctrl = http
            .post(&url)
            .header("X-KIM-Issue-Key", "ik")
            .json(&serde_json::json!({"account":"a\nb"}))
            .send()
            .await
            .unwrap();
        assert_eq!(ctrl.status(), StatusCode::BAD_REQUEST);
    }
}
