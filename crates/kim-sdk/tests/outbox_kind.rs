use std::sync::{Arc, Mutex};

use kim_sdk::{
    KimSdk, OutgoingPayload, ProtocolClient, SdkError, SendMessageCommand, StartSession,
};

struct KindProto {
    kinds: Mutex<Vec<i32>>,
}

#[async_trait::async_trait]
impl ProtocolClient for KindProto {
    async fn send_message(
        &self,
        _dest: &str,
        kind: i32,
        _body: &str,
        _extra: &str,
        _payload_type: i32,
        _client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        self.kinds.lock().expect("lock").push(kind);
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
}

#[tokio::test]
async fn group_kind_survives_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("kim-cache.db");
    let path_s = path.to_string_lossy().into_owned();
    let sdk = KimSdk::open(path_s.clone()).await.expect("open");
    sdk.start_session(StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: "alice".into(),
    })
    .await
    .expect("session");
    sdk.enqueue_message(SendMessageCommand {
        dest: "g1".into(),
        kind: kim_protocol::INBOX_KIND_GROUP,
        payload: OutgoingPayload::Text { body: "hi".into() },
        client_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into()),
        batch_id: None,
    })
    .await
    .expect("enqueue");
    drop(sdk);

    let sdk = KimSdk::open(path_s).await.expect("reopen");
    sdk.start_session(StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: "alice".into(),
    })
    .await
    .expect("session");
    let proto = Arc::new(KindProto {
        kinds: Mutex::new(Vec::new()),
    });
    sdk.install_protocol(proto.clone());
    sdk.retry_send("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into())
        .await
        .expect("retry");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(
        *proto.kinds.lock().expect("lock"),
        vec![kim_protocol::INBOX_KIND_GROUP]
    );
}
