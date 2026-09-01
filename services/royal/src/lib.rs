//! In-process Royal: protobuf HTTP over axum. Chat talks to this via `Http*` adapters.

mod auth;
mod product;
mod revoke;

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use chat::directory::{CreateGroup, GroupDirectory, GroupError, MemoryGroupDirectory};
use chat::idgen::{IdGenerator, SequenceIdGen, SnowflakeGen};
use chat::social::{MemorySocialDirectory, SocialDirectory};
use chat::store::{
    collect_ack_ids, DeliveryTarget, InsertMessage, InsertResult, MemoryMessageStore, MessageKind,
    MessageStore,
};
use chat::users::{MemoryUserDirectory, UserDirectory, UserError};
use chat::{HmacNonceGuard, MemoryHmacNonceGuard};
use http_body_util::BodyExt;
use kim_protocol::pkt::{
    AccountExists, AccountQuery, AckMessageReq, DeliveryBackfillReq, GroupCreateResp, GroupDetail,
    GroupMembersResp, InsertFanout, InsertMessageReq, InsertMessageResp, InternalGroupCreate,
    InternalGroupMember, InternalGroupQuery, KickAccount, MessageContentReq, MessageContentResp,
    MessageIndex, MessageIndexResp, OfflineIndexReq, RevokeQuery, RevokeStatus,
};
use kim_protocol::{
    hmac_headers_from, resolve_internal_hmac_secret, sign_internal_hmac, verify_internal_hmac,
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
    hmac_secret: String,
    nonce: Arc<dyn HmacNonceGuard>,
    pending_receipt: bool,
}

impl RoyalState {
    pub fn memory(idgen: Arc<dyn IdGenerator>) -> Self {
        Self::memory_with_jwt(idgen, JwtConfig::default())
    }

    pub fn memory_with_jwt(idgen: Arc<dyn IdGenerator>, jwt: JwtConfig) -> Self {
        Self::memory_with_jwt_receipt(idgen, jwt, false)
    }

    pub fn memory_with_jwt_receipt(
        idgen: Arc<dyn IdGenerator>,
        jwt: JwtConfig,
        pending_receipt: bool,
    ) -> Self {
        let store: Arc<dyn MessageStore> = if pending_receipt {
            Arc::new(MemoryMessageStore::with_pending_receipt(idgen.clone()))
        } else {
            Arc::new(MemoryMessageStore::new(idgen.clone()))
        };
        Self {
            store,
            groups: Arc::new(MemoryGroupDirectory::new(idgen)),
            users: Arc::new(MemoryUserDirectory::new()),
            social: Arc::new(MemorySocialDirectory::new()),
            jwt,
            revoke: Arc::new(MemoryRevocation::new()),
            app: "kim".into(),
            chat_url: String::new(),
            hmac_secret: resolve_internal_hmac_secret(""),
            nonce: Arc::new(MemoryHmacNonceGuard::new()),
            pending_receipt,
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

    #[must_use]
    pub fn with_hmac_secret(mut self, secret: impl Into<String>) -> Self {
        self.hmac_secret = secret.into();
        self
    }

    #[must_use]
    pub fn with_nonce(mut self, nonce: Arc<dyn HmacNonceGuard>) -> Self {
        self.nonce = nonce;
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
            hmac_secret: resolve_internal_hmac_secret(""),
            nonce: Arc::new(MemoryHmacNonceGuard::new()),
            pending_receipt: false,
        }
    }

    #[must_use]
    pub fn with_pending_receipt(mut self, enabled: bool) -> Self {
        self.pending_receipt = enabled;
        self
    }

    pub fn start_maintenance(&self) {
        spawn_maintenance(self.store.clone());
    }
}

pub fn router(state: RoyalState) -> Router {
    Router::new()
        .route("/internal/user/lookup", post(user_lookup))
        .route("/internal/user/upsert", post(user_upsert))
        .route("/internal/revoke/check", post(revoke_check))
        .route("/api/v1/message/user", post(insert_user))
        .route("/api/v1/message/group", post(insert_group))
        .route("/api/v1/message/ack", post(ack))
        .route("/api/v1/offline/index", post(offline_index))
        .route("/api/v1/delivery/backfill", post(delivery_backfill))
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
        .route_layer(middleware::from_fn_with_state(state.clone(), require_hmac))
        .route("/health", get(health))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/password", post(auth::change_password))
        .route("/api/v1/auth/me", get(auth::me))
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

fn hmac_unauthorized() -> (StatusCode, String) {
    (StatusCode::UNAUTHORIZED, "unauthorized".into())
}

fn header_str<'a>(headers: &'a axum::http::HeaderMap, name: &'static str) -> &'a str {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

async fn require_hmac(
    State(st): State<RoyalState>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let (parts, body) = req.into_parts();
    let collected = body.collect().await.map_err(|_| hmac_unauthorized())?;
    let bytes = collected.to_bytes();
    if bytes.len() > 4 * 1024 * 1024 {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "payload too large".into()));
    }
    let headers = hmac_headers_from(|name| header_str(&parts.headers, name));
    if !verify_internal_hmac(
        st.hmac_secret.as_bytes(),
        parts.method.as_str(),
        parts.uri.path(),
        &bytes,
        &headers,
    ) {
        return Err(hmac_unauthorized());
    }
    match st.nonce.claim(&headers.nonce).await {
        Ok(true) => {}
        Ok(false) => return Err(hmac_unauthorized()),
        Err(err) => {
            tracing::error!(%err, "hmac nonce");
            return Err((StatusCode::SERVICE_UNAVAILABLE, "unavailable".into()));
        }
    }
    let req = Request::from_parts(parts, Body::from(bytes));
    Ok(next.run(req).await)
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

pub fn spawn_maintenance(store: Arc<dyn MessageStore>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            match store.gc_expired_deliveries(1000).await {
                Ok(n) if n > 0 => tracing::info!(gc_deleted = n, "pending delivery gc"),
                Ok(_) => {}
                Err(err) => tracing::warn!(%err, "pending delivery gc failed"),
            }
            match store.pending_delivery_stats().await {
                Ok((pending_rows, oldest_receipt_age_seconds)) => {
                    if pending_rows > 10_000_000 {
                        tracing::error!(pending_rows, "pending_delivery backlog");
                    } else {
                        tracing::info!(
                            pending_rows,
                            oldest_receipt_age_seconds,
                            "pending delivery stats"
                        );
                    }
                }
                Err(err) => tracing::warn!(%err, "pending delivery stats failed"),
            }
        }
    });
}

const PENDING_NOT_ENABLED: &str = "pending-not-enabled";

fn pending_disabled() -> (StatusCode, String) {
    (StatusCode::SERVICE_UNAVAILABLE, PENDING_NOT_ENABLED.into())
}

fn resolve_req_app(st: &RoyalState, req_app: &str) -> Result<String, (StatusCode, String)> {
    if req_app.is_empty() || req_app == st.app {
        Ok(st.app.clone())
    } else {
        Err((StatusCode::BAD_REQUEST, "app mismatch".into()))
    }
}

fn is_new_ack(req: &AckMessageReq) -> bool {
    !req.target_id.is_empty() || !req.message_ids.is_empty() || !req.app.is_empty()
}

fn is_new_index(req: &OfflineIndexReq) -> bool {
    !req.target_id.is_empty() || req.resume || !req.app.is_empty()
}

fn insert_from_req(req: InsertMessageReq) -> InsertMessage {
    let msg = req.message.unwrap_or_default();
    InsertMessage {
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
        online_targets: req
            .online_targets
            .into_iter()
            .map(|t| DeliveryTarget {
                account: t.account,
                target_id: t.target_id,
            })
            .collect(),
    }
}

async fn insert_user(
    State(st): State<RoyalState>,

    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InsertMessageReq>(&body)?;
    let inserted = st
        .store
        .insert_user(&st.app, &insert_from_req(req))
        .await
        .map_err(backend)?;
    Ok(encode(&encode_insert(&inserted)))
}

async fn insert_group(
    State(st): State<RoyalState>,

    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InsertMessageReq>(&body)?;
    let members = if req.members.is_empty() {
        st.groups
            .members(&st.app, &req.dest)
            .await
            .map_err(backend)?
    } else {
        req.members.clone()
    };
    let inserted = st
        .store
        .insert_group(&st.app, &insert_from_req(req), &members)
        .await
        .map_err(backend)?;
    Ok(encode(&encode_insert(&inserted)))
}

fn encode_insert(inserted: &InsertResult) -> InsertMessageResp {
    InsertMessageResp {
        message_id: inserted.message_id,
        send_time: inserted.send_time,
        duplicate: inserted.duplicate,
        fanout: Some(InsertFanout {
            r#type: inserted.fanout.msg_type,
            body: inserted.fanout.body.clone(),
            extra: inserted.fanout.extra.clone(),
            sender: inserted.fanout.sender.clone(),
            dest: inserted.fanout.dest.clone(),
            kind: match inserted.fanout.kind {
                MessageKind::User => 0,
                MessageKind::Group => 1,
            },
            recipients: inserted.fanout.recipients.clone(),
        }),
    }
}

async fn ack(State(st): State<RoyalState>, body: Bytes) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AckMessageReq>(&body)?;
    if is_new_ack(&req) {
        if !st.pending_receipt {
            return Err(pending_disabled());
        }
        let app = resolve_req_app(&st, &req.app)?;
        let ids = collect_ack_ids(req.message_id, &req.message_ids);
        st.store
            .ack(&app, &req.account, &req.target_id, &ids)
            .await
            .map_err(backend)?;
    } else {
        let ids = collect_ack_ids(req.message_id, &[]);
        st.store
            .ack(&st.app, &req.account, "", &ids)
            .await
            .map_err(backend)?;
    }
    Ok(Bytes::new())
}

async fn offline_index(
    State(st): State<RoyalState>,

    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<OfflineIndexReq>(&body)?;
    let (rows, has_more) = if is_new_index(&req) {
        if !st.pending_receipt {
            return Err(pending_disabled());
        }
        let app = resolve_req_app(&st, &req.app)?;
        st.store
            .offline_index(
                &app,
                &req.account,
                &req.target_id,
                req.message_id,
                req.resume,
            )
            .await
            .map_err(backend)?
    } else {
        st.store
            .offline_index(&st.app, &req.account, "", req.message_id, false)
            .await
            .map_err(backend)?
    };
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
        has_more,
    };
    Ok(encode(&resp))
}

async fn delivery_backfill(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    if !st.pending_receipt {
        return Err(pending_disabled());
    }
    let req = decode::<DeliveryBackfillReq>(&body)?;
    let app = resolve_req_app(&st, &req.app)?;
    st.store
        .backfill_delivery(&app, &req.account, &req.target_id)
        .await
        .map_err(backend)?;
    Ok(Bytes::new())
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
    let headers =
        match sign_internal_hmac(st.hmac_secret.as_bytes(), "POST", "/internal/kick", &body) {
            Ok(h) => h,
            Err(err) => {
                tracing::error!(%err, account, "kick sign");
                return;
            }
        };
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(err) => {
            tracing::error!(%err, account, "kick client");
            return;
        }
    };
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/x-protobuf");
    for (k, v) in headers.pairs() {
        req = req.header(k, v);
    }
    match req.body(body).send().await {
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
    use kim_protocol::{sign_internal_hmac, MESSAGE_TYPE_TEXT};
    use prost::Message;

    fn signed_post(url: &str, path: &str, body: Vec<u8>) -> reqwest::RequestBuilder {
        let secret = resolve_internal_hmac_secret("");
        let headers = sign_internal_hmac(secret.as_bytes(), "POST", path, &body).unwrap();
        let mut req = reqwest::Client::new()
            .post(url)
            .header("Content-Type", "application/x-protobuf")
            .header("Accept", "application/x-protobuf");
        for (k, v) in headers.pairs() {
            req = req.header(k, v);
        }
        req.body(body)
    }

    #[tokio::test]
    async fn http_create_join_detail() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        let base = format!("http://{addr}");
        tokio::time::sleep(Duration::from_millis(20)).await;

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
                    online_targets: Vec::new(),
                },
            )
            .await
            .unwrap();
        assert!(inserted.message_id > 10_000);
        assert_eq!(inserted.fanout.body, "hi");
        assert!(inserted.fanout.recipients.contains(&"alice".into()));
        assert!(inserted.fanout.recipients.contains(&"bob".into()));
    }

    #[tokio::test]
    async fn http_insert_user_duplicate_returns_original_fanout() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        let base = format!("http://{addr}");
        tokio::time::sleep(Duration::from_millis(20)).await;

        let (store, _, _, _) = chat::http_backends(&base).unwrap();
        let first = store
            .insert_user(
                "kim",
                &InsertMessage {
                    sender: "alice".into(),
                    dest: "bob".into(),
                    send_time: 1,
                    msg_type: MESSAGE_TYPE_TEXT,
                    body: "orig".into(),
                    extra: String::new(),
                    client_id: "c1".into(),
                    online_targets: Vec::new(),
                },
            )
            .await
            .unwrap();
        assert!(!first.duplicate);
        let second = store
            .insert_user(
                "kim",
                &InsertMessage {
                    sender: "alice".into(),
                    dest: "carol".into(),
                    send_time: 2,
                    msg_type: MESSAGE_TYPE_TEXT,
                    body: "CHANGED".into(),
                    extra: String::new(),
                    client_id: "c1".into(),
                    online_targets: Vec::new(),
                },
            )
            .await
            .unwrap();
        assert!(second.duplicate);
        assert_eq!(second.message_id, first.message_id);
        assert_eq!(second.fanout.body, "orig");
        assert_eq!(second.fanout.dest, "bob");
    }

    #[tokio::test]
    async fn unauthenticated_duplicate_insert_is_unauthorized_without_fanout() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        let base = format!("http://{addr}");
        tokio::time::sleep(Duration::from_millis(20)).await;

        let health = reqwest::Client::new()
            .get(format!("{base}/health"))
            .send()
            .await
            .unwrap();
        assert!(health.status().is_success());

        let (store, _, _, _) = chat::http_backends(&base).unwrap();
        let first = store
            .insert_user(
                "kim",
                &InsertMessage {
                    sender: "alice".into(),
                    dest: "bob".into(),
                    send_time: 1,
                    msg_type: MESSAGE_TYPE_TEXT,
                    body: "orig".into(),
                    extra: String::new(),
                    client_id: "c1".into(),
                    online_targets: Vec::new(),
                },
            )
            .await
            .unwrap();
        assert!(!first.duplicate);

        let body = InsertMessageReq {
            sender: "alice".into(),
            dest: "carol".into(),
            send_time: 2,
            message: Some(kim_protocol::pkt::MessageReq {
                r#type: MESSAGE_TYPE_TEXT,
                body: "CHANGED".into(),
                extra: String::new(),
                client_id: "c1".into(),
            }),
            members: Vec::new(),
            client_id: "c1".into(),
            online_targets: Vec::new(),
        }
        .encode_to_vec();
        let resp = reqwest::Client::new()
            .post(format!("{base}/api/v1/message/user"))
            .header("Content-Type", "application/x-protobuf")
            .header("Accept", "application/x-protobuf")
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let buf = resp.bytes().await.unwrap();
        assert!(!buf.as_ref().windows(b"orig".len()).any(|w| w == b"orig"));
        assert!(!buf
            .as_ref()
            .windows(b"CHANGED".len())
            .any(|w| w == b"CHANGED"));
        if let Ok(decoded) = InsertMessageResp::decode(buf.as_ref()) {
            assert!(decoded.fanout.is_none());
            assert_eq!(decoded.message_id, 0);
        }
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
        tokio::time::sleep(Duration::from_millis(20)).await;
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
        let looked = signed_post(
            &format!("http://{addr}/internal/user/lookup"),
            "/internal/user/lookup",
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
        tokio::time::sleep(Duration::from_millis(20)).await;
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
                    online_targets: Vec::new(),
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

        let unsigned = reqwest::Client::new()
            .post(format!("http://{addr}/api/v1/offline/content"))
            .header("Content-Type", "application/x-protobuf")
            .body(
                MessageContentReq {
                    message_ids: vec![inserted.message_id],
                    account: "bob".into(),
                    app: "kim".into(),
                }
                .encode_to_vec(),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(unsigned.status(), StatusCode::UNAUTHORIZED);

        let empty = signed_post(
            &format!("http://{addr}/api/v1/offline/content"),
            "/api/v1/offline/content",
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
        let no_app = signed_post(
            &format!("http://{addr}/api/v1/group/detail"),
            "/api/v1/group/detail",
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

    #[tokio::test]
    async fn replay_signed_request_is_unauthorized() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let body = MessageContentReq {
            message_ids: vec![1],
            account: "bob".into(),
            app: "kim".into(),
        }
        .encode_to_vec();
        let secret = resolve_internal_hmac_secret("");
        let headers =
            sign_internal_hmac(secret.as_bytes(), "POST", "/api/v1/offline/content", &body)
                .unwrap();
        let send = || {
            let mut req = reqwest::Client::new()
                .post(format!("http://{addr}/api/v1/offline/content"))
                .header("Content-Type", "application/x-protobuf");
            for (k, v) in headers.pairs() {
                req = req.header(k, v);
            }
            req.body(body.clone())
        };
        let first = send().send().await.unwrap();
        assert_ne!(first.status(), StatusCode::UNAUTHORIZED);
        let second = send().send().await.unwrap();
        assert_eq!(second.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(second.text().await.unwrap(), "unauthorized");
    }

    #[tokio::test]
    async fn kick_account_sends_hmac_headers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let got = Arc::new(std::sync::Mutex::new(None::<axum::http::HeaderMap>));
        let got2 = got.clone();
        let app = Router::new().route(
            "/internal/kick",
            post(move |headers: axum::http::HeaderMap, body: Bytes| {
                let got2 = got2.clone();
                async move {
                    *got2.lock().unwrap_or_else(|e| e.into_inner()) = Some(headers);
                    let req = KickAccount::decode(body.as_ref()).unwrap();
                    assert_eq!(req.account, "alice");
                    StatusCode::NO_CONTENT
                }
            }),
        );
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let st = RoyalState::memory(Arc::new(SequenceIdGen::default()))
            .with_chat_url(format!("http://{addr}"))
            .with_hmac_secret("kick-hmac-secret-xx");
        kick_account(&st, "alice").await;
        let headers = got
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .expect("headers");
        assert!(!headers
            .get(kim_protocol::HEADER_TIMESTAMP)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .is_empty());
        assert!(!headers
            .get(kim_protocol::HEADER_NONCE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .is_empty());
        assert!(!headers
            .get(kim_protocol::HEADER_SIGNATURE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .is_empty());
    }

    #[tokio::test]
    async fn writer_off_rejects_new_ack_index_backfill() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let ack = AckMessageReq {
            account: "bob".into(),
            target_id: "j1".into(),
            app: "kim".into(),
            message_ids: vec![1],
            ..Default::default()
        }
        .encode_to_vec();
        let resp = signed_post(
            &format!("http://{addr}/api/v1/message/ack"),
            "/api/v1/message/ack",
            ack,
        )
        .send()
        .await
        .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.text().await.unwrap(), "pending-not-enabled");

        let idx = OfflineIndexReq {
            account: "bob".into(),
            target_id: "j1".into(),
            app: "kim".into(),
            resume: true,
            ..Default::default()
        }
        .encode_to_vec();
        let resp = signed_post(
            &format!("http://{addr}/api/v1/offline/index"),
            "/api/v1/offline/index",
            idx,
        )
        .send()
        .await
        .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

        let bf = DeliveryBackfillReq {
            app: "kim".into(),
            account: "bob".into(),
            target_id: "j1".into(),
        }
        .encode_to_vec();
        let resp = signed_post(
            &format!("http://{addr}/api/v1/delivery/backfill"),
            "/api/v1/delivery/backfill",
            bf,
        )
        .send()
        .await
        .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn writer_on_chat_off_piles_receipts() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let mem = Arc::new(MemoryMessageStore::with_pending_receipt(idgen.clone()));
        let state = RoyalState::with_backends(
            mem.clone(),
            Arc::new(MemoryGroupDirectory::new(idgen)),
            Arc::new(MemoryUserDirectory::new()),
            Arc::new(MemorySocialDirectory::new()),
            JwtConfig::default(),
            Arc::new(MemoryRevocation::new()),
        )
        .with_pending_receipt(true);
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let (store, _, _, _) = chat::http_backends(&format!("http://{addr}")).unwrap();
        let req = InsertMessage {
            sender: "alice".into(),
            dest: "bob".into(),
            send_time: 1,
            msg_type: MESSAGE_TYPE_TEXT,
            body: "hi".into(),
            extra: String::new(),
            client_id: String::new(),
            online_targets: vec![DeliveryTarget {
                account: "bob".into(),
                target_id: "j1".into(),
            }],
        };
        let inserted = store.insert_user("kim", &req).await.unwrap();
        store
            .ack("kim", "bob", "", &[inserted.message_id])
            .await
            .unwrap();
        let (idx, _) = mem
            .offline_index("kim", "bob", "j1", 0, true)
            .await
            .unwrap();
        assert!(idx.iter().any(|r| r.message_id == inserted.message_id));
    }

    #[tokio::test]
    async fn chat_on_royal_off_new_ack_is_503() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = RoyalState::memory(Arc::new(SequenceIdGen::default()));
        tokio::spawn(async move {
            let _ = serve(listener, state).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let secret = resolve_internal_hmac_secret("");
        let (store, _, _, _) =
            chat::http_backends_with_hmac_receipt(&format!("http://{addr}"), &secret, true)
                .unwrap();
        let err = store.ack("kim", "bob", "j1", &[11]).await.unwrap_err();
        match err {
            chat::store::StoreError::Http { status, msg } => {
                assert_eq!(status, 503);
                assert!(msg.contains("pending-not-enabled"));
            }
            other => panic!("{other}"),
        }
        let err = store
            .offline_index("kim", "bob", "j1", 0, true)
            .await
            .unwrap_err();
        match err {
            chat::store::StoreError::Http { status, .. } => assert_eq!(status, 503),
            other => panic!("{other}"),
        }
    }
}
