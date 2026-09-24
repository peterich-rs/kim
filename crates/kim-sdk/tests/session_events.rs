#![allow(clippy::unwrap_used)]
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
async fn start_session_starts_reconnect_loop_and_publishes_snapshot() {
    let sdk = KimSdk::protocol_only();
    let mut snap = sdk.subscribe_session_snapshot();
    let mut events = sdk.subscribe_session();
    assert_eq!(snap.borrow().link, kim_sdk::LinkStateView::Offline);
    sdk.start_session(kim_sdk::StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: "alice".into(),
    })
    .await
    .expect("session");
    tokio::time::timeout(std::time::Duration::from_secs(2), snap.changed())
        .await
        .expect("start_session must publish a snapshot")
        .expect("watch");
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match events.recv().await {
                Some(SessionUpdate::Link { .. }) => return,
                Some(_) => {}
                None => panic!("session event channel closed"),
            }
        }
    })
    .await
    .expect("reconnect loop must emit Link");
}

#[tokio::test]
async fn expired_token_latches_auth_expired_on_snapshot() {
    let sdk = KimSdk::protocol_only();
    let mut snap = sdk.subscribe_session_snapshot();
    sdk.start_session(kim_sdk::StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: expired_jwt("alice"),
        user_agent: "test".into(),
        account: "alice".into(),
    })
    .await
    .expect("session");
    let last_error = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let err = snap.borrow().last_error.clone();
            if err.as_deref() == Some("auth_expired") {
                return err;
            }
            snap.changed().await.expect("watch");
        }
    })
    .await
    .expect("snapshot latches auth_expired");
    assert_eq!(last_error.as_deref(), Some("auth_expired"));
}

fn expired_jwt(account: &str) -> String {
    kim_protocol::generate(kim_protocol::DEMO_DEFAULT_SECRET, account, "kim", 1).unwrap()
}

fn future_jwt(account: &str) -> String {
    kim_protocol::generate(
        kim_protocol::DEMO_DEFAULT_SECRET,
        account,
        "kim",
        4_000_000_000,
    )
    .unwrap()
}

#[tokio::test]
async fn protocol_unauthorized_latches_snapshot() {
    let sdk = KimSdk::protocol_only();
    let mut snap = sdk.subscribe_session_snapshot();
    sdk.start_session(kim_sdk::StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: future_jwt("alice"),
        user_agent: "test".into(),
        account: "alice".into(),
    })
    .await
    .expect("session");
    sdk.observe_error(&kim_sdk::SdkError::Unauthorized);
    let last_error = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let err = snap.borrow().last_error.clone();
            if err.as_deref() == Some("auth_expired") {
                return err;
            }
            snap.changed().await.expect("watch");
        }
    })
    .await
    .expect("unauthorized call latches auth_expired");
    assert_eq!(last_error.as_deref(), Some("auth_expired"));
}

#[tokio::test]
async fn timeline_watch_starts_without_store() {
    let sdk = KimSdk::protocol_only();
    let rx = sdk.subscribe_timeline(TimelineQuery {
        dest: "bob".into(),
        limit: 50,
    });
    let init = rx.borrow().clone();
    match init {
        kim_sdk::TimelineUpdate::Snapshot { snapshot } => assert_eq!(snapshot.dest, "bob"),
        other => panic!("expected snapshot, got {other:?}"),
    }
}
