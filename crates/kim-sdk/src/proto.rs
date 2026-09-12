//! Protocol client trait. `KimClient` impl lands with merge/unread (not this PR).

use crate::error::SdkError;

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
