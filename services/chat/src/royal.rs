//! HTTP adapters that send Chat store/directory calls to royal.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Bytes;
use kim_protocol::pkt::{
    AccountExists, AccountList, AccountPair, AccountQuery, AckMessageReq, ConversationRead,
    DeliveryBackfillReq, DeliveryTarget as PbDeliveryTarget, GroupCreateResp, GroupDetail,
    GroupMembersResp, HistoryQuery, HistoryResp, InboxQuery, InboxResp, InsertFanout,
    InsertMessageReq, InsertMessageResp, InternalGroupCreate, InternalGroupMember,
    InternalGroupQuery, MessageContentReq, MessageContentResp, MessageIndexResp, MessageReq,
    OfflineIndexReq, ProfileUpdateReq, UserListResp, UserProfile as PbProfile, UserSearchQuery,
    UserSearchResp,
};
use kim_protocol::{resolve_internal_hmac_secret, sign_internal_hmac};
use prost::Message;
use reqwest::StatusCode;
use tracing::warn;

use crate::directory::{CreateGroup, GroupDirectory, GroupError, GroupInfo};
use crate::inbox::parse_kind;
use crate::royal_pool::RoyalPool;
use crate::social::{FriendRequestOutcome, SocialDirectory, SocialError};
use crate::store::{
    Fanout, HistoryEntry, InboxEntry, InsertMessage, InsertResult, MessageContentRow,
    MessageIndexRow, MessageKind, MessageStore, StoreError,
};
use crate::users::{ProfilePatch, UserDirectory, UserError, UserProfile};

pub(crate) const RETRIES: usize = 3;
const PER_ATTEMPT: Duration = Duration::from_millis(400);
pub(crate) const DEFAULT_ATTEMPT: Duration = Duration::from_secs(4);
const CIRCUIT_FAILS: u32 = 5;
const HALF_OPEN_MS: u64 = 30_000;

tokio::task_local! {
    static RPC_DEADLINE: Instant;
}

pub(crate) async fn with_rpc_deadline<F, T>(budget: Duration, fut: F) -> Result<T, ()>
where
    F: std::future::Future<Output = T>,
{
    let deadline = Instant::now() + budget;
    match tokio::time::timeout(budget, RPC_DEADLINE.scope(deadline, fut)).await {
        Ok(v) => Ok(v),
        Err(_) => Err(()),
    }
}

fn attempt_timeout() -> Duration {
    RPC_DEADLINE
        .try_with(|d| d.saturating_duration_since(Instant::now()).min(PER_ATTEMPT))
        .unwrap_or(DEFAULT_ATTEMPT)
}

async fn backoff(attempt: usize) {
    let shift = u32::try_from(attempt.min(2)).unwrap_or(2);
    let base = 100u64.saturating_mul(1u64 << shift);
    let jitter = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_millis() % 50))
        .unwrap_or(0);
    let sleep = Duration::from_millis(base.saturating_add(jitter));
    let still = RPC_DEADLINE
        .try_with(|d| Instant::now() + sleep < *d)
        .unwrap_or(true);
    if still {
        tokio::time::sleep(sleep).await;
    }
}

fn fanout_from_resp(fanout: Option<InsertFanout>) -> Fanout {
    match fanout {
        Some(f) => {
            let kind = match parse_kind(f.kind) {
                Some(k) => k,
                None => {
                    warn!(kind = f.kind, "royal insert fanout unknown kind");
                    MessageKind::User
                }
            };
            Fanout {
                msg_type: f.r#type,
                body: f.body,
                extra: f.extra,
                sender: f.sender,
                dest: f.dest,
                kind,
                recipients: f.recipients,
            }
        }
        None => {
            warn!("royal insert resp missing fanout");
            Fanout {
                msg_type: 0,
                body: String::new(),
                extra: String::new(),
                sender: String::new(),
                dest: String::new(),
                kind: MessageKind::User,
                recipients: Vec::new(),
            }
        }
    }
}

fn http_status_err(status: StatusCode, body: &[u8]) -> StoreError {
    StoreError::Http {
        status: status.as_u16(),
        msg: String::from_utf8_lossy(body).into_owned(),
    }
}

pub(crate) fn retry_http(status: StatusCode) -> bool {
    status.is_server_error() && status != StatusCode::SERVICE_UNAVAILABLE
}

pub(crate) fn circuit_failure_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 500 | 502 | 503 | 504)
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[derive(Clone)]
pub struct RoyalClient {
    pub(crate) base: String,
    http: reqwest::Client,
    hmac_secret: String,
    pub(crate) fails: Arc<AtomicU32>,
    pub(crate) opened: Arc<AtomicBool>,
    pub(crate) half_open_at: Arc<AtomicU64>,
}

impl RoyalClient {
    pub fn new(base: &str) -> Result<Self, StoreError> {
        Self::with_hmac(base, &resolve_internal_hmac_secret(""))
    }

    pub fn with_hmac(base: &str, hmac_secret: &str) -> Result<Self, StoreError> {
        let http = reqwest::Client::builder()
            .timeout(DEFAULT_ATTEMPT)
            .build()
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            http,
            hmac_secret: hmac_secret.to_string(),
            fails: Arc::new(AtomicU32::new(0)),
            opened: Arc::new(AtomicBool::new(false)),
            half_open_at: Arc::new(AtomicU64::new(0)),
        })
    }

    pub(crate) fn is_open(&self) -> bool {
        self.opened.load(Ordering::SeqCst)
    }

    pub(crate) fn try_probe(&self) -> bool {
        if !self.opened.load(Ordering::SeqCst) {
            return true;
        }
        let now = unix_ms();
        let gate = self.half_open_at.load(Ordering::SeqCst);
        if now < gate {
            return false;
        }
        self.half_open_at
            .compare_exchange(
                gate,
                now.saturating_add(HALF_OPEN_MS),
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
    }

    pub(crate) fn report_success(&self) {
        self.fails.store(0, Ordering::SeqCst);
        self.opened.store(false, Ordering::SeqCst);
    }

    pub(crate) fn report_failure(&self) {
        let n = self.fails.fetch_add(1, Ordering::SeqCst).saturating_add(1);
        if n >= CIRCUIT_FAILS {
            self.opened.store(true, Ordering::SeqCst);
            self.half_open_at
                .store(unix_ms().saturating_add(HALF_OPEN_MS), Ordering::SeqCst);
        }
    }

    pub(crate) fn signed(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &[u8],
    ) -> Result<reqwest::RequestBuilder, StoreError> {
        let url = format!("{}{path}", self.base);
        let headers = sign_internal_hmac(self.hmac_secret.as_bytes(), method.as_str(), path, body)
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        let mut req = self
            .http
            .request(method, &url)
            .header("Content-Type", "application/x-protobuf")
            .header("Accept", "application/x-protobuf");
        for (k, v) in headers.pairs() {
            req = req.header(k, v);
        }
        if !body.is_empty() {
            req = req.body(body.to_vec());
        }
        Ok(req)
    }
}

impl RoyalPool {
    pub(crate) async fn send_pb<T: Message + Default, B: Message>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, StoreError> {
        let bytes = body.map(|b| Bytes::from(b.encode_to_vec()));
        let payload = bytes.as_deref().unwrap_or(&[]);
        let mut last = StoreError::Backend("royal request failed".into());
        for attempt in 0..RETRIES {
            if attempt_timeout().is_zero() {
                break;
            }
            let client = self.pick()?;
            let req = client
                .signed(method.clone(), path, payload)?
                .timeout(attempt_timeout());
            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let buf = resp
                        .bytes()
                        .await
                        .map_err(|e| StoreError::Backend(e.to_string()))?;
                    if status.is_success() {
                        self.report_success(&client);
                        return T::decode(buf.as_ref())
                            .map_err(|e| StoreError::Backend(e.to_string()));
                    }
                    last = http_status_err(status, &buf);
                    if circuit_failure_status(status) {
                        self.report_failure(&client);
                    } else {
                        self.report_success(&client);
                    }
                    if !retry_http(status) {
                        return Err(last);
                    }
                }
                Err(err) => {
                    self.report_failure(&client);
                    last = StoreError::Backend(err.to_string());
                }
            }
            if attempt + 1 < RETRIES {
                backoff(attempt).await;
            }
        }
        Err(last)
    }

    /// Decode-empty-tolerant POST for ack/join/quit.
    pub(crate) async fn post_maybe_empty(
        &self,
        path: &str,
        body: &impl Message,
    ) -> Result<(), StoreError> {
        let bytes = Bytes::from(body.encode_to_vec());
        let mut last = StoreError::Backend("royal request failed".into());
        for attempt in 0..RETRIES {
            if attempt_timeout().is_zero() {
                break;
            }
            let client = self.pick()?;
            match client
                .signed(reqwest::Method::POST, path, &bytes)?
                .timeout(attempt_timeout())
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    let buf = resp.bytes().await.unwrap_or_default();
                    if status.is_success() {
                        self.report_success(&client);
                        return Ok(());
                    }
                    last = http_status_err(status, &buf);
                    if circuit_failure_status(status) {
                        self.report_failure(&client);
                    } else {
                        self.report_success(&client);
                    }
                    if !retry_http(status) {
                        return Err(last);
                    }
                }
                Err(err) => {
                    self.report_failure(&client);
                    last = StoreError::Backend(err.to_string());
                }
            }
            if attempt + 1 < RETRIES {
                backoff(attempt).await;
            }
        }
        Err(last)
    }
}

pub struct HttpMessageStore {
    pool: Arc<RoyalPool>,
    pending_receipt: bool,
}

impl HttpMessageStore {
    pub fn new(base: &str) -> Result<Self, StoreError> {
        Ok(Self {
            pool: Arc::new(RoyalPool::new(
                Some(base),
                None,
                &resolve_internal_hmac_secret(""),
            )?),
            pending_receipt: crate::store::pending_receipt_enabled(),
        })
    }
}

#[async_trait]
impl MessageStore for HttpMessageStore {
    async fn insert_user(
        &self,
        app: &str,
        req: &InsertMessage,
    ) -> Result<InsertResult, StoreError> {
        let body = InsertMessageReq {
            sender: req.sender.clone(),
            dest: req.dest.clone(),
            send_time: req.send_time,
            message: Some(MessageReq {
                r#type: req.msg_type,
                body: req.body.clone(),
                extra: req.extra.clone(),
                client_id: req.client_id.clone(),
            }),
            members: Vec::new(),
            client_id: req.client_id.clone(),
            online_targets: req
                .online_targets
                .iter()
                .map(|t| PbDeliveryTarget {
                    account: t.account.clone(),
                    target_id: t.target_id.clone(),
                })
                .collect(),
        };
        let _ = app;
        let path = "/api/v1/message/user";
        let resp: InsertMessageResp = self
            .pool
            .send_pb(reqwest::Method::POST, path, Some(&body))
            .await?;
        Ok(InsertResult {
            message_id: resp.message_id,
            send_time: resp.send_time,
            duplicate: resp.duplicate,
            fanout: fanout_from_resp(resp.fanout),
        })
    }

    async fn insert_group(
        &self,
        app: &str,
        req: &InsertMessage,
        members: &[String],
    ) -> Result<InsertResult, StoreError> {
        let body = InsertMessageReq {
            sender: req.sender.clone(),
            dest: req.dest.clone(),
            send_time: req.send_time,
            message: Some(MessageReq {
                r#type: req.msg_type,
                body: req.body.clone(),
                extra: req.extra.clone(),
                client_id: req.client_id.clone(),
            }),
            members: members.to_vec(),
            client_id: req.client_id.clone(),
            online_targets: req
                .online_targets
                .iter()
                .map(|t| PbDeliveryTarget {
                    account: t.account.clone(),
                    target_id: t.target_id.clone(),
                })
                .collect(),
        };
        let _ = app;
        let path = "/api/v1/message/group";
        let resp: InsertMessageResp = self
            .pool
            .send_pb(reqwest::Method::POST, path, Some(&body))
            .await?;
        Ok(InsertResult {
            message_id: resp.message_id,
            send_time: resp.send_time,
            duplicate: resp.duplicate,
            fanout: fanout_from_resp(resp.fanout),
        })
    }

    async fn ack(
        &self,
        app: &str,
        account: &str,
        target_id: &str,
        message_ids: &[i64],
    ) -> Result<(), StoreError> {
        let body = if self.pending_receipt {
            AckMessageReq {
                account: account.to_string(),
                message_id: 0,
                message_ids: message_ids.to_vec(),
                target_id: target_id.to_string(),
                app: app.to_string(),
            }
        } else {
            AckMessageReq {
                account: account.to_string(),
                message_id: message_ids.first().copied().unwrap_or(0),
                ..Default::default()
            }
        };
        self.pool
            .post_maybe_empty("/api/v1/message/ack", &body)
            .await
    }

    async fn offline_index(
        &self,
        app: &str,
        account: &str,
        target_id: &str,
        message_id: i64,
        resume: bool,
    ) -> Result<(Vec<MessageIndexRow>, bool), StoreError> {
        let body = if self.pending_receipt {
            OfflineIndexReq {
                account: account.to_string(),
                message_id,
                target_id: target_id.to_string(),
                app: app.to_string(),
                resume,
            }
        } else {
            OfflineIndexReq {
                account: account.to_string(),
                message_id,
                ..Default::default()
            }
        };
        let path = "/api/v1/offline/index";
        let resp: MessageIndexResp = self
            .pool
            .send_pb(reqwest::Method::POST, path, Some(&body))
            .await?;
        Ok((
            resp.indexes
                .into_iter()
                .map(|r| MessageIndexRow {
                    message_id: r.message_id,
                    direction: r.direction,
                    send_time: r.send_time,
                    account_b: r.account_b,
                    group: r.group,
                })
                .collect(),
            resp.has_more,
        ))
    }

    async fn backfill_delivery(
        &self,
        app: &str,
        account: &str,
        target_id: &str,
    ) -> Result<(), StoreError> {
        if !self.pending_receipt {
            return Ok(());
        }
        let body = DeliveryBackfillReq {
            app: app.to_string(),
            account: account.to_string(),
            target_id: target_id.to_string(),
        };
        self.pool
            .post_maybe_empty("/api/v1/delivery/backfill", &body)
            .await
    }

    async fn offline_content(
        &self,
        app: &str,
        account: &str,
        message_ids: &[i64],
    ) -> Result<Vec<MessageContentRow>, StoreError> {
        let body = MessageContentReq {
            message_ids: message_ids.to_vec(),
            account: account.to_string(),
            app: app.to_string(),
        };
        let path = "/api/v1/offline/content";
        let resp: MessageContentResp = self
            .pool
            .send_pb(reqwest::Method::POST, path, Some(&body))
            .await?;
        Ok(resp
            .messages
            .into_iter()
            .map(|m| MessageContentRow {
                message_id: m.message_id,
                msg_type: m.r#type,
                body: m.body,
                extra: m.extra,
            })
            .collect())
    }

    async fn inbox(
        &self,
        _app: &str,
        account: &str,
        limit: i32,
    ) -> Result<Vec<InboxEntry>, StoreError> {
        let body = InboxQuery {
            account: account.to_string(),
            limit,
        };
        let resp: InboxResp = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/inbox", Some(&body))
            .await?;
        Ok(resp
            .items
            .into_iter()
            .filter_map(|i| {
                let kind = parse_kind(i.kind)?;
                Some(InboxEntry {
                    dest: i.dest,
                    kind,
                    last_message_id: i.last_message_id,
                    last_send_time: i.last_send_time,
                    last_body: i.last_body,
                    last_sender: i.last_sender,
                    last_msg_type: 0,
                    unread: i.unread,
                })
            })
            .collect())
    }

    async fn history(
        &self,
        _app: &str,
        account: &str,
        dest: &str,
        kind: MessageKind,
        before_id: i64,
        limit: i32,
    ) -> Result<Vec<HistoryEntry>, StoreError> {
        let body = HistoryQuery {
            account: account.to_string(),
            dest: dest.to_string(),
            kind: match kind {
                MessageKind::User => 0,
                MessageKind::Group => 1,
            },
            before_id,
            limit,
        };
        let resp: HistoryResp = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/history", Some(&body))
            .await?;
        Ok(resp
            .messages
            .into_iter()
            .map(|m| HistoryEntry {
                message_id: m.message_id,
                msg_type: m.r#type,
                body: m.body,
                extra: m.extra,
                sender: m.sender,
                send_time: m.send_time,
                direction: m.direction,
            })
            .collect())
    }

    async fn mark_read(
        &self,
        _app: &str,
        account: &str,
        dest: &str,
        kind: MessageKind,
        message_id: i64,
    ) -> Result<(), StoreError> {
        let body = ConversationRead {
            account: account.to_string(),
            dest: dest.to_string(),
            kind: match kind {
                MessageKind::User => 0,
                MessageKind::Group => 1,
            },
            message_id,
        };
        self.pool
            .post_maybe_empty("/api/v1/inbox/read", &body)
            .await
    }
}

pub struct HttpGroupDirectory {
    pool: Arc<RoyalPool>,
}

impl HttpGroupDirectory {
    pub fn new(base: &str) -> Result<Self, GroupError> {
        Ok(Self {
            pool: Arc::new(
                RoyalPool::new(Some(base), None, &resolve_internal_hmac_secret(""))
                    .map_err(|e| GroupError::Backend(e.to_string()))?,
            ),
        })
    }
}

fn group_err(e: StoreError) -> GroupError {
    match e {
        StoreError::Http { status: 404, .. } => GroupError::NotFound,
        StoreError::Http { status, msg } => {
            GroupError::Backend(format!("royal http {status}: {msg}"))
        }
        StoreError::Backend(s) => GroupError::Backend(s),
        StoreError::Id(e) => GroupError::Id(e),
    }
}

#[async_trait]
impl GroupDirectory for HttpGroupDirectory {
    async fn create(&self, app: &str, req: &CreateGroup) -> Result<String, GroupError> {
        let body = InternalGroupCreate {
            app: app.to_string(),
            name: req.name.clone(),
            avatar: req.avatar.clone(),
            introduction: req.introduction.clone(),
            owner: req.owner.clone(),
            members: req.members.clone(),
        };
        let path = "/api/v1/group";
        let resp: GroupCreateResp = self
            .pool
            .send_pb(reqwest::Method::POST, path, Some(&body))
            .await
            .map_err(group_err)?;
        Ok(resp.group_id)
    }

    async fn members(&self, app: &str, group_id: &str) -> Result<Vec<String>, GroupError> {
        let body = InternalGroupQuery {
            app: app.to_string(),
            group_id: group_id.to_string(),
        };
        let resp: GroupMembersResp = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/group/members", Some(&body))
            .await
            .map_err(group_err)?;
        Ok(resp.members)
    }

    async fn join(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError> {
        let body = InternalGroupMember {
            app: app.to_string(),
            group_id: group_id.to_string(),
            account: account.to_string(),
        };
        self.pool
            .post_maybe_empty("/api/v1/group/member", &body)
            .await
            .map_err(group_err)
    }

    async fn quit(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError> {
        let body = InternalGroupMember {
            app: app.to_string(),
            group_id: group_id.to_string(),
            account: account.to_string(),
        };
        self.pool
            .post_maybe_empty("/api/v1/group/quit", &body)
            .await
            .map_err(group_err)
    }

    async fn detail(&self, app: &str, group_id: &str) -> Result<GroupInfo, GroupError> {
        let body = InternalGroupQuery {
            app: app.to_string(),
            group_id: group_id.to_string(),
        };
        let resp: GroupDetail = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/group/detail", Some(&body))
            .await
            .map_err(group_err)?;
        Ok(GroupInfo {
            id: resp.group_id,
            name: resp.name,
            avatar: resp.avatar,
            introduction: resp.introduction,
            owner: resp.owner,
            members: resp.members,
        })
    }
}

pub struct HttpUserDirectory {
    pool: Arc<RoyalPool>,
}

impl HttpUserDirectory {
    pub fn new(base: &str) -> Result<Self, StoreError> {
        Ok(Self {
            pool: Arc::new(RoyalPool::new(
                Some(base),
                None,
                &resolve_internal_hmac_secret(""),
            )?),
        })
    }
}

fn user_err(e: StoreError) -> UserError {
    UserError::Backend(e.to_string())
}

#[async_trait]
impl UserDirectory for HttpUserDirectory {
    async fn upsert(&self, _app: &str, account: &str) -> Result<(), UserError> {
        let body = AccountQuery {
            account: account.to_string(),
        };
        self.pool
            .post_maybe_empty("/internal/user/upsert", &body)
            .await
            .map_err(user_err)
    }

    async fn create(
        &self,
        _app: &str,
        _account: &str,
        _password_hash: &str,
    ) -> Result<(), UserError> {
        Err(UserError::Backend("create is royal-only".into()))
    }

    async fn password_hash(&self, _app: &str, _account: &str) -> Result<Option<String>, UserError> {
        Err(UserError::Backend("password_hash is royal-only".into()))
    }

    async fn exists(&self, _app: &str, account: &str) -> Result<bool, UserError> {
        let body = AccountQuery {
            account: account.to_string(),
        };
        let resp: AccountExists = self
            .pool
            .send_pb(reqwest::Method::POST, "/internal/user/lookup", Some(&body))
            .await
            .map_err(user_err)?;
        Ok(resp.exists)
    }

    async fn profile(&self, _app: &str, account: &str) -> Result<Option<UserProfile>, UserError> {
        let body = AccountQuery {
            account: account.to_string(),
        };
        match self
            .pool
            .send_pb::<PbProfile, _>(reqwest::Method::POST, "/api/v1/user/profile", Some(&body))
            .await
        {
            Ok(p) => Ok(Some(from_pb_profile(p))),
            Err(StoreError::Http { status: 404, .. }) => Ok(None),
            Err(e) => Err(user_err(e)),
        }
    }

    async fn update_profile(
        &self,
        _app: &str,
        account: &str,
        patch: &ProfilePatch,
    ) -> Result<UserProfile, UserError> {
        let body = ProfileUpdateReq {
            account: account.to_string(),
            nickname: patch.nickname.clone(),
            avatar: patch.avatar.clone(),
            bio: patch.bio.clone(),
        };
        let p: PbProfile = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/user/update", Some(&body))
            .await
            .map_err(user_err)?;
        Ok(from_pb_profile(p))
    }

    async fn profiles(
        &self,
        _app: &str,
        accounts: &[String],
    ) -> Result<Vec<UserProfile>, UserError> {
        let body = AccountList {
            accounts: accounts.to_vec(),
        };
        let resp: UserListResp = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/user/profiles", Some(&body))
            .await
            .map_err(user_err)?;
        Ok(resp.users.into_iter().map(from_pb_profile).collect())
    }

    async fn search(
        &self,
        _app: &str,
        query: &str,
        exclude: &[String],
        limit: usize,
    ) -> Result<Vec<UserProfile>, UserError> {
        let body = UserSearchQuery {
            query: query.to_string(),
            exclude: exclude.to_vec(),
            limit: i32::try_from(limit).unwrap_or(20),
        };
        let resp: UserSearchResp = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/user/search", Some(&body))
            .await
            .map_err(user_err)?;
        Ok(resp.users.into_iter().map(from_pb_profile).collect())
    }

    async fn set_password(
        &self,
        _app: &str,
        _account: &str,
        _password_hash: &str,
    ) -> Result<(), UserError> {
        Err(UserError::Backend("set_password is royal-only".into()))
    }

    async fn token_epoch(&self, _app: &str, _account: &str) -> Result<u32, UserError> {
        Err(UserError::Backend("token_epoch is royal-only".into()))
    }

    async fn bump_token_epoch(&self, _app: &str, _account: &str) -> Result<u32, UserError> {
        Err(UserError::Backend("bump_token_epoch is royal-only".into()))
    }

    async fn set_password_and_bump_epoch(
        &self,
        _app: &str,
        _account: &str,
        _password_hash: &str,
    ) -> Result<u32, UserError> {
        Err(UserError::Backend(
            "set_password_and_bump_epoch is royal-only".into(),
        ))
    }
}

fn from_pb_profile(p: PbProfile) -> UserProfile {
    UserProfile {
        account: p.account,
        nickname: p.nickname,
        avatar: p.avatar,
        bio: p.bio,
    }
}

pub struct HttpSocialDirectory {
    pool: Arc<RoyalPool>,
}

impl HttpSocialDirectory {
    pub fn new(base: &str) -> Result<Self, StoreError> {
        Ok(Self {
            pool: Arc::new(RoyalPool::new(
                Some(base),
                None,
                &resolve_internal_hmac_secret(""),
            )?),
        })
    }

    async fn pair_op(&self, path: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        let body = AccountPair {
            account: account.to_string(),
            peer: peer.to_string(),
        };
        self.pool
            .post_maybe_empty(path, &body)
            .await
            .map_err(social_err)
    }
}

fn social_err(e: StoreError) -> SocialError {
    match e {
        StoreError::Http { status: 403, .. } => SocialError::Blocked,
        StoreError::Http { status: 404, .. } => SocialError::NotFound,
        StoreError::Http { status: 400, .. } => SocialError::SelfOp,
        StoreError::Http { status, msg } => {
            SocialError::Backend(format!("royal http {status}: {msg}"))
        }
        other => SocialError::Backend(other.to_string()),
    }
}

#[async_trait]
impl SocialDirectory for HttpSocialDirectory {
    async fn request(
        &self,
        _app: &str,
        from: &str,
        to: &str,
    ) -> Result<FriendRequestOutcome, SocialError> {
        let body = AccountPair {
            account: from.to_string(),
            peer: to.to_string(),
        };
        let resp: AccountExists = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/friend/request", Some(&body))
            .await
            .map_err(social_err)?;
        Ok(if resp.exists {
            FriendRequestOutcome::AutoAccepted
        } else {
            FriendRequestOutcome::Sent
        })
    }

    async fn accept(&self, _app: &str, account: &str, from: &str) -> Result<(), SocialError> {
        self.pair_op("/api/v1/friend/accept", account, from).await
    }

    async fn reject(&self, _app: &str, account: &str, from: &str) -> Result<(), SocialError> {
        self.pair_op("/api/v1/friend/reject", account, from).await
    }

    async fn remove(&self, _app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        self.pair_op("/api/v1/friend/remove", account, peer).await
    }

    async fn list_friends(&self, _app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        let body = AccountQuery {
            account: account.to_string(),
        };
        let resp: AccountList = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/friend/list", Some(&body))
            .await
            .map_err(social_err)?;
        Ok(resp.accounts)
    }

    async fn incoming(&self, _app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        let body = AccountQuery {
            account: account.to_string(),
        };
        let resp: AccountList = self
            .pool
            .send_pb(
                reqwest::Method::POST,
                "/api/v1/friend/incoming",
                Some(&body),
            )
            .await
            .map_err(social_err)?;
        Ok(resp.accounts)
    }

    async fn is_friend(&self, _app: &str, a: &str, b: &str) -> Result<bool, SocialError> {
        let body = AccountPair {
            account: a.to_string(),
            peer: b.to_string(),
        };
        let resp: AccountExists = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/friend/check", Some(&body))
            .await
            .map_err(social_err)?;
        Ok(resp.exists)
    }

    async fn block(&self, _app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        self.pair_op("/api/v1/block/add", account, peer).await
    }

    async fn unblock(&self, _app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        self.pair_op("/api/v1/block/remove", account, peer).await
    }

    async fn list_blocked(&self, _app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        let body = AccountQuery {
            account: account.to_string(),
        };
        let resp: AccountList = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/block/list", Some(&body))
            .await
            .map_err(social_err)?;
        Ok(resp.accounts)
    }

    async fn is_blocked_either(&self, _app: &str, a: &str, b: &str) -> Result<bool, SocialError> {
        let body = AccountPair {
            account: a.to_string(),
            peer: b.to_string(),
        };
        let resp: AccountExists = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/block/check", Some(&body))
            .await
            .map_err(social_err)?;
        Ok(resp.exists)
    }
}

pub type HttpBackends = (
    Arc<dyn MessageStore>,
    Arc<dyn GroupDirectory>,
    Arc<dyn UserDirectory>,
    Arc<dyn SocialDirectory>,
);

pub fn http_backends(royal_url: &str) -> Result<HttpBackends, StoreError> {
    http_backends_with_hmac(royal_url, &resolve_internal_hmac_secret(""))
}

pub fn http_backends_with_hmac(
    royal_url: &str,
    hmac_secret: &str,
) -> Result<HttpBackends, StoreError> {
    http_backends_with_hmac_receipt(
        royal_url,
        hmac_secret,
        crate::store::pending_receipt_enabled(),
    )
}

pub fn http_backends_with_hmac_receipt(
    royal_url: &str,
    hmac_secret: &str,
    pending_receipt: bool,
) -> Result<HttpBackends, StoreError> {
    let pool = Arc::new(RoyalPool::new(Some(royal_url), None, hmac_secret)?);
    http_backends_with_pool_receipt(pool, pending_receipt)
}

pub fn http_backends_with_pool(pool: Arc<RoyalPool>) -> Result<HttpBackends, StoreError> {
    http_backends_with_pool_receipt(pool, crate::store::pending_receipt_enabled())
}

pub fn http_backends_with_pool_receipt(
    pool: Arc<RoyalPool>,
    pending_receipt: bool,
) -> Result<HttpBackends, StoreError> {
    Ok((
        Arc::new(HttpMessageStore {
            pool: pool.clone(),
            pending_receipt,
        }),
        Arc::new(HttpGroupDirectory { pool: pool.clone() }),
        Arc::new(HttpUserDirectory { pool: pool.clone() }),
        Arc::new(HttpSocialDirectory { pool }),
    ))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::routing::post;
    use axum::Router;

    use super::*;
    use crate::directory::GroupError;

    #[test]
    fn group_err_maps_http_404_without_string_match() {
        match group_err(StoreError::Http {
            status: 404,
            msg: "gone".into(),
        }) {
            GroupError::NotFound => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
        match group_err(StoreError::Http {
            status: 500,
            msg: "boom".into(),
        }) {
            GroupError::Backend(s) => assert!(s.contains("500"), "{s}"),
            other => panic!("expected Backend, got {other:?}"),
        }
        match group_err(StoreError::Backend("dial tcp".into())) {
            GroupError::Backend(s) => assert_eq!(s, "dial tcp"),
            other => panic!("expected Backend, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn http_group_directory_404_is_not_found() {
        async fn not_found() -> StatusCode {
            StatusCode::NOT_FOUND
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route("/api/v1/group/detail", post(not_found));
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let dir = HttpGroupDirectory::new(&format!("http://{addr}")).unwrap();
        match dir.detail("kim", "g1").await {
            Err(GroupError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn http_group_directory_500_and_network_are_backend() {
        async fn boom() -> StatusCode {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route("/api/v1/group/detail", post(boom));
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let dir = HttpGroupDirectory::new(&format!("http://{addr}")).unwrap();
        match dir.detail("kim", "g1").await {
            Err(GroupError::Backend(_)) => {}
            other => panic!("expected Backend, got {other:?}"),
        }

        let closed = HttpGroupDirectory::new("http://127.0.0.1:1").unwrap();
        match closed.detail("kim", "g1").await {
            Err(GroupError::Backend(_)) => {}
            other => panic!("expected Backend, got {other:?}"),
        }
    }

    #[test]
    fn service_unavailable_is_not_retried() {
        assert!(!retry_http(StatusCode::SERVICE_UNAVAILABLE));
        assert!(retry_http(StatusCode::INTERNAL_SERVER_ERROR));
    }
}
