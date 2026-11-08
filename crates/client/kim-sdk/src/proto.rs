//! Protocol client trait. Impl for `KimClient` lives here (orphan rule).

use std::sync::Arc;

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
    async fn mark_read_state(
        &self,
        dest: &str,
        kind: i32,
        message_id: i64,
    ) -> Result<Option<kim_client::ConversationReadState>, SdkError> {
        self.mark_read(dest, kind, message_id).await?;
        Ok(None)
    }
    async fn conversation_states(
        &self,
        conversations: &[(String, i32)],
    ) -> Result<Vec<kim_client::ConversationReadState>, SdkError> {
        let _ = conversations;
        Ok(Vec::new())
    }
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

    async fn bot_typing(&self, dest: &str, kind: i32, active: bool) -> Result<(), SdkError> {
        let _ = (dest, kind, active);
        Ok(())
    }

    async fn agent_spec_sync(
        &self,
    ) -> Result<
        (
            Vec<kim_client::AgentSpecRecord>,
            Vec<kim_client::AgentProviderAccount>,
        ),
        SdkError,
    > {
        Ok((vec![], vec![]))
    }

    async fn agent_spec_upsert(
        &self,
        record: Option<&kim_client::AgentSpecRecord>,
        account: Option<&kim_client::AgentProviderAccount>,
    ) -> Result<kim_client::AgentSpecRecord, SdkError> {
        let _ = (record, account);
        Err(SdkError::InvalidArgument {
            message: "agent_spec_upsert not supported".into(),
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

    async fn mark_read_state(
        &self,
        dest: &str,
        kind: i32,
        message_id: i64,
    ) -> Result<Option<kim_client::ConversationReadState>, SdkError> {
        KimClient::mark_read_state(self, dest, kind, message_id)
            .await
            .map_err(|e| map_client(e, dest))
    }

    async fn conversation_states(
        &self,
        conversations: &[(String, i32)],
    ) -> Result<Vec<kim_client::ConversationReadState>, SdkError> {
        KimClient::conversation_states(self, conversations)
            .await
            .map_err(|e| map_client(e, ""))
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

    async fn bot_typing(&self, dest: &str, kind: i32, active: bool) -> Result<(), SdkError> {
        KimClient::bot_typing(self, dest, kind, active)
            .await
            .map_err(|e| map_client(e, dest))
    }

    async fn agent_spec_sync(
        &self,
    ) -> Result<
        (
            Vec<kim_client::AgentSpecRecord>,
            Vec<kim_client::AgentProviderAccount>,
        ),
        SdkError,
    > {
        KimClient::agent_spec_sync(self)
            .await
            .map_err(|e| map_client(e, ""))
    }

    async fn agent_spec_upsert(
        &self,
        record: Option<&kim_client::AgentSpecRecord>,
        account: Option<&kim_client::AgentProviderAccount>,
    ) -> Result<kim_client::AgentSpecRecord, SdkError> {
        KimClient::agent_spec_upsert(self, record, account)
            .await
            .map_err(|e| map_client(e, ""))
    }

    async fn friend_list(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        KimClient::friend_list(self).await
    }

    async fn friend_incoming(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        KimClient::friend_incoming(self).await
    }
}

#[async_trait::async_trait]
impl ProtocolClient for Arc<KimClient> {
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

    async fn mark_read_state(
        &self,
        dest: &str,
        kind: i32,
        message_id: i64,
    ) -> Result<Option<kim_client::ConversationReadState>, SdkError> {
        ProtocolClient::mark_read_state(&**self, dest, kind, message_id).await
    }

    async fn conversation_states(
        &self,
        conversations: &[(String, i32)],
    ) -> Result<Vec<kim_client::ConversationReadState>, SdkError> {
        ProtocolClient::conversation_states(&**self, conversations).await
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

    async fn bot_typing(&self, dest: &str, kind: i32, active: bool) -> Result<(), SdkError> {
        ProtocolClient::bot_typing(&**self, dest, kind, active).await
    }

    async fn agent_spec_sync(
        &self,
    ) -> Result<
        (
            Vec<kim_client::AgentSpecRecord>,
            Vec<kim_client::AgentProviderAccount>,
        ),
        SdkError,
    > {
        ProtocolClient::agent_spec_sync(&**self).await
    }

    async fn agent_spec_upsert(
        &self,
        record: Option<&kim_client::AgentSpecRecord>,
        account: Option<&kim_client::AgentProviderAccount>,
    ) -> Result<kim_client::AgentSpecRecord, SdkError> {
        ProtocolClient::agent_spec_upsert(&**self, record, account).await
    }

    async fn friend_list(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        ProtocolClient::friend_list(&**self).await
    }

    async fn friend_incoming(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        ProtocolClient::friend_incoming(&**self).await
    }
}

pub(crate) struct ObservingProtocol {
    pub inner: Arc<dyn ProtocolClient>,
    pub sdk: std::sync::Weak<crate::Inner>,
}

fn note<T>(sdk: &std::sync::Weak<crate::Inner>, r: Result<T, SdkError>) -> Result<T, SdkError> {
    if let Err(err) = &r {
        if let Some(inner) = sdk.upgrade() {
            crate::KimSdk::from_inner(inner).observe_error(err);
        }
    }
    r
}

fn note_client<T>(
    sdk: &std::sync::Weak<crate::Inner>,
    r: Result<T, kim_client::ClientError>,
) -> Result<T, kim_client::ClientError> {
    if let Err(err) = &r {
        if err.is_fatal_auth() {
            if let Some(inner) = sdk.upgrade() {
                crate::KimSdk::from_inner(inner).observe_error(&SdkError::Unauthorized);
            }
        }
    }
    r
}

#[async_trait::async_trait]
impl ProtocolClient for ObservingProtocol {
    async fn send_message(
        &self,
        dest: &str,
        kind: i32,
        body: &str,
        extra: &str,
        payload_type: i32,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        note(
            &self.sdk,
            self.inner
                .send_message(dest, kind, body, extra, payload_type, client_id)
                .await,
        )
    }

    async fn ack(&self, message_id: i64) -> Result<(), SdkError> {
        note(&self.sdk, self.inner.ack(message_id).await)
    }

    async fn ack_batch(&self, ids: &[i64]) -> Result<(), SdkError> {
        note(&self.sdk, self.inner.ack_batch(ids).await)
    }

    async fn mark_read(&self, dest: &str, kind: i32, message_id: i64) -> Result<(), SdkError> {
        note(
            &self.sdk,
            self.inner.mark_read(dest, kind, message_id).await,
        )
    }

    async fn mark_read_state(
        &self,
        dest: &str,
        kind: i32,
        message_id: i64,
    ) -> Result<Option<kim_client::ConversationReadState>, SdkError> {
        note(
            &self.sdk,
            self.inner.mark_read_state(dest, kind, message_id).await,
        )
    }

    async fn conversation_states(
        &self,
        conversations: &[(String, i32)],
    ) -> Result<Vec<kim_client::ConversationReadState>, SdkError> {
        note(
            &self.sdk,
            self.inner.conversation_states(conversations).await,
        )
    }

    async fn history(
        &self,
        dest: &str,
        kind: i32,
        before_id: i64,
        limit: i32,
    ) -> Result<Vec<kim_client::HistoryItem>, SdkError> {
        note(
            &self.sdk,
            self.inner.history(dest, kind, before_id, limit).await,
        )
    }

    async fn bot_pending(
        &self,
        dest: &str,
        limit: i32,
    ) -> Result<Vec<kim_client::BotPendingItem>, SdkError> {
        note(&self.sdk, self.inner.bot_pending(dest, limit).await)
    }

    async fn bot_reply(
        &self,
        dest: &str,
        body: &str,
        in_reply_to: i64,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        note(
            &self.sdk,
            self.inner
                .bot_reply(dest, body, in_reply_to, client_id)
                .await,
        )
    }

    async fn bot_typing(&self, dest: &str, kind: i32, active: bool) -> Result<(), SdkError> {
        note(&self.sdk, self.inner.bot_typing(dest, kind, active).await)
    }

    async fn agent_spec_sync(
        &self,
    ) -> Result<
        (
            Vec<kim_client::AgentSpecRecord>,
            Vec<kim_client::AgentProviderAccount>,
        ),
        SdkError,
    > {
        note(&self.sdk, self.inner.agent_spec_sync().await)
    }

    async fn agent_spec_upsert(
        &self,
        record: Option<&kim_client::AgentSpecRecord>,
        account: Option<&kim_client::AgentProviderAccount>,
    ) -> Result<kim_client::AgentSpecRecord, SdkError> {
        note(
            &self.sdk,
            self.inner.agent_spec_upsert(record, account).await,
        )
    }

    async fn friend_list(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        note_client(&self.sdk, self.inner.friend_list().await)
    }

    async fn friend_incoming(&self) -> Result<Vec<kim_client::Profile>, kim_client::ClientError> {
        note_client(&self.sdk, self.inner.friend_incoming().await)
    }
}
