//! HTTP adapters that send Chat store/directory calls to royal.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_protocol::pkt::{
    AccountExists, AccountQuery, AckMessageReq, GroupCreateReq, GroupCreateResp, GroupDetail,
    GroupJoinReq, GroupMembersResp, GroupQueryReq, GroupQuitReq, InsertMessageReq,
    InsertMessageResp, MessageContentReq, MessageContentResp, MessageIndexResp, MessageReq,
    OfflineIndexReq,
};
use prost::Message;
use reqwest::StatusCode;

use crate::directory::{CreateGroup, GroupDirectory, GroupError, GroupInfo};
use crate::store::{
    InsertMessage, InsertResult, MessageContentRow, MessageIndexRow, MessageStore, StoreError,
};
use crate::users::{UserDirectory, UserError};

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
}

pub type HttpBackends = (
    Arc<dyn MessageStore>,
    Arc<dyn GroupDirectory>,
    Arc<dyn UserDirectory>,
);

pub fn http_backends(royal_url: &str) -> Result<HttpBackends, StoreError> {
    Ok((
        Arc::new(HttpMessageStore::new(royal_url)?),
        Arc::new(
            HttpGroupDirectory::new(royal_url).map_err(|e| StoreError::Backend(e.to_string()))?,
        ),
        Arc::new(HttpUserDirectory::new(royal_url)?),
    ))
}
