#![allow(clippy::unwrap_used)]

use kim_sdk::{
    KimSdk, OutgoingPayload, ProtocolClient, SdkError, SendMessageCommand, StartSession,
};

struct OkProto;

#[async_trait::async_trait]
impl ProtocolClient for OkProto {
    async fn send_message(
        &self,
        _dest: &str,
        _kind: i32,
        _body: &str,
        _extra: &str,
        _payload_type: i32,
        _client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        Ok((1, 1))
    }
    async fn ack(&self, _message_id: i64) -> Result<(), SdkError> {
        Ok(())
    }
    async fn ack_batch(&self, _ids: &[i64]) -> Result<(), SdkError> {
        Ok(())
    }
    async fn mark_read(&self, _dest: &str, _kind: i32, _message_id: i64) -> Result<(), SdkError> {
        Ok(())
    }
    async fn history(
        &self,
        _dest: &str,
        _kind: i32,
        _before_id: i64,
        _limit: i32,
    ) -> Result<Vec<kim_client::HistoryItem>, SdkError> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn search_like_is_capped_and_can_scope_dest() {
    let dir = tempfile::tempdir().unwrap();
    let sdk = KimSdk::open(
        dir.path()
            .join("kim-cache.db")
            .to_string_lossy()
            .into_owned(),
    )
    .await
    .unwrap();
    sdk.start_session(StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: "alice".into(),
    })
    .await
    .unwrap();
    sdk.install_protocol(std::sync::Arc::new(OkProto));
    sdk.enqueue_message(SendMessageCommand {
        dest: "bob".into(),
        kind: 0,
        payload: OutgoingPayload::Text {
            body: "hello world".into(),
        },
        client_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeee1".into()),
        batch_id: None,
    })
    .await
    .unwrap();
    sdk.enqueue_message(SendMessageCommand {
        dest: "carol".into(),
        kind: 0,
        payload: OutgoingPayload::Text {
            body: "hello carol".into(),
        },
        client_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeee2".into()),
        batch_id: None,
    })
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    let global = sdk.search_messages("hello".into(), None).await.unwrap();
    assert!(global.len() >= 2);
    let bob = sdk
        .search_messages("hello".into(), Some("bob".into()))
        .await
        .unwrap();
    assert_eq!(bob.len(), 1);
    assert_eq!(bob[0].dest, "bob");
}
