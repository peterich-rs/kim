use std::sync::{Arc, Mutex};

use kim_sdk::{
    KimSdk, OutgoingPayload, PageCursor, ProtocolClient, SdkError, SendMessageCommand, StartSession,
};

struct CountingProto {
    sent: Mutex<usize>,
}

#[async_trait::async_trait]
impl ProtocolClient for CountingProto {
    async fn send_message(
        &self,
        _dest: &str,
        _kind: i32,
        _body: &str,
        _extra: &str,
        _payload_type: i32,
        _client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        *self.sent.lock().expect("lock") += 1;
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
async fn delete_thread_drops_outbox_so_restart_does_not_send() {
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
        dest: "bob".into(),
        kind: 0,
        payload: OutgoingPayload::Text { body: "hi".into() },
        client_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into()),
        batch_id: None,
    })
    .await
    .expect("enqueue");
    sdk.delete_thread("bob".into()).await.expect("delete");
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
    let proto = Arc::new(CountingProto {
        sent: Mutex::new(0),
    });
    sdk.install_protocol(proto.clone());
    let err = sdk
        .retry_send("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into())
        .await
        .expect_err("gone");
    assert!(matches!(err, SdkError::NotFound { .. }));
    assert_eq!(*proto.sent.lock().expect("lock"), 0);
    let page = sdk
        .load_older(PageCursor {
            dest: "bob".into(),
            before_at: 0,
            before_key: String::new(),
            limit: 10,
            before_id: 0,
        })
        .await
        .expect("load");
    assert!(page.messages.is_empty());
}
