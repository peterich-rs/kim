#![allow(clippy::unwrap_used)]
use std::sync::{Arc, Mutex};

use kim_sdk::{
    AgentPort, KimSdk, OutgoingPayload, ProtocolClient, SdkError, SendMessageCommand, SessionEpoch,
    StartSession,
};

struct RecAgent {
    turns: Mutex<Vec<(String, String, i64)>>,
}

#[async_trait::async_trait]
impl AgentPort for RecAgent {
    async fn enqueue_turn(&self, dest: &str, text: &str, in_reply_to: i64, _epoch: SessionEpoch) {
        self.turns
            .lock()
            .expect("lock")
            .push((dest.into(), text.into(), in_reply_to));
    }
    async fn catch_up(&self, _dests: &[String], _epoch: SessionEpoch) {}
}

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
        Ok((9, 1))
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
async fn sent_text_enqueues_agent_turn() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sdk = KimSdk::open(
        dir.path()
            .join("kim-cache.db")
            .to_string_lossy()
            .into_owned(),
    )
    .await
    .expect("open");
    sdk.start_session(StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: "alice".into(),
    })
    .await
    .expect("session");
    let agent = Arc::new(RecAgent {
        turns: Mutex::new(Vec::new()),
    });
    sdk.set_agent(agent.clone());
    sdk.install_protocol(Arc::new(OkProto));
    sdk.enqueue_message(SendMessageCommand {
        dest: "b_bot".into(),
        kind: 0,
        payload: OutgoingPayload::Text { body: "hi".into() },
        client_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into()),
        batch_id: None,
    })
    .await
    .expect("enqueue");
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let turns = agent.turns.lock().expect("lock").clone();
    assert_eq!(turns, vec![("b_bot".into(), "hi".into(), 9)]);
}
