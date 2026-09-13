#![allow(clippy::unwrap_used)]
use std::sync::{Arc, Mutex};

use kim_sdk::{
    KimSdk, MediaRef, OutgoingPayload, ProtocolClient, SdkError, SendMessageCommand, SessionUpdate,
    StartSession,
};

struct RecProto {
    sent: Mutex<Vec<(String, String, i32, String)>>,
    fail: Mutex<Option<SdkError>>,
}

impl RecProto {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            sent: Mutex::new(Vec::new()),
            fail: Mutex::new(None),
        })
    }

    fn fail_with(&self, err: SdkError) {
        *self.fail.lock().expect("lock") = Some(err);
    }

    fn sent(&self) -> Vec<(String, String, i32, String)> {
        self.sent.lock().expect("lock").clone()
    }
}

#[async_trait::async_trait]
impl ProtocolClient for RecProto {
    async fn send_message(
        &self,
        dest: &str,
        _kind: i32,
        body: &str,
        extra: &str,
        payload_type: i32,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        if let Some(err) = self.fail.lock().expect("lock").take() {
            return Err(err);
        }
        let mut sent = self.sent.lock().expect("lock");
        sent.push((
            client_id.to_string(),
            dest.to_string(),
            payload_type,
            extra.to_string(),
        ));
        let id = i64::try_from(sent.len()).unwrap_or(i64::MAX);
        let _ = body;
        Ok((id, 1))
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

fn session(account: &str) -> StartSession {
    StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: account.into(),
    }
}

fn text(dest: &str, body: &str, client_id: &str) -> SendMessageCommand {
    SendMessageCommand {
        dest: dest.into(),
        kind: 0,
        payload: OutgoingPayload::Text { body: body.into() },
        client_id: Some(client_id.into()),
        batch_id: None,
    }
}

async fn open_sdk() -> (tempfile::TempDir, Arc<KimSdk>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .expect("open");
    (dir, sdk)
}

#[tokio::test]
async fn status_109_is_not_resent_until_retry_send() {
    let (_dir, sdk) = open_sdk().await;
    sdk.start_session(session("alice")).await.expect("session");
    let proto = RecProto::new();
    proto.fail_with(SdkError::NotFriends { dest: "bob".into() });
    sdk.install_protocol(proto.clone());
    sdk.enqueue_message(text("bob", "hi", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"))
        .await
        .expect("enqueue");
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert!(proto.sent().is_empty(), "109 must not count as sent");
    sdk.enqueue_message(text(
        "carol",
        "later",
        "bbbbbbbb-bbbb-cccc-dddd-eeeeeeeeeeee",
    ))
    .await
    .expect("enqueue2");
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let sent = proto.sent();
    assert_eq!(sent.len(), 1, "failed 109 row must stay out of load_due");
    assert_eq!(sent[0].0, "bbbbbbbb-bbbb-cccc-dddd-eeeeeeeeeeee");
    sdk.retry_send("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into())
        .await
        .expect("retry");
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let sent = proto.sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].0, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
}

#[tokio::test]
async fn switch_account_drops_inflight_and_does_not_send_old_outbox() {
    let (_dir, sdk) = open_sdk().await;
    sdk.start_session(session("alice")).await.expect("session");
    sdk.enqueue_message(text("bob", "hi", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"))
        .await
        .expect("enqueue");
    sdk.switch_account("carol".into(), "t2".into())
        .await
        .expect("switch");
    let proto = RecProto::new();
    sdk.install_protocol(proto.clone());
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert!(proto.sent().is_empty());
    let err = sdk
        .retry_send("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into())
        .await
        .expect_err("other account");
    assert!(matches!(err, SdkError::NotFound { .. }));
}

#[tokio::test]
async fn kickout_from_supervisor_is_delivered_before_poll() {
    let sdk = KimSdk::protocol_only();
    sdk.start_session(session("alice")).await.expect("session");
    let mut rx = sdk.subscribe_session();
    sdk.supervisor()
        .expect("sup")
        .inject_event(kim_client::SessionEvent::Kickout {
            channel_id: "ch-9".into(),
        });
    match rx.recv().await {
        Some(SessionUpdate::Kickout { channel_id }) => assert_eq!(channel_id, "ch-9"),
        other => panic!("expected kickout, got {other:?}"),
    }
}

#[tokio::test]
async fn emit_session_full_does_not_drop_subscriber() {
    let sdk = KimSdk::protocol_only();
    let mut rx = sdk.subscribe_session();
    for i in 0..80 {
        sdk.emit_session(SessionUpdate::Kickout {
            channel_id: format!("ch-{i}"),
        });
    }
    sdk.emit_session(SessionUpdate::TokenRenew {
        token: "n".into(),
        exp: 1,
    });
    let mut saw_token = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, SessionUpdate::TokenRenew { .. }) {
            saw_token = true;
        }
    }
    sdk.emit_session(SessionUpdate::Kickout {
        channel_id: "after".into(),
    });
    let mut saw_after = false;
    for _ in 0..8 {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, SessionUpdate::Kickout { channel_id } if channel_id == "after") {
                saw_after = true;
            }
        }
        if saw_after {
            break;
        }
    }
    assert!(
        saw_token || saw_after,
        "full queue must keep the subscriber for later events"
    );
}

#[tokio::test]
async fn pump_sends_image_extra_from_outbox_row() {
    let app = axum::Router::new().route("/v1/objects", axum::routing::post(upload_ok));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.jpg");
    tokio::fs::write(&path, [0xFF, 0xD8, 0xFF, 0xD9])
        .await
        .expect("write");
    let sdk = KimSdk::open(
        dir.path()
            .join("kim-cache.db")
            .to_string_lossy()
            .into_owned(),
    )
    .await
    .expect("open");
    sdk.start_session(session("alice")).await.expect("session");
    sdk.set_upload_origin(format!("http://{addr}"))
        .expect("origin");
    let proto = RecProto::new();
    sdk.install_protocol(proto.clone());
    sdk.enqueue_message(SendMessageCommand {
        dest: "bob".into(),
        kind: 0,
        payload: OutgoingPayload::Image {
            media: MediaRef {
                path: path.to_string_lossy().into_owned(),
                mime: "image/jpeg".into(),
                width: 12,
                height: 8,
                byte_size: 4,
            },
        },
        client_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into()),
        batch_id: None,
    })
    .await
    .expect("enqueue");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let sent = proto.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].2, kim_protocol::MESSAGE_TYPE_IMAGE);
    assert_eq!(sent[0].3, r#"{"w":12,"h":8}"#);
}

async fn upload_ok() -> (axum::http::StatusCode, String) {
    (
        axum::http::StatusCode::OK,
        r#"{"url":"https://media.kim.ainexc.com/a.jpg"}"#.into(),
    )
}

#[tokio::test]
async fn mark_read_zeros_unread_without_protocol() {
    let (_dir, sdk) = open_sdk().await;
    sdk.start_session(session("alice")).await.expect("session");
    sdk.persist_talks(
        vec![kim_client::IncomingTalk {
            command: "chat.user.talk".into(),
            dest: "bob".into(),
            message_id: 9,
            sender: "bob".into(),
            msg_type: 1,
            body: "hi".into(),
            extra: String::new(),
            send_time: 1_700_000_000_000,
        }],
        kim_sdk::UnreadPolicy::IfInserted,
    )
    .await
    .expect("persist");
    assert_eq!(sdk.load_threads().await.expect("t")[0].unread, 1);
    sdk.mark_read(kim_sdk::ReadMarker {
        dest: "bob".into(),
        kind: 0,
        visible_message_id: 9,
    })
    .await
    .expect("read");
    assert_eq!(sdk.load_threads().await.expect("t")[0].unread, 0);
}

struct HoldProto {
    sent: Mutex<Vec<String>>,
    started: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: tokio::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

#[async_trait::async_trait]
impl ProtocolClient for HoldProto {
    async fn send_message(
        &self,
        _dest: &str,
        _kind: i32,
        _body: &str,
        _extra: &str,
        _payload_type: i32,
        client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        if let Some(tx) = self.started.lock().expect("lock").take() {
            let _ = tx.send(());
        }
        if let Some(rx) = self.release.lock().await.take() {
            let _ = rx.await;
        }
        let mut sent = self.sent.lock().expect("lock");
        sent.push(client_id.to_string());
        let id = i64::try_from(sent.len()).unwrap_or(i64::MAX);
        Ok((id, 1))
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
async fn second_enqueue_during_in_flight_send_is_not_dropped() {
    let (_dir, sdk) = open_sdk().await;
    sdk.start_session(session("alice")).await.expect("session");
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let proto = Arc::new(HoldProto {
        sent: Mutex::new(Vec::new()),
        started: Mutex::new(Some(started_tx)),
        release: tokio::sync::Mutex::new(Some(release_rx)),
    });
    sdk.install_protocol(proto.clone());
    sdk.enqueue_message(text("bob", "one", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"))
        .await
        .expect("enqueue1");
    started_rx.await.expect("started");
    sdk.enqueue_message(text("bob", "two", "bbbbbbbb-bbbb-cccc-dddd-eeeeeeeeeeee"))
        .await
        .expect("enqueue2");
    release_tx.send(()).expect("release");
    let a = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    let b = "bbbbbbbb-bbbb-cccc-dddd-eeeeeeeeeeee";
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        let sent = proto.sent.lock().expect("lock").clone();
        let has_a = sent.iter().any(|id| id == a);
        let has_b = sent.iter().any(|id| id == b);
        if has_a && has_b {
            assert_eq!(sent.iter().filter(|id| *id == a).count(), 1);
            assert_eq!(sent.iter().filter(|id| *id == b).count(), 1);
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
            let statuses: Vec<_> = page
                .messages
                .iter()
                .map(|m| (m.key.clone(), m.send_status))
                .collect();
            assert_eq!(page.messages.len(), 2);
            assert!(
                page.messages
                    .iter()
                    .all(|m| m.send_status == kim_sdk::SendStatus::Sent),
                "statuses {statuses:?}"
            );
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("expected both rows sent once, got {sent:?}");
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn not_connected_then_radio_up_sends() {
    let (_dir, sdk) = open_sdk().await;
    sdk.start_session(session("alice")).await.expect("session");
    let proto = RecProto::new();
    proto.fail_with(SdkError::NotConnected);
    sdk.install_protocol(proto.clone());
    sdk.enqueue_message(text("bob", "hi", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"))
        .await
        .expect("enqueue");
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert!(
        proto.sent().is_empty(),
        "NotConnected must not count as sent"
    );
    sdk.notify_radio_up().await.expect("radio");
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        let sent = proto.sent();
        if sent.len() == 1 {
            assert_eq!(sent[0].0, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("radio-up must send, got {sent:?}");
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}
