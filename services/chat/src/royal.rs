//! HTTP adapters that send Chat store/directory calls to royal.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_protocol::pkt::{
    AckMessageReq, GroupCreateReq, GroupCreateResp, GroupDetail, GroupJoinReq, GroupMembersResp,
    GroupQuitReq, InsertMessageReq, InsertMessageResp, MessageContentReq, MessageContentResp,
    MessageIndexResp, MessageReq, OfflineIndexReq,
};
use prost::Message;
use reqwest::StatusCode;

use crate::directory::{CreateGroup, GroupDirectory, GroupError, GroupInfo};
use crate::store::{
    InsertMessage, InsertResult, MessageContentRow, MessageIndexRow, MessageStore, StoreError,
};

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

async fn delete_maybe_empty(
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
            .request(reqwest::Method::DELETE, &url)
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
            }),
            members: Vec::new(),
        };
        let path = format!("/api/{app}/message/user");
        let resp: InsertMessageResp = self
            .client
            .send_pb(reqwest::Method::POST, &path, Some(&body))
            .await?;
        Ok(InsertResult {
            message_id: resp.message_id,
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
            }),
            members: members.to_vec(),
        };
        let path = format!("/api/{app}/message/group");
        let resp: InsertMessageResp = self
            .client
            .send_pb(reqwest::Method::POST, &path, Some(&body))
            .await?;
        Ok(InsertResult {
            message_id: resp.message_id,
        })
    }

    async fn ack(&self, app: &str, account: &str, message_id: i64) -> Result<(), StoreError> {
        let body = AckMessageReq {
            account: account.to_string(),
            message_id,
        };
        post_maybe_empty(&self.client, &format!("/api/{app}/message/ack"), &body).await
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
        let path = format!("/api/{app}/offline/index");
        let resp: MessageIndexResp = self
            .client
            .send_pb(reqwest::Method::POST, &path, Some(&body))
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
        let path = format!("/api/{app}/offline/content");
        let resp: MessageContentResp = self
            .client
            .send_pb(reqwest::Method::POST, &path, Some(&body))
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
        let path = format!("/api/{app}/group");
        let resp: GroupCreateResp = self
            .client
            .send_pb(reqwest::Method::POST, &path, Some(&body))
            .await
            .map_err(group_err)?;
        Ok(resp.group_id)
    }

    async fn members(&self, app: &str, group_id: &str) -> Result<Vec<String>, GroupError> {
        let path = format!("/api/{app}/group/members/{group_id}");
        let resp: GroupMembersResp = self
            .client
            .send_pb::<GroupMembersResp, GroupCreateReq>(reqwest::Method::GET, &path, None)
            .await
            .map_err(group_err)?;
        Ok(resp.members)
    }

    async fn join(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError> {
        let body = GroupJoinReq {
            account: account.to_string(),
            group_id: group_id.to_string(),
        };
        post_maybe_empty(&self.client, &format!("/api/{app}/group/member"), &body)
            .await
            .map_err(group_err)
    }

    async fn quit(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError> {
        let body = GroupQuitReq {
            account: account.to_string(),
            group_id: group_id.to_string(),
        };
        delete_maybe_empty(&self.client, &format!("/api/{app}/group/member"), &body)
            .await
            .map_err(group_err)
    }

    async fn detail(&self, app: &str, group_id: &str) -> Result<GroupInfo, GroupError> {
        let path = format!("/api/{app}/group/{group_id}");
        let resp: GroupDetail = self
            .client
            .send_pb::<GroupDetail, GroupCreateReq>(reqwest::Method::GET, &path, None)
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

pub type HttpBackends = (Arc<dyn MessageStore>, Arc<dyn GroupDirectory>);

pub fn http_backends(royal_url: &str) -> Result<HttpBackends, StoreError> {
    Ok((
        Arc::new(HttpMessageStore::new(royal_url)?),
        Arc::new(
            HttpGroupDirectory::new(royal_url).map_err(|e| StoreError::Backend(e.to_string()))?,
        ),
    ))
}
