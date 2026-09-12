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
        ProtocolClient::send_message(
            &**self,
            dest,
            kind,
            body,
            extra,
            payload_type,
            client_id,
        )
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
}
