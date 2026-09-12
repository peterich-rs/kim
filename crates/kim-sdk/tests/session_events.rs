use kim_sdk::{KimSdk, SessionUpdate, ThreadView, TimelineQuery};

#[tokio::test]
async fn kickout_from_supervisor_reaches_mpsc_before_consumer_polls() {
    let sdk = KimSdk::protocol_only();
    sdk.start_session(kim_sdk::StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: "alice".into(),
    })
    .await
    .expect("session");
    let mut rx = sdk.subscribe_session();
    sdk.supervisor()
        .expect("sup")
        .inject_event(kim_client::SessionEvent::Kickout {
            channel_id: "ch-sup".into(),
        });
    match rx.recv().await {
        Some(SessionUpdate::Kickout { channel_id }) => assert_eq!(channel_id, "ch-sup"),
        other => panic!("expected kickout from supervisor, got {other:?}"),
    }
}

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
