//! In-process Royal: protobuf HTTP over axum. Chat talks to this via `Http*` adapters.

mod auth;
mod product;
mod revoke;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use chat::directory::{CreateGroup, GroupDirectory, GroupError, MemoryGroupDirectory};
use chat::idgen::{IdGenerator, SequenceIdGen, SnowflakeGen};
use chat::social::{MemorySocialDirectory, SocialDirectory};
use chat::store::{InsertMessage, MemoryMessageStore, MessageStore};
use chat::users::{MemoryUserDirectory, UserDirectory, UserError};
use kim_protocol::pkt::{
    AccountExists, AccountQuery, AckMessageReq, GroupCreateResp, GroupDetail, GroupMembersResp,
    InsertMessageReq, InsertMessageResp, InternalGroupCreate, InternalGroupMember,
    InternalGroupQuery, KickAccount, MessageContentReq, MessageContentResp, MessageIndex,
    MessageIndexResp, OfflineIndexReq, RevokeQuery, RevokeStatus,
};
use prost::Message;

#[cfg(feature = "redis")]
pub use revoke::RedisRevocation;
pub use revoke::{MemoryRevocation, TokenRevocation};

#[derive(Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub ttl_secs: i64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: kim_protocol::DEMO_DEFAULT_SECRET.to_string(),
            ttl_secs: 86_400,
        }
    }
}

#[derive(Clone)]
pub struct RoyalState {
    store: Arc<dyn MessageStore>,
    groups: Arc<dyn GroupDirectory>,
    pub(crate) users: Arc<dyn UserDirectory>,
    pub(crate) social: Arc<dyn SocialDirectory>,
    pub(crate) jwt: JwtConfig,
    pub(crate) revoke: Arc<dyn TokenRevocation>,
    pub(crate) app: String,
    pub(crate) chat_url: String,
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
            social: Arc::new(MemorySocialDirectory::new()),
            jwt,
            revoke: Arc::new(MemoryRevocation::new()),
            app: "kim".into(),
            chat_url: String::new(),
        }
    }

    #[must_use]
    pub fn with_revoke(mut self, revoke: Arc<dyn TokenRevocation>) -> Self {
        self.revoke = revoke;
        self
    }

    #[must_use]
    pub fn with_app(mut self, app: impl Into<String>) -> Self {
        let app = app.into();
        self.app = if app.is_empty() { "kim".into() } else { app };
        self
    }

    #[must_use]
    pub fn with_chat_url(mut self, url: impl Into<String>) -> Self {
        self.chat_url = url.into().trim_end_matches('/').to_string();
        self
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
        social: Arc<dyn SocialDirectory>,
        jwt: JwtConfig,
        revoke: Arc<dyn TokenRevocation>,
    ) -> Self {
        Self {
            store,
            groups,
            users,
            social,
            jwt,
            revoke,
            app: "kim".into(),
            chat_url: String::new(),
        }
    }
}

pub fn router(state: RoyalState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/password", post(auth::change_password))
        .route("/api/v1/auth/me", get(auth::me))
        .route("/internal/user/lookup", post(user_lookup))
        .route("/internal/user/upsert", post(user_upsert))
        .route("/internal/revoke/check", post(revoke_check))
        .route("/api/v1/message/user", post(insert_user))
        .route("/api/v1/message/group", post(insert_group))
        .route("/api/v1/message/ack", post(ack))
        .route("/api/v1/offline/index", post(offline_index))
        .route("/api/v1/offline/content", post(offline_content))
        .route("/api/v1/group", post(group_create))
        .route("/api/v1/group/member", post(group_join))
        .route("/api/v1/group/quit", post(group_quit))
        .route("/api/v1/group/members", post(group_members))
        .route("/api/v1/group/detail", post(group_detail))
        .route("/api/v1/user/profile", post(product::user_profile))
        .route("/api/v1/user/update", post(product::user_update))
        .route("/api/v1/user/profiles", post(product::user_profiles))
        .route("/api/v1/user/search", post(product::user_search))
        .route("/api/v1/friend/request", post(product::friend_request))
        .route("/api/v1/friend/accept", post(product::friend_accept))
        .route("/api/v1/friend/reject", post(product::friend_reject))
        .route("/api/v1/friend/remove", post(product::friend_remove))
        .route("/api/v1/friend/list", post(product::friend_list))
        .route("/api/v1/friend/incoming", post(product::friend_incoming))
        .route("/api/v1/friend/check", post(product::friend_check))
        .route("/api/v1/block/add", post(product::block_add))
        .route("/api/v1/block/remove", post(product::block_remove))
        .route("/api/v1/block/list", post(product::block_list))
        .route("/api/v1/block/check", post(product::block_check))
        .route("/api/v1/inbox", post(product::inbox_list))
        .route("/api/v1/history", post(product::history))
        .route("/api/v1/inbox/read", post(product::inbox_read))
        .with_state(state)
}

pub(crate) fn decode<T: Message + Default>(body: &Bytes) -> Result<T, (StatusCode, String)> {
    T::decode(body.as_ref()).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

pub(crate) fn encode(msg: &impl Message) -> Bytes {
    Bytes::from(msg.encode_to_vec())
}

pub(crate) fn backend(err: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

fn group_http(err: GroupError) -> (StatusCode, String) {
    match err {
        GroupError::NotFound => (StatusCode::NOT_FOUND, err.to_string()),
        GroupError::Id(_) | GroupError::Backend(_) => backend(err),
    }
}

fn require_app(app: &str) -> Result<(), (StatusCode, String)> {
    if app.is_empty() {
        Err((StatusCode::BAD_REQUEST, "empty app".into()))
    } else {
        Ok(())
    }
}

async fn health() -> &'static str {
    "ok"
}

pub(crate) fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn insert_user(
    State(st): State<RoyalState>,

    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InsertMessageReq>(&body)?;
    let msg = req.message.unwrap_or_default();
    let inserted = st
        .store
        .insert_user(
            &st.app,
            &InsertMessage {
                sender: req.sender,
                dest: req.dest,
                send_time: req.send_time,
                msg_type: msg.r#type,
                body: msg.body,
                extra: msg.extra,
                client_id: if req.client_id.is_empty() {
                    msg.client_id
                } else {
                    req.client_id
                },
            },
        )
        .await
        .map_err(backend)?;
    Ok(encode(&InsertMessageResp {
        message_id: inserted.message_id,
        send_time: inserted.send_time,
        duplicate: inserted.duplicate,
    }))
}

async fn insert_group(
    State(st): State<RoyalState>,

    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InsertMessageReq>(&body)?;
    let msg = req.message.unwrap_or_default();
    let members = if req.members.is_empty() {
        st.groups
            .members(&st.app, &req.dest)
            .await
            .map_err(backend)?
    } else {
        req.members
    };
    let inserted = st
        .store
        .insert_group(
            &st.app,
            &InsertMessage {
                sender: req.sender,
                dest: req.dest,
                send_time: req.send_time,
                msg_type: msg.r#type,
                body: msg.body,
                extra: msg.extra,
                client_id: if req.client_id.is_empty() {
                    msg.client_id
                } else {
                    req.client_id
                },
            },
            &members,
        )
        .await
        .map_err(backend)?;
    Ok(encode(&InsertMessageResp {
        message_id: inserted.message_id,
        send_time: inserted.send_time,
        duplicate: inserted.duplicate,
    }))
}

async fn ack(State(st): State<RoyalState>, body: Bytes) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AckMessageReq>(&body)?;
    st.store
        .ack(&st.app, &req.account, req.message_id)
        .await
        .map_err(backend)?;
    Ok(Bytes::new())
}

async fn offline_index(
    State(st): State<RoyalState>,

    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<OfflineIndexReq>(&body)?;
    let rows = st
        .store
        .offline_index(&st.app, &req.account, req.message_id)
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

    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<MessageContentReq>(&body)?;
    if req.app.is_empty() || req.account.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty app or account".into()));
    }
    let rows = st
        .store
        .offline_content(&req.app, &req.account, &req.message_ids)
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

    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InternalGroupCreate>(&body)?;
    require_app(&req.app)?;
    let group_id = st
        .groups
        .create(
            &req.app,
            &CreateGroup {
                name: req.name,
                avatar: req.avatar,
                introduction: req.introduction,
                owner: req.owner,
                members: req.members,
            },
        )
        .await
        .map_err(group_http)?;
    Ok(encode(&GroupCreateResp { group_id }))
}

async fn group_join(
    State(st): State<RoyalState>,

    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InternalGroupMember>(&body)?;
    require_app(&req.app)?;
    st.groups
        .join(&req.app, &req.group_id, &req.account)
        .await
        .map_err(group_http)?;
    Ok(Bytes::new())
}

async fn group_quit(
    State(st): State<RoyalState>,

    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InternalGroupMember>(&body)?;
    require_app(&req.app)?;
    st.groups
        .quit(&req.app, &req.group_id, &req.account)
        .await
        .map_err(group_http)?;
    Ok(Bytes::new())
}

async fn group_members(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InternalGroupQuery>(&body)?;
    require_app(&req.app)?;
    let members = st
        .groups
        .members(&req.app, &req.group_id)
        .await
        .map_err(group_http)?;
    Ok(encode(&GroupMembersResp { members }))
}

async fn group_detail(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InternalGroupQuery>(&body)?;
    require_app(&req.app)?;
    let info = st
        .groups
        .detail(&req.app, &req.group_id)
        .await
        .map_err(group_http)?;
    Ok(encode(&GroupDetail {
        group_id: info.id,
        name: info.name,
        avatar: info.avatar,
        introduction: info.introduction,
        owner: info.owner,
        members: info.members,
    }))
}

async fn user_lookup(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountQuery>(&body)?;
    if req.account.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty account".into()));
    }
    let exists = st
        .users
        .exists(&st.app, &req.account)
        .await
        .map_err(|e| match e {
            UserError::Backend(s) => backend(s),
            UserError::Conflict => (StatusCode::CONFLICT, "conflict".into()),
            UserError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
            UserError::InvalidProfile => (StatusCode::BAD_REQUEST, "invalid profile".into()),
        })?;
    Ok(encode(&AccountExists { exists }))
}

async fn user_upsert(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let req = decode::<AccountQuery>(&body)?;
    if req.account.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty account".into()));
    }
    st.users
        .upsert(&st.app, &req.account)
        .await
        .map_err(|e| match e {
            UserError::Backend(s) => backend(s),
            UserError::Conflict => (StatusCode::CONFLICT, "conflict".into()),
            UserError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
            UserError::InvalidProfile => (StatusCode::BAD_REQUEST, "invalid profile".into()),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke_check(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<RevokeQuery>(&body)?;
    if req.jti.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty jti".into()));
    }
    let revoked = st
        .revoke
        .is_revoked(&req.jti)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(encode(&RevokeStatus { revoked }))
}

pub(crate) async fn kick_account(st: &RoyalState, account: &str) {
    if st.chat_url.is_empty() || account.is_empty() {
        return;
    }
    let url = format!("{}/internal/kick", st.chat_url);
    let body = KickAccount {
        account: account.to_string(),
        app: st.app.clone(),
    }
    .encode_to_vec();
    match reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/x-protobuf")
        .body(body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {}
        Ok(resp) => tracing::error!(status = %resp.status(), account, "kick http"),
        Err(err) => tracing::error!(%err, account, "kick http"),
    }
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
    use prost::Message;

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

        let (store, groups, _users, _social) = chat::http_backends(&base).unwrap();
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
                    client_id: String::new(),
                },
            )
            .await
            .unwrap();
        assert!(inserted.message_id > 10_000);
    }

    #[tokio::test]
    async fn register_login_logout_and_rejects() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let jwt = JwtConfig {
            secret: "test-secret".into(),
            ttl_secs: 60,
        };
        let state = RoyalState::memory_with_jwt(Arc::new(SequenceIdGen::default()), jwt);
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let http = reqwest::Client::new();
        let gone = http
            .post(format!("http://{addr}/api/kim/token"))
            .send()
            .await
            .unwrap();
        assert_eq!(gone.status(), StatusCode::NOT_FOUND);

        let register = format!("http://{addr}/api/v1/auth/register");
        let login = format!("http://{addr}/api/v1/auth/login");
        let logout = format!("http://{addr}/api/v1/auth/logout");
        let pb = |account: &str, password: &str| {
            kim_protocol::pkt::AuthReq {
                account: account.into(),
                password: password.into(),
            }
            .encode_to_vec()
        };
        let post_pb = |url: &str, body: Vec<u8>| {
            http.post(url)
                .header("Content-Type", "application/x-protobuf")
                .header("Accept", "application/x-protobuf")
                .body(body)
        };

        let short = post_pb(&register, pb("ab", "secret123"))
            .send()
            .await
            .unwrap();
        assert_eq!(short.status(), StatusCode::BAD_REQUEST);

        let weak = post_pb(&register, pb("alice", "short"))
            .send()
            .await
            .unwrap();
        assert_eq!(weak.status(), StatusCode::BAD_REQUEST);

        let created = post_pb(&register, pb("alice", "secret123"))
            .send()
            .await
            .unwrap();
        assert!(created.status().is_success());
        let buf = created.bytes().await.unwrap();
        let resp = kim_protocol::pkt::AuthResp::decode(buf.as_ref()).unwrap();
        let looked = post_pb(
            &format!("http://{addr}/internal/user/lookup"),
            AccountQuery {
                account: "alice".into(),
            }
            .encode_to_vec(),
        )
        .send()
        .await
        .unwrap();
        assert!(looked.status().is_success());
        let exists = AccountExists::decode(looked.bytes().await.unwrap().as_ref()).unwrap();
        assert!(exists.exists);
        let claims = kim_protocol::parse("test-secret", &resp.token).unwrap();
        assert_eq!(claims.account, "alice");
        assert_eq!(claims.app, "kim");
        assert!(claims.jti.is_some());

        let dup = post_pb(&register, pb("alice", "secret123"))
            .send()
            .await
            .unwrap();
        assert_eq!(dup.status(), StatusCode::CONFLICT);

        let bad_pw = post_pb(&login, pb("alice", "wrongpass"))
            .send()
            .await
            .unwrap();
        assert_eq!(bad_pw.status(), StatusCode::UNAUTHORIZED);
        let unknown = post_pb(&login, pb("bob", "secret123"))
            .send()
            .await
            .unwrap();
        assert_eq!(unknown.status(), StatusCode::UNAUTHORIZED);

        let ok = post_pb(&login, pb("alice", "secret123"))
            .send()
            .await
            .unwrap();
        assert!(ok.status().is_success());
        let login_buf = ok.bytes().await.unwrap();
        let login_resp = kim_protocol::pkt::AuthResp::decode(login_buf.as_ref()).unwrap();

        let me = http
            .get(format!("http://{addr}/api/v1/auth/me"))
            .header("Authorization", format!("Bearer {}", login_resp.token))
            .send()
            .await
            .unwrap();
        assert!(me.status().is_success());
        let me_body: serde_json::Value = me.json().await.unwrap();
        assert_eq!(me_body["account"], "alice");
        assert_eq!(me_body["app"], "kim");

        let out = http
            .post(&logout)
            .header("Authorization", format!("Bearer {}", login_resp.token))
            .send()
            .await
            .unwrap();
        assert_eq!(out.status(), StatusCode::NO_CONTENT);

        let me_after = http
            .get(format!("http://{addr}/api/v1/auth/me"))
            .header("Authorization", format!("Bearer {}", login_resp.token))
            .send()
            .await
            .unwrap();
        assert_eq!(me_after.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn content_and_group_use_request_app_not_process_app() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = RoyalState::memory(Arc::new(SequenceIdGen::default())).with_app("kim");
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let (store, groups, _, _) =
            chat::http_backends(&format!("http://{addr}")).expect("backends");

        let inserted = store
            .insert_user(
                "kim",
                &InsertMessage {
                    sender: "alice".into(),
                    dest: "bob".into(),
                    send_time: 1,
                    msg_type: MESSAGE_TYPE_TEXT,
                    body: "secret".into(),
                    extra: String::new(),
                    client_id: String::new(),
                },
            )
            .await
            .unwrap();
        let gray = store
            .offline_content("kim-gray", "bob", &[inserted.message_id])
            .await
            .unwrap();
        assert!(gray.is_empty());
        let bob = store
            .offline_content("kim", "bob", &[inserted.message_id])
            .await
            .unwrap();
        assert_eq!(bob.len(), 1);
        assert_eq!(bob[0].body, "secret");
        let carol = store
            .offline_content("kim", "carol", &[inserted.message_id])
            .await
            .unwrap();
        assert!(carol.is_empty());

        let empty = reqwest::Client::new()
            .post(format!("http://{addr}/api/v1/offline/content"))
            .header("Content-Type", "application/x-protobuf")
            .body(
                MessageContentReq {
                    message_ids: vec![inserted.message_id],
                    ..Default::default()
                }
                .encode_to_vec(),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(empty.status(), StatusCode::BAD_REQUEST);

        let gid = groups
            .create(
                "kim-gray",
                &CreateGroup {
                    name: "g".into(),
                    avatar: String::new(),
                    introduction: String::new(),
                    owner: "alice".into(),
                    members: vec!["alice".into()],
                },
            )
            .await
            .unwrap();
        let gray_detail = groups.detail("kim-gray", &gid).await.unwrap();
        assert_eq!(gray_detail.name, "g");
        match groups.detail("kim", &gid).await {
            Err(GroupError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
        let no_app = reqwest::Client::new()
            .post(format!("http://{addr}/api/v1/group/detail"))
            .header("Content-Type", "application/x-protobuf")
            .body(
                InternalGroupQuery {
                    group_id: gid,
                    ..Default::default()
                }
                .encode_to_vec(),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(no_app.status(), StatusCode::BAD_REQUEST);
    }
}
