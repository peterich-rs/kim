#![allow(clippy::unwrap_used)]
use std::sync::{Arc, Mutex};

use kim_sdk::{
    KimSdk, OutgoingPayload, ProtocolClient, SdkError, SendMessageCommand, SendStatus, StartSession,
};

struct MockProto {
    sent: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl ProtocolClient for MockProto {
    async fn send_message(
        &self,
        _dest: &str,
        _kind: i32,
        _body: &str,
        _extra: &str,
        _payload_type: i32,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        self.sent.lock().expect("lock").push(client_id.to_string());
        Ok((42, 1))
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

fn session() -> StartSession {
    StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: "alice".into(),
    }
}

#[tokio::test]
async fn kill_mid_send_keeps_same_client_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("kim-cache.db");
    let path_s = path.to_string_lossy().into_owned();
    let client_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    let sdk = KimSdk::open(path_s.clone()).await.expect("open");
    sdk.start_session(session()).await.expect("session");
    sdk.enqueue_message(SendMessageCommand {
        dest: "bob".into(),
        kind: 0,
        payload: OutgoingPayload::Text { body: "hi".into() },
        client_id: Some(client_id.into()),
        batch_id: None,
    })
    .await
    .expect("enqueue");
    drop(sdk);

    let sdk = KimSdk::open(path_s).await.expect("reopen");
    sdk.start_session(session()).await.expect("session");
    let proto = Arc::new(MockProto {
        sent: Mutex::new(Vec::new()),
    });
    sdk.install_protocol(proto.clone());
    sdk.retry_send(client_id.into()).await.expect("retry");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let sent = proto.sent.lock().expect("lock").clone();
    assert_eq!(sent, vec![client_id]);
    let page = sdk
        .load_older(kim_sdk::PageCursor {
            dest: "bob".into(),
            before_at: 0,
            before_key: String::new(),
            limit: 10,
            before_id: 0,
        })
        .await
        .expect("load");
    assert_eq!(page.messages[0].key, client_id);
    assert_eq!(page.messages[0].send_status, SendStatus::Sent);
}
