use kim_sdk::{KimSdk, SessionUpdate, ThreadView, TimelineQuery};

#[tokio::test]
async fn kickout_is_delivered_before_inbox_when_enqueued_first() {
    let sdk = KimSdk::protocol_only();
    let mut rx = sdk.subscribe_session();
    sdk.emit_session(SessionUpdate::Kickout {
        channel_id: "ch-1".into(),
    });
    sdk.emit_session(SessionUpdate::Inbox {
        threads: vec![ThreadView {
            id: "bob".into(),
            kind: 0,
            title: "bob".into(),
            avatar: String::new(),
            last_body: "hi".into(),
            last_at: 1,
            unread: 1,
        }],
    });
    match rx.recv().await {
        Some(SessionUpdate::Kickout { channel_id }) => assert_eq!(channel_id, "ch-1"),
        other => panic!("expected kickout, got {other:?}"),
    }
    match rx.recv().await {
        Some(SessionUpdate::Inbox { threads }) => assert_eq!(threads[0].id, "bob"),
        other => panic!("expected inbox, got {other:?}"),
    }
}

#[tokio::test]
async fn timeline_watch_starts_without_store() {
    let sdk = KimSdk::protocol_only();
    let rx = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let dest = match &*rx.borrow() {
        kim_sdk::TimelineUpdate::Resync { dest, .. } => dest.clone(),
        other => panic!("expected resync, got {other:?}"),
    };
    assert_eq!(dest, "bob");
}
