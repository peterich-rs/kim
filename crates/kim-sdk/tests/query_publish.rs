#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use kim_client::{InboxItem, IncomingTalk};
use kim_sdk::{
    KimSdk, OutgoingPayload, PersonRef, ProtocolClient, SdkError, SendMessageCommand, SendStatus,
    StartSession, TimelineQuery, TimelineSnapshot, TimelineUpdate, UnreadPolicy,
};
use sqlx::Connection;
use tokio::sync::Barrier;

struct BlockingHistoryProto {
    history_started: Arc<Barrier>,
    resume_history: tokio::sync::Notify,
    rows: Vec<kim_client::HistoryItem>,
}

impl BlockingHistoryProto {
    fn new(rows: Vec<kim_client::HistoryItem>) -> Arc<Self> {
        Arc::new(Self {
            history_started: Arc::new(Barrier::new(2)),
            resume_history: tokio::sync::Notify::new(),
            rows,
        })
    }
}

#[async_trait::async_trait]
impl ProtocolClient for BlockingHistoryProto {
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
        _dest: &str,
        _kind: i32,
        _before_id: i64,
        _limit: i32,
    ) -> Result<Vec<kim_client::HistoryItem>, SdkError> {
        self.history_started.wait().await;
        self.resume_history.notified().await;
        Ok(self.rows.clone())
    }
}

fn session() -> StartSession {
    session_for("alice")
}

fn session_for(account: &str) -> StartSession {
    StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: account.into(),
    }
}

fn talk(message_id: i64, body: &str) -> IncomingTalk {
    IncomingTalk {
        command: "chat.user.talk".into(),
        dest: "bob".into(),
        message_id,
        sender: "bob".into(),
        msg_type: kim_protocol::MESSAGE_TYPE_TEXT,
        body: body.into(),
        extra: String::new(),
        send_time: 1_700_000_000_000,
    }
}

fn command(client_id: &str, body: &str) -> SendMessageCommand {
    SendMessageCommand {
        dest: "bob".into(),
        kind: kim_protocol::INBOX_KIND_USER,
        payload: OutgoingPayload::Text { body: body.into() },
        client_id: Some(client_id.into()),
        batch_id: None,
    }
}

fn contact(account: &str) -> PersonRef {
    PersonRef {
        account: account.into(),
        nickname: account.into(),
        avatar: String::new(),
        bio: String::new(),
        relation: "friend".into(),
        kind: kim_protocol::INBOX_KIND_USER,
    }
}

async fn open_sdk() -> (tempfile::TempDir, Arc<KimSdk>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    sdk.start_session(session()).await.unwrap();
    (dir, sdk)
}

async fn wait_for_snapshot<F>(
    rx: &mut tokio::sync::watch::Receiver<TimelineUpdate>,
    mut predicate: F,
) -> TimelineSnapshot
where
    F: FnMut(&TimelineSnapshot) -> bool,
{
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let TimelineUpdate::Snapshot { snapshot } = rx.borrow().clone() {
                if predicate(&snapshot) {
                    return snapshot;
                }
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("timeline snapshot")
}

#[tokio::test]
async fn enqueue_records_timeline_and_inbox() {
    let (_dir, sdk) = open_sdk().await;
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let mut inbox = sdk.subscribe_session_snapshot();

    sdk.enqueue_message(command("11111111-1111-4111-8111-111111111111", "pending"))
        .await
        .unwrap();

    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot.pending.iter().any(|msg| msg.body == "pending")
    })
    .await;
    assert!(snapshot.pending.iter().any(|msg| msg.body == "pending"));
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if inbox
                .borrow()
                .threads
                .iter()
                .any(|thread| thread.id == "bob")
            {
                return;
            }
            inbox.changed().await.unwrap();
        }
    })
    .await
    .expect("inbox snapshot");
}

#[tokio::test]
async fn duplicate_identical_keep_is_noop() {
    let (_dir, sdk) = open_sdk().await;
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let incoming = talk(42, "same");
    sdk.persist_talks(vec![incoming.clone()], UnreadPolicy::Keep)
        .await
        .unwrap();
    let first = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot.messages.iter().any(|msg| msg.body == "same")
    })
    .await;

    sdk.persist_talks(vec![incoming], UnreadPolicy::Keep)
        .await
        .unwrap();
    let second = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot.messages.iter().any(|msg| msg.body == "same")
    })
    .await;
    assert_eq!(second.version, first.version);
}

#[tokio::test]
async fn dual_key_merge_collapses_and_publishes() {
    let (dir, sdk) = open_sdk().await;
    let client_id = "22222222-2222-4222-8222-222222222222";
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    sdk.enqueue_message(command(client_id, "local pending"))
        .await
        .unwrap();
    let before = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot.pending.iter().any(|msg| msg.key == client_id)
    })
    .await;

    // Reproduce an old/corrupt dual-key cache: current schema normally prevents
    // two non-zero rows with the same message ID.
    let mut db = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        dir.path().join("kim-cache.db").display()
    ))
    .await
    .unwrap();
    sqlx::query("DROP INDEX messages_mid_unique")
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::query("UPDATE messages SET message_id = 77 WHERE account = ? AND dest = ? AND key = ?")
        .bind("alice")
        .bind("bob")
        .bind(client_id)
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO messages (
            account, dest, key, sender, body, at, sys, failed, kind, width, height,
            message_id, batch_id, status, local_path, thread_kind
        ) VALUES (?, ?, ?, ?, ?, ?, 0, 0, 'text', 0, 0, ?, '', 'sent', '', ?)",
    )
    .bind("alice")
    .bind("bob")
    .bind(kim_sdk::incoming_message_key(77, 1_700_000_000_000, "bob"))
    .bind("bob")
    .bind("remote copy")
    .bind(1_700_000_000_000_i64)
    .bind(77_i64)
    .bind(kim_protocol::INBOX_KIND_USER)
    .execute(&mut db)
    .await
    .unwrap();
    db.close().await.unwrap();

    sdk.persist_talks(vec![talk(77, "server copy")], UnreadPolicy::Keep)
        .await
        .unwrap();
    let after =
        wait_for_snapshot(&mut timeline, |snapshot| snapshot.version > before.version).await;
    assert_eq!(
        after
            .messages
            .iter()
            .filter(|message| message.key == client_id)
            .count(),
        1
    );
    assert!(after
        .messages
        .iter()
        .chain(after.pending.iter())
        .any(|message| message.key == client_id));
}

#[tokio::test]
async fn pending_merge_to_sent_publishes_timeline() {
    let (dir, sdk) = open_sdk().await;
    let client_id = "33333333-3333-4333-8333-333333333333";
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    sdk.enqueue_message(command(client_id, "local pending"))
        .await
        .unwrap();
    let before = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot.pending.iter().any(|msg| msg.key == client_id)
    })
    .await;

    let mut db = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        dir.path().join("kim-cache.db").display()
    ))
    .await
    .unwrap();
    sqlx::query("UPDATE messages SET message_id = 88 WHERE account = ? AND dest = ? AND key = ?")
        .bind("alice")
        .bind("bob")
        .bind(client_id)
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();

    sdk.persist_talks(vec![talk(88, "local pending")], UnreadPolicy::Keep)
        .await
        .unwrap();
    let after = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot.version > before.version
            && snapshot
                .messages
                .iter()
                .any(|message| message.key == client_id && message.send_status == SendStatus::Sent)
    })
    .await;
    assert!(after
        .messages
        .iter()
        .any(|message| message.key == client_id && message.send_status == SendStatus::Sent));
}

#[tokio::test]
async fn replace_contacts_stale_epoch_keeps_existing_contacts() {
    let (dir, sdk) = open_sdk().await;
    sdk.replace_contacts(vec![contact("bob")]).await.unwrap();

    let mut blocker = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        dir.path().join("kim-cache.db").display()
    ))
    .await
    .unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut blocker)
        .await
        .unwrap();

    let persist_sdk = sdk.clone();
    let blocked_write = tokio::spawn(async move {
        persist_sdk
            .persist_inbox(vec![InboxItem {
                dest: "blocked".into(),
                kind: kim_protocol::INBOX_KIND_USER,
                title: "blocked".into(),
                avatar: String::new(),
                last_body: String::new(),
                last_sender: String::new(),
                last_message_id: 0,
                last_send_time: 0,
                unread: 0,
            }])
            .await
    });
    tokio::time::sleep(Duration::from_millis(25)).await;

    let stale_sdk = sdk.clone();
    let stale_replace =
        tokio::spawn(async move { stale_sdk.replace_contacts(vec![contact("carol")]).await });
    tokio::time::sleep(Duration::from_millis(25)).await;
    sdk.stop_session().await.unwrap();

    sqlx::query("ROLLBACK").execute(&mut blocker).await.unwrap();
    blocker.close().await.unwrap();
    blocked_write.await.unwrap().unwrap();
    let err = stale_replace
        .await
        .unwrap()
        .expect_err("stale contacts write");
    assert!(matches!(err, SdkError::StaleEpoch { .. }));

    sdk.start_session(session()).await.unwrap();
    let contacts = sdk.load_contacts().await.unwrap();
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].account, "bob");
}

#[tokio::test]
async fn publisher_survives_start_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    sdk.start_session(session()).await.unwrap();
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });

    let started = tokio::time::Instant::now();
    sdk.enqueue_message(command(
        "44444444-4444-4444-8444-444444444444",
        "publisher stays alive",
    ))
    .await
    .unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "enqueue waited for the 2s fallback instead of the live publisher"
    );
    wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot
            .pending
            .iter()
            .any(|message| message.body == "publisher stays alive")
    })
    .await;
}

#[tokio::test]
async fn reconnect_same_account_rebuilds() {
    let (_dir, sdk) = open_sdk().await;
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    sdk.persist_talks(vec![talk(101, "survives reconnect")], UnreadPolicy::Keep)
        .await
        .unwrap();
    wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot
            .messages
            .iter()
            .any(|message| message.body == "survives reconnect")
    })
    .await;

    sdk.start_session(session()).await.unwrap();
    let rebuilt = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot
            .messages
            .iter()
            .any(|message| message.body == "survives reconnect")
    })
    .await;
    assert!(rebuilt
        .messages
        .iter()
        .any(|message| message.body == "survives reconnect"));
}

#[tokio::test]
async fn switch_via_start_session_holds_stamp() {
    let (_dir, sdk) = open_sdk().await;
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    sdk.start_session(session_for("carol")).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let update = timeline.borrow().clone();
            match update {
                TimelineUpdate::Resync { reason, .. } if reason == "account" => return,
                _ => timeline.changed().await.unwrap(),
            }
        }
    })
    .await
    .expect("account switch resync");

    sdk.persist_talks(vec![talk(102, "carol only")], UnreadPolicy::Keep)
        .await
        .unwrap();
    let no_foreign_body = tokio::time::timeout(Duration::from_millis(200), async {
        loop {
            if let TimelineUpdate::Snapshot { snapshot } = timeline.borrow().clone() {
                assert!(!snapshot
                    .messages
                    .iter()
                    .chain(snapshot.pending.iter())
                    .any(|message| message.body == "carol only"));
            }
            timeline.changed().await.unwrap();
        }
    })
    .await;
    assert!(
        no_foreign_body.is_err(),
        "old stamp received a new account body"
    );
    assert!(!timeline.has_changed().unwrap());
}

#[tokio::test]
async fn delete_thread_resync_clears_older() {
    let (_dir, sdk) = open_sdk().await;
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    sdk.persist_talks(vec![talk(103, "delete me")], UnreadPolicy::Keep)
        .await
        .unwrap();
    wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot
            .messages
            .iter()
            .any(|message| message.body == "delete me")
    })
    .await;

    let mut resync = timeline.clone();
    let resync_waiter = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let update = resync.borrow().clone();
                match update {
                    TimelineUpdate::Resync { dest, reason }
                        if dest == "bob" && reason == "deleted" =>
                    {
                        return;
                    }
                    _ => resync.changed().await.unwrap(),
                }
            }
        })
        .await
    });
    tokio::task::yield_now().await;
    sdk.delete_thread("bob".into()).await.unwrap();
    resync_waiter.await.unwrap().expect("delete resync");

    let empty = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot.messages.is_empty() && snapshot.pending.is_empty()
    })
    .await;
    assert!(empty.messages.is_empty());
    assert!(empty.pending.is_empty());
}

async fn settle_query_refreshes(sdk: &KimSdk) -> u64 {
    let mut last = sdk.query_refresh_total();
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(2)).await;
        let now = sdk.query_refresh_total();
        if now == last {
            return now;
        }
        last = now;
    }
    last
}

#[tokio::test]
async fn coalesced_refresh() {
    let (_dir, sdk) = open_sdk().await;
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let _ = wait_for_snapshot(&mut timeline, |_| true).await;
    let before = settle_query_refreshes(&sdk).await;
    let mut writes = tokio::task::JoinSet::new();
    for id in 200..232 {
        let sdk = sdk.clone();
        writes.spawn(async move {
            sdk.persist_talks(
                vec![talk(id, &format!("coalesced-{id}"))],
                UnreadPolicy::Keep,
            )
            .await
        });
    }
    while let Some(result) = writes.join_next().await {
        result.unwrap().unwrap();
    }
    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        (200..232).all(|id| {
            snapshot
                .messages
                .iter()
                .any(|message| message.body == format!("coalesced-{id}"))
        })
    })
    .await;
    for id in 200..232 {
        assert!(
            snapshot
                .messages
                .iter()
                .any(|message| message.body == format!("coalesced-{id}")),
            "missing coalesced-{id}"
        );
    }
    let refreshes = settle_query_refreshes(&sdk).await.saturating_sub(before);
    // Timeline + Inbox per persist (2N). The write worker is serial, so a
    // burst may hit the ceiling when each refresh finishes before the next
    // COMMIT. Slot merge is covered by `changes.rs`.
    assert!(
        (2..=32 * 2).contains(&refreshes),
        "unexpected refresh count: {refreshes} for 32 commits"
    );
}

#[tokio::test]
async fn slow_subscriber_converges() {
    let (_dir, sdk) = open_sdk().await;
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    for id in 300..305 {
        sdk.persist_talks(vec![talk(id, &format!("slow-{id}"))], UnreadPolicy::Keep)
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    sdk.persist_talks(vec![talk(305, "slow-final")], UnreadPolicy::Keep)
        .await
        .unwrap();
    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot
            .messages
            .iter()
            .any(|message| message.body == "slow-final")
    })
    .await;
    assert!(snapshot
        .messages
        .iter()
        .any(|message| message.body == "slow-final"));
}

#[tokio::test]
async fn first_subscribe_rebuilds() {
    let (_dir, sdk) = open_sdk().await;
    sdk.persist_talks(vec![talk(400, "before subscribe")], UnreadPolicy::Keep)
        .await
        .unwrap();

    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot
            .messages
            .iter()
            .any(|message| message.body == "before subscribe")
    })
    .await;
    assert!(snapshot
        .messages
        .iter()
        .any(|message| message.body == "before subscribe"));
}

#[tokio::test]
async fn persist_hook_does_not_wait_query() {
    let (_dir, sdk) = open_sdk().await;
    let initial_refreshes = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let refreshes = sdk.query_refresh_total();
            if refreshes > 0 {
                return refreshes;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("session startup query refresh");

    let started = tokio::time::Instant::now();
    sdk.persist_talks(vec![talk(401, "persist does not wait")], UnreadPolicy::Keep)
        .await
        .unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "persist path waited for the query publisher"
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if sdk.query_refresh_total() > initial_refreshes {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("persist commit must be drained from the ChangeLog without a timeline subscriber");
}

#[tokio::test]
async fn concurrent_persist_during_load_older_converges_to_one_snapshot() {
    let (_dir, sdk) = open_sdk().await;
    sdk.persist_talks(vec![talk(700, "existing")], UnreadPolicy::Keep)
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
            .any(|message| message.body == "existing")
    })
    .await;

    let proto = BlockingHistoryProto::new(vec![kim_client::HistoryItem {
        message_id: 699,
        msg_type: kim_protocol::MESSAGE_TYPE_TEXT,
        body: "remote during load".into(),
        extra: String::new(),
        sender: "bob".into(),
        send_time: 1_699_999_999_999,
        direction: 0,
    }]);
    sdk.install_protocol(proto.clone());
    let loading_sdk = sdk.clone();
    let loading = tokio::spawn(async move { loading_sdk.load_older("bob".into()).await });
    proto.history_started.wait().await;

    sdk.persist_talks(vec![talk(701, "live during load")], UnreadPolicy::Keep)
        .await
        .unwrap();
    assert!(
        !loading.is_finished(),
        "load_older completed before its blocked history request resumed"
    );

    proto.resume_history.notify_one();
    loading.await.unwrap().unwrap();
    let snapshot = wait_for_snapshot(&mut timeline, |snapshot| {
        snapshot
            .messages
            .iter()
            .any(|message| message.body == "remote during load")
            && snapshot
                .messages
                .iter()
                .any(|message| message.body == "live during load")
    })
    .await;
    let keys: std::collections::HashSet<_> = snapshot
        .messages
        .iter()
        .chain(snapshot.pending.iter())
        .map(|message| &message.key)
        .collect();
    assert_eq!(
        keys.len(),
        snapshot.messages.len() + snapshot.pending.len(),
        "the concurrent write and expanded window must not duplicate rows"
    );
}

#[tokio::test]
async fn load_hot_window_consistent_read() {
    let (_dir, sdk) = open_sdk().await;
    let mut timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let writer = {
        let sdk = sdk.clone();
        tokio::spawn(async move {
            for id in 500..520 {
                sdk.persist_talks(
                    vec![talk(id, &format!("consistent-{id}"))],
                    UnreadPolicy::IfInserted,
                )
                .await
                .unwrap();
            }
        })
    };

    while !writer.is_finished() {
        let update = timeline.borrow().clone();
        if let TimelineUpdate::Snapshot { snapshot } = update {
            assert_eq!(
                snapshot.unread,
                snapshot.messages.len() as i32,
                "unread and messages must come from one read transaction"
            );
        }
        let _ = tokio::time::timeout(Duration::from_millis(20), timeline.changed()).await;
    }
    writer.await.unwrap();
    let final_snapshot =
        wait_for_snapshot(&mut timeline, |snapshot| snapshot.messages.len() == 20).await;
    assert_eq!(final_snapshot.unread, 20);
}

#[tokio::test]
async fn commit_failure_does_not_publish() {
    let (dir, sdk) = open_sdk().await;
    let timeline = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let mut blocker = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        dir.path().join("kim-cache.db").display()
    ))
    .await
    .unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut blocker)
        .await
        .unwrap();

    let error = sdk
        .persist_talks(vec![talk(600, "must not publish")], UnreadPolicy::Keep)
        .await
        .expect_err("locked write must fail");
    assert!(matches!(error, SdkError::SqliteBusy));
    sqlx::query("ROLLBACK").execute(&mut blocker).await.unwrap();
    blocker.close().await.unwrap();

    tokio::time::sleep(Duration::from_millis(25)).await;
    let update = timeline.borrow().clone();
    if let TimelineUpdate::Snapshot { snapshot } = update {
        assert!(!snapshot
            .messages
            .iter()
            .chain(snapshot.pending.iter())
            .any(|message| message.body == "must not publish"));
    }
}
