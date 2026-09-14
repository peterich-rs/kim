#![allow(clippy::unwrap_used)]

use kim_sdk::{KimSdk, StartSession, TimelineQuery, TimelineUpdate, UnreadPolicy};

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
