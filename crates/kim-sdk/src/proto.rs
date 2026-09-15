//! Protocol client trait. Impl for `KimClient` lives here (orphan rule).

use kim_client::{KimClient, OutgoingContent};

use crate::error::{map_client, SdkError};

#[async_trait::async_trait]
pub trait ProtocolClient: Send + Sync {
    async fn send_message(
        &self,
        dest: &str,
        kind: i32,
        body: &str,
        extra: &str,
        payload_type: i32,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError>;

    async fn ack(&self, message_id: i64) -> Result<(), SdkError>;
    async fn ack_batch(&self, ids: &[i64]) -> Result<(), SdkError>;
    async fn mark_read(&self, dest: &str, kind: i32, message_id: i64) -> Result<(), SdkError>;
    async fn history(
        &self,
        dest: &str,
        kind: i32,
        before_id: i64,
        limit: i32,
    ) -> Result<Vec<kim_client::HistoryItem>, SdkError>;

    async fn bot_pending(
        &self,
        dest: &str,
        limit: i32,
    ) -> Result<Vec<kim_client::BotPendingItem>, SdkError> {
        let _ = (dest, limit);
        Ok(vec![])
    }

    async fn bot_reply(
        &self,
        dest: &str,
        body: &str,
        in_reply_to: i64,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        let _ = (dest, body, in_reply_to, client_id);
        Err(SdkError::InvalidArgument {
            message: "bot_reply not supported".into(),
        })
    }

    async fn friend_list(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        Err(kim_client::ClientError::NotConnected)
    }

    async fn friend_incoming(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        Err(kim_client::ClientError::NotConnected)
    }
}

#[async_trait::async_trait]
impl ProtocolClient for KimClient {
    async fn send_message(
        &self,
        dest: &str,
        kind: i32,
        body: &str,
        extra: &str,
        payload_type: i32,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        let content = match payload_type {
            kim_protocol::MESSAGE_TYPE_IMAGE => OutgoingContent::Image {
                url: body.to_string(),
                extra: extra.to_string(),
            },
            kim_protocol::MESSAGE_TYPE_VIDEO => OutgoingContent::Video {
                url: body.to_string(),
                extra: extra.to_string(),
            },
            _ => OutgoingContent::Text(body.to_string()),
        };
        let result = KimClient::send_message(self, dest, kind, content, client_id)
            .await
            .map_err(|e| map_client(e, dest))?;
        Ok((result.message_id, result.send_time))
    }

    async fn ack(&self, message_id: i64) -> Result<(), SdkError> {
        KimClient::ack(self, message_id)
            .await
            .map_err(|e| map_client(e, ""))
    }

    async fn ack_batch(&self, ids: &[i64]) -> Result<(), SdkError> {
        KimClient::ack_batch(self, ids)
            .await
            .map_err(|e| map_client(e, ""))
    }

    async fn mark_read(&self, dest: &str, kind: i32, message_id: i64) -> Result<(), SdkError> {
        KimClient::mark_read(self, dest, kind, message_id)
            .await
            .map_err(|e| map_client(e, dest))
    }

    async fn history(
        &self,
        dest: &str,
        kind: i32,
        before_id: i64,
        limit: i32,
    ) -> Result<Vec<kim_client::HistoryItem>, SdkError> {
        KimClient::history(self, dest, kind, before_id, limit)
            .await
            .map_err(|e| map_client(e, dest))
    }

    async fn bot_pending(
        &self,
        dest: &str,
        limit: i32,
    ) -> Result<Vec<kim_client::BotPendingItem>, SdkError> {
        KimClient::bot_pending(self, dest, limit)
            .await
            .map_err(|e| map_client(e, dest))
    }

    async fn bot_reply(
        &self,
        dest: &str,
        body: &str,
        in_reply_to: i64,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        let result = KimClient::bot_reply(self, dest, body, in_reply_to, client_id)
            .await
            .map_err(|e| map_client(e, dest))?;
        Ok((result.message_id, result.send_time))
    }

    async fn friend_list(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        KimClient::friend_list(self).await
    }

    async fn friend_incoming(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        KimClient::friend_incoming(self).await
    }
}

#[async_trait::async_trait]
impl ProtocolClient for std::sync::Arc<KimClient> {
    async fn send_message(
        &self,
        dest: &str,
        kind: i32,
        body: &str,
        extra: &str,
        payload_type: i32,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        ProtocolClient::send_message(&**self, dest, kind, body, extra, payload_type, client_id)
            .await
    }

    async fn ack(&self, message_id: i64) -> Result<(), SdkError> {
        ProtocolClient::ack(&**self, message_id).await
    }

    async fn ack_batch(&self, ids: &[i64]) -> Result<(), SdkError> {
        ProtocolClient::ack_batch(&**self, ids).await
    }

    async fn mark_read(&self, dest: &str, kind: i32, message_id: i64) -> Result<(), SdkError> {
        ProtocolClient::mark_read(&**self, dest, kind, message_id).await
    }

    async fn history(
        &self,
        dest: &str,
        kind: i32,
        before_id: i64,
        limit: i32,
    ) -> Result<Vec<kim_client::HistoryItem>, SdkError> {
        ProtocolClient::history(&**self, dest, kind, before_id, limit).await
    }

    async fn bot_pending(
        &self,
        dest: &str,
        limit: i32,
    ) -> Result<Vec<kim_client::BotPendingItem>, SdkError> {
        ProtocolClient::bot_pending(&**self, dest, limit).await
    }

    async fn bot_reply(
        &self,
        dest: &str,
        body: &str,
        in_reply_to: i64,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        ProtocolClient::bot_reply(&**self, dest, body, in_reply_to, client_id).await
    }

    async fn friend_list(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        ProtocolClient::friend_list(&**self).await
    }

    async fn friend_incoming(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        ProtocolClient::friend_incoming(&**self).await
    }
}
