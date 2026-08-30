//! HTTP adapters that send Chat store/directory calls to royal.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_protocol::pkt::{
    AccountExists, AccountList, AccountPair, AccountQuery, AckMessageReq, ConversationRead,
    GroupCreateReq, GroupCreateResp, GroupDetail, GroupJoinReq, GroupMembersResp, GroupQueryReq,
    GroupQuitReq, HistoryQuery, HistoryResp, InboxQuery, InboxResp, InsertMessageReq,
    InsertMessageResp, MessageContentReq, MessageContentResp, MessageIndexResp, MessageReq,
    OfflineIndexReq, ProfileUpdateReq, UserListResp, UserProfile as PbProfile, UserSearchQuery,
    UserSearchResp,
};
use prost::Message;
use reqwest::StatusCode;

use crate::directory::{CreateGroup, GroupDirectory, GroupError, GroupInfo};
use crate::inbox::parse_kind;
use crate::social::{FriendRequestOutcome, SocialDirectory, SocialError};
use crate::store::{
    HistoryEntry, InboxEntry, InsertMessage, InsertResult, MessageContentRow, MessageIndexRow,
    MessageKind, MessageStore, StoreError,
};
use crate::users::{ProfilePatch, UserDirectory, UserError, UserProfile};

const RETRIES: usize = 3;

#[derive(Clone)]
pub struct RoyalClient {
    base: String,
    http: reqwest::Client,
}

impl RoyalClient {
    pub fn new(base: &str) -> Result<Self, StoreError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            http,
        })
    }

    async fn send_pb<T: Message + Default, B: Message>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, StoreError> {
        let url = format!("{}{path}", self.base);
        let bytes = body.map(|b| Bytes::from(b.encode_to_vec()));
        let mut last = StoreError::Backend("royal request failed".into());
        for _ in 0..RETRIES {
            let mut req = self
                .http
                .request(method.clone(), &url)
                .header("Content-Type", "application/x-protobuf")
                .header("Accept", "application/x-protobuf");
            if let Some(b) = bytes.clone() {
                req = req.body(b);
            }
            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let buf = resp
                        .bytes()
                        .await
                        .map_err(|e| StoreError::Backend(e.to_string()))?;
                    if !status.is_success() {
                        last = StoreError::Backend(format!("royal http {status}"));
                        if status == StatusCode::BAD_REQUEST {
                            return Err(last);
                        }
                        continue;
                    }
                    return T::decode(buf.as_ref()).map_err(|e| StoreError::Backend(e.to_string()));
                }
                Err(err) => last = StoreError::Backend(err.to_string()),
            }
        }
        Err(last)
    }
}

/// Decode-empty-tolerant POST for ack/join/quit.
async fn post_maybe_empty(
    client: &RoyalClient,
    path: &str,
    body: &impl Message,
) -> Result<(), StoreError> {
    let url = format!("{}{path}", client.base);
    let bytes = Bytes::from(body.encode_to_vec());
    let mut last = StoreError::Backend("royal request failed".into());
    for _ in 0..RETRIES {
        match client
            .http
            .post(&url)
            .header("Content-Type", "application/x-protobuf")
            .header("Accept", "application/x-protobuf")
            .body(bytes.clone())
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let _ = resp.bytes().await;
                if status.is_success() {
                    return Ok(());
                }
                last = StoreError::Backend(format!("royal http {status}"));
                if status == StatusCode::BAD_REQUEST {
                    return Err(last);
                }
            }
            Err(err) => last = StoreError::Backend(err.to_string()),
        }
    }
    Err(last)
}

pub struct HttpMessageStore {
    client: RoyalClient,
}

impl HttpMessageStore {
    pub fn new(base: &str) -> Result<Self, StoreError> {
        Ok(Self {
            client: RoyalClient::new(base)?,
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
        };
        let _ = app;
        let path = "/api/v1/message/user";
        let resp: InsertMessageResp = self
            .client
            .send_pb(reqwest::Method::POST, path, Some(&body))
            .await?;
        Ok(InsertResult {
            message_id: resp.message_id,
            send_time: resp.send_time,
            duplicate: resp.duplicate,
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
        };
        let _ = app;
        let path = "/api/v1/message/group";
        let resp: InsertMessageResp = self
            .client
            .send_pb(reqwest::Method::POST, path, Some(&body))
            .await?;
        Ok(InsertResult {
            message_id: resp.message_id,
            send_time: resp.send_time,
            duplicate: resp.duplicate,
        })
    }

    async fn ack(&self, app: &str, account: &str, message_id: i64) -> Result<(), StoreError> {
        let body = AckMessageReq {
            account: account.to_string(),
            message_id,
        };
        let _ = app;
        post_maybe_empty(&self.client, "/api/v1/message/ack", &body).await
    }

    async fn offline_index(
        &self,
        app: &str,
        account: &str,
        message_id: i64,
    ) -> Result<Vec<MessageIndexRow>, StoreError> {
        let body = OfflineIndexReq {
            account: account.to_string(),
            message_id,
        };
        let _ = app;
        let path = "/api/v1/offline/index";
        let resp: MessageIndexResp = self
            .client
            .send_pb(reqwest::Method::POST, path, Some(&body))
            .await?;
        Ok(resp
            .indexes
            .into_iter()
            .map(|r| MessageIndexRow {
                message_id: r.message_id,
                direction: r.direction,
                send_time: r.send_time,
                account_b: r.account_b,
                group: r.group,
            })
            .collect())
    }

    async fn offline_content(
        &self,
        app: &str,
        message_ids: &[i64],
    ) -> Result<Vec<MessageContentRow>, StoreError> {
        let body = MessageContentReq {
            message_ids: message_ids.to_vec(),
        };
        let _ = app;
        let path = "/api/v1/offline/content";
        let resp: MessageContentResp = self
            .client
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
            .client
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
            .client
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
        post_maybe_empty(&self.client, "/api/v1/inbox/read", &body).await
    }
}

pub struct HttpGroupDirectory {
    client: RoyalClient,
}

impl HttpGroupDirectory {
    pub fn new(base: &str) -> Result<Self, GroupError> {
        Ok(Self {
            client: RoyalClient::new(base).map_err(|e| GroupError::Backend(e.to_string()))?,
        })
    }
}

fn group_err(e: StoreError) -> GroupError {
    GroupError::Backend(e.to_string())
}

#[async_trait]
impl GroupDirectory for HttpGroupDirectory {
    async fn create(&self, app: &str, req: &CreateGroup) -> Result<String, GroupError> {
        let body = GroupCreateReq {
            name: req.name.clone(),
            avatar: req.avatar.clone(),
            introduction: req.introduction.clone(),
            owner: req.owner.clone(),
            members: req.members.clone(),
        };
        let _ = app;
        let path = "/api/v1/group";
        let resp: GroupCreateResp = self
            .client
            .send_pb(reqwest::Method::POST, path, Some(&body))
            .await
            .map_err(group_err)?;
        Ok(resp.group_id)
    }

    async fn members(&self, app: &str, group_id: &str) -> Result<Vec<String>, GroupError> {
        let _ = app;
        let body = GroupQueryReq {
            group_id: group_id.to_string(),
        };
        let resp: GroupMembersResp = self
            .client
            .send_pb(reqwest::Method::POST, "/api/v1/group/members", Some(&body))
            .await
            .map_err(group_err)?;
        Ok(resp.members)
    }

    async fn join(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError> {
        let body = GroupJoinReq {
            account: account.to_string(),
            group_id: group_id.to_string(),
        };
        let _ = app;
        post_maybe_empty(&self.client, "/api/v1/group/member", &body)
            .await
            .map_err(group_err)
    }

    async fn quit(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError> {
        let body = GroupQuitReq {
            account: account.to_string(),
            group_id: group_id.to_string(),
        };
        let _ = app;
        post_maybe_empty(&self.client, "/api/v1/group/quit", &body)
            .await
            .map_err(group_err)
    }

    async fn detail(&self, app: &str, group_id: &str) -> Result<GroupInfo, GroupError> {
        let _ = app;
        let body = GroupQueryReq {
            group_id: group_id.to_string(),
        };
        let resp: GroupDetail = self
            .client
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
    client: RoyalClient,
}

impl HttpUserDirectory {
    pub fn new(base: &str) -> Result<Self, StoreError> {
        Ok(Self {
            client: RoyalClient::new(base)?,
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
        post_maybe_empty(&self.client, "/internal/user/upsert", &body)
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
            .client
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
            .client
            .send_pb::<PbProfile, _>(reqwest::Method::POST, "/api/v1/user/profile", Some(&body))
            .await
        {
            Ok(p) => Ok(Some(from_pb_profile(p))),
            Err(e) if e.to_string().contains("404") => Ok(None),
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
            .client
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
            .client
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
            .client
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
    client: RoyalClient,
}

impl HttpSocialDirectory {
    pub fn new(base: &str) -> Result<Self, StoreError> {
        Ok(Self {
            client: RoyalClient::new(base)?,
        })
    }

    async fn pair_op(&self, path: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        let body = AccountPair {
            account: account.to_string(),
            peer: peer.to_string(),
        };
        post_social(&self.client, path, &body).await
    }
}

fn social_err(e: StoreError) -> SocialError {
    let s = e.to_string();
    if s.contains("403") {
        SocialError::Blocked
    } else if s.contains("404") {
        SocialError::NotFound
    } else if s.contains("400") {
        SocialError::SelfOp
    } else {
        SocialError::Backend(s)
    }
}

async fn post_social(
    client: &RoyalClient,
    path: &str,
    body: &impl Message,
) -> Result<(), SocialError> {
    post_maybe_empty(client, path, body)
        .await
        .map_err(social_err)
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
    Ok((
        Arc::new(HttpMessageStore::new(royal_url)?),
        Arc::new(
            HttpGroupDirectory::new(royal_url).map_err(|e| StoreError::Backend(e.to_string()))?,
        ),
        Arc::new(HttpUserDirectory::new(royal_url)?),
        Arc::new(HttpSocialDirectory::new(royal_url)?),
    ))
}
