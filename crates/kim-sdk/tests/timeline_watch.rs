#![allow(clippy::unwrap_used)]

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use kim_sdk::{
    KimSdk, ProtocolClient, SdkError, StartSession, TimelineQuery, TimelineSnapshot,
    TimelineUpdate, UnreadPolicy,
};
use sqlx::Connection;

struct HistoryProto {
    calls: AtomicUsize,
    dests: Mutex<Vec<String>>,
    rows: Mutex<Vec<kim_client::HistoryItem>>,
}

impl HistoryProto {
    fn new(rows: Vec<kim_client::HistoryItem>) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            dests: Mutex::new(Vec::new()),
            rows: Mutex::new(rows),
        })
    }
}

#[async_trait::async_trait]
impl ProtocolClient for HistoryProto {
    async fn send_message(
        &self,
        _dest: &str,
        _kind: i32,
        _body: &str,
        _extra: &str,
        _payload_type: i32,
        _client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        Err(SdkError::NotConnected)
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
        dest: &str,
        _kind: i32,
        before_id: i64,
        limit: i32,
    ) -> Result<Vec<kim_client::HistoryItem>, SdkError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.dests
            .lock()
            .expect("history dests")
            .push(dest.to_string());
        let mut rows = self.rows.lock().expect("history rows").clone();
        rows.sort_by_key(|row| std::cmp::Reverse(row.message_id));
        if before_id > 0 {
            rows.retain(|row| row.message_id < before_id);
        }
        let take = if limit <= 0 { 50 } else { limit as usize };
        rows.truncate(take);
        Ok(rows)
    }
}

fn talk_at(message_id: i64, body: impl Into<String>) -> kim_client::IncomingTalk {
    kim_client::IncomingTalk {
        command: "chat.user.talk".into(),
        dest: "bob".into(),
        message_id,
        sender: "bob".into(),
        msg_type: kim_protocol::MESSAGE_TYPE_TEXT,
        body: body.into(),
        extra: String::new(),
        send_time: 1_700_000_000 + message_id,
    }
}

async fn wait_for_snapshot<F>(
    rx: &mut tokio::sync::watch::Receiver<TimelineUpdate>,
    mut predicate: F,
) -> TimelineSnapshot
where
    F: FnMut(&TimelineSnapshot) -> bool,
{
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let TimelineUpdate::Snapshot { snapshot } = rx.borrow().clone() {
                if predicate(&snapshot) {
                    return snapshot;
                }
            }
            rx.changed().await.expect("timeline open");
        }
    })
    .await
    .expect("timeline snapshot")
}

#[tokio::test]
async fn persist_talk_reaches_timeline_subscriber_with_message_view() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
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
    sdk.persist_talks(
        vec![kim_client::IncomingTalk {
            command: "chat.user.talk".into(),
            dest: "bob".into(),
            message_id: 42,
            sender: "bob".into(),
            msg_type: 1,
            body: "hello-watch".into(),
            extra: String::new(),
            send_time: 1_700_000_000,
        }],
        UnreadPolicy::Keep,
    )
    .await
    .unwrap();

    let mut rx = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let update = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let cur = rx.borrow().clone();
            match &cur {
                TimelineUpdate::Resync { .. } => {
                    panic!("subscribe initial must not be Resync-only")
                }
                TimelineUpdate::Snapshot { snapshot } => {
                    let hit = snapshot
                        .messages
                        .iter()
                        .chain(snapshot.pending.iter())
                        .any(|m| m.body == "hello-watch");
                    if hit {
                        return cur;
                    }
                }
                TimelineUpdate::Delta { delta } => {
                    if delta.upserts.iter().any(|m| m.body == "hello-watch") {
                        return cur;
                    }
                }
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("timeline MessageView");
    match update {
        TimelineUpdate::Snapshot { snapshot } => {
            let msg = snapshot
                .messages
                .iter()
                .chain(snapshot.pending.iter())
                .find(|m| m.body == "hello-watch")
                .unwrap();
            assert!(!msg.body.is_empty());
        }
        TimelineUpdate::Delta { delta } => {
            assert!(delta.upserts.iter().any(|m| !m.body.is_empty()));
        }
        TimelineUpdate::Resync { .. } => panic!("expected Snapshot or Delta"),
    }
}

#[tokio::test]
async fn subscribe_initial_is_snapshot_not_resync() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    let rx = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let init = rx.borrow().clone();
    match init {
        TimelineUpdate::Resync { .. } => panic!("subscribe initial must not be Resync-only"),
        TimelineUpdate::Snapshot { .. } | TimelineUpdate::Delta { .. } => {}
    }
}

#[test]
fn subscribe_timeline_without_tokio_runtime_does_not_panic() {
    let sdk = KimSdk::protocol_only();
    let rx = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let init = rx.borrow().clone();
    match init {
        TimelineUpdate::Snapshot { snapshot } => assert_eq!(snapshot.dest, "bob"),
        other => panic!("expected snapshot, got {other:?}"),
    }
}

#[tokio::test]
async fn window_load_older_then_live() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
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
    for id in 1..=120 {
        sdk.persist_talks(
            vec![talk_at(id, format!("window-{id}"))],
            UnreadPolicy::Keep,
        )
        .await
        .unwrap();
    }

    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    wait_for_snapshot(&mut timeline, |snapshot| snapshot.messages.len() == 50).await;
    sdk.load_older("bob".into()).await.unwrap();
    sdk.persist_talks(vec![talk_at(121, "window-live")], UnreadPolicy::Keep)
        .await
        .unwrap();

    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot
            .messages
            .iter()
            .any(|message| message.body == "window-71")
            && snapshot
                .messages
                .iter()
                .any(|message| message.body == "window-live")
    })
    .await;
    let keys: HashSet<_> = snapshot
        .messages
        .iter()
        .chain(snapshot.pending.iter())
        .map(|message| &message.key)
        .collect();
    assert_eq!(keys.len(), snapshot.messages.len() + snapshot.pending.len());
}

#[tokio::test]
async fn load_older_at_cap_skips_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
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
    for id in 1..=400 {
        sdk.persist_talks(vec![talk_at(id, format!("cap-{id}"))], UnreadPolicy::Keep)
            .await
            .unwrap();
    }
    let proto = HistoryProto::new(vec![]);
    sdk.install_protocol(proto.clone());
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    wait_for_snapshot(&mut timeline, |snapshot| snapshot.messages.len() == 50).await;

    for _ in 0..7 {
        sdk.load_older("bob".into()).await.unwrap();
    }

    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot.messages.len() == 400 && !snapshot.loading_older
    })
    .await;
    assert!(!snapshot.has_more);
    assert_eq!(proto.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn load_older_under_cap_fetches_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
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
    for id in 1..=80 {
        sdk.persist_talks(vec![talk_at(id, format!("local-{id}"))], UnreadPolicy::Keep)
            .await
            .unwrap();
    }
    let proto = HistoryProto::new(vec![kim_client::HistoryItem {
        message_id: 1,
        msg_type: kim_protocol::MESSAGE_TYPE_TEXT,
        body: "remote-history".into(),
        extra: String::new(),
        sender: "bob".into(),
        send_time: 1_699_999_999,
        direction: 0,
    }]);
    sdk.install_protocol(proto.clone());
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    wait_for_snapshot(&mut timeline, |snapshot| snapshot.messages.len() == 50).await;

    sdk.load_older("bob".into()).await.unwrap();

    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        !snapshot.loading_older
            && snapshot
                .messages
                .iter()
                .any(|message| message.body == "remote-history")
    })
    .await;
    assert_eq!(proto.calls.load(Ordering::SeqCst), 1);
    assert!(!snapshot.has_more);
}

#[tokio::test]
async fn history_error_not_end() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
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
    sdk.persist_talks(vec![talk_at(1, "disk-failure")], UnreadPolicy::Keep)
        .await
        .unwrap();
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot
            .messages
            .iter()
            .any(|message| message.body == "disk-failure")
    })
    .await;

    let mut db = sqlx::SqliteConnection::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    sqlx::query("DROP TABLE messages")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();

    sdk.load_older("bob".into())
        .await
        .expect_err("load page must fail");
    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        !snapshot.loading_older && snapshot.history_error.is_some()
    })
    .await;
    assert!(snapshot.has_more, "load error must not signal history end");
}

fn inbox_item(dest: &str, last_message_id: i64, last_body: &str) -> kim_client::InboxItem {
    kim_client::InboxItem {
        dest: dest.into(),
        kind: 0,
        title: dest.into(),
        avatar: String::new(),
        last_body: last_body.into(),
        last_sender: dest.into(),
        last_message_id,
        last_send_time: 1_700_000_000_000_000_000,
        unread: 0,
    }
}

async fn open_alice(path: &std::path::Path) -> Arc<KimSdk> {
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
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
    sdk
}

#[tokio::test]
async fn empty_local_hydrates_latest_history_on_subscribe() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = open_alice(&path).await;
    sdk.persist_inbox(vec![inbox_item("bob", 1_001, "from-cloud")])
        .await
        .unwrap();
    let proto = HistoryProto::new(vec![kim_client::HistoryItem {
        message_id: 1_001,
        msg_type: kim_protocol::MESSAGE_TYPE_TEXT,
        body: "from-cloud".into(),
        extra: String::new(),
        sender: "bob".into(),
        send_time: 1_700_000_000,
        direction: 0,
    }]);
    sdk.install_protocol(proto.clone());
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        !snapshot.loading_older
            && snapshot
                .messages
                .iter()
                .any(|message| message.body == "from-cloud")
    })
    .await;
    assert_eq!(proto.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        proto.dests.lock().expect("dests").as_slice(),
        ["bob".to_string()]
    );
    assert!(!snapshot.loading_older);
}

#[tokio::test]
async fn caught_up_local_tip_skips_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = open_alice(&path).await;
    sdk.persist_talks(vec![talk_at(1_001, "already-local")], UnreadPolicy::Keep)
        .await
        .unwrap();
    sdk.persist_inbox(vec![inbox_item("bob", 1_001, "already-local")])
        .await
        .unwrap();
    let proto = HistoryProto::new(vec![]);
    sdk.install_protocol(proto.clone());
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot
            .messages
            .iter()
            .any(|message| message.body == "already-local")
    })
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(proto.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn inbox_persist_does_not_hydrate_unopened_threads() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = open_alice(&path).await;
    let proto = HistoryProto::new(vec![kim_client::HistoryItem {
        message_id: 1_001,
        msg_type: kim_protocol::MESSAGE_TYPE_TEXT,
        body: "should-not-fetch".into(),
        extra: String::new(),
        sender: "bob".into(),
        send_time: 1_700_000_000,
        direction: 0,
    }]);
    sdk.install_protocol(proto.clone());
    sdk.persist_inbox(vec![
        inbox_item("bob", 1_001, "preview"),
        inbox_item("carol", 2_002, "other"),
    ])
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(proto.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn empty_thread_without_inbox_tip_skips_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = open_alice(&path).await;
    let proto = HistoryProto::new(vec![]);
    sdk.install_protocol(proto.clone());
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    wait_for_snapshot(&mut timeline, |snapshot| snapshot.messages.is_empty()).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(proto.calls.load(Ordering::SeqCst), 0);
    sdk.load_older("bob".into()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(proto.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn open_thread_pages_history_until_local_window_filled() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = open_alice(&path).await;
    let rows: Vec<kim_client::HistoryItem> = (1..=120)
        .map(|id| kim_client::HistoryItem {
            message_id: id,
            msg_type: kim_protocol::MESSAGE_TYPE_TEXT,
            body: format!("cloud-{id}"),
            extra: String::new(),
            sender: if id % 2 == 0 { "alice" } else { "bob" }.into(),
            send_time: 1_700_000_000 + id,
            direction: i32::from(id % 2 == 0),
        })
        .collect();
    sdk.persist_inbox(vec![inbox_item("bob", 120, "cloud-120")])
        .await
        .unwrap();
    let proto = HistoryProto::new(rows);
    sdk.install_protocol(proto.clone());
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        !snapshot.loading_older
            && snapshot
                .messages
                .iter()
                .any(|message| message.body == "cloud-120")
    })
    .await;
    assert!(proto.calls.load(Ordering::SeqCst) >= 3);
    assert!(
        snapshot.has_more,
        "visible window is 50 of 120 persisted rows"
    );
    sdk.load_older("bob".into()).await.unwrap();
    let older = wait_for_snapshot(&mut timeline, |snapshot| {
        !snapshot.loading_older && snapshot.messages.len() > 50
    })
    .await;
    assert!(
        older
            .messages
            .iter()
            .any(|message| message.body == "cloud-1")
            || older.messages.len() == 100
    );
}
