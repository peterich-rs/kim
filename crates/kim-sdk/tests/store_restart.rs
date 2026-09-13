#![allow(clippy::unwrap_used)]
use kim_sdk::{KimSdk, OutgoingPayload, PageCursor, SendMessageCommand, SendStatus, StartSession};

fn session(account: &str) -> StartSession {
    StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: account.into(),
    }
}

fn text(dest: &str, body: &str, client_id: Option<&str>) -> SendMessageCommand {
    SendMessageCommand {
        dest: dest.into(),
        kind: 0,
        payload: OutgoingPayload::Text { body: body.into() },
        client_id: client_id.map(str::to_string),
        batch_id: None,
    }
}

#[tokio::test]
async fn pending_survives_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("kim-cache.db");
    let path_s = path.to_string_lossy().into_owned();
    let client_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

    let sdk = KimSdk::open(path_s.clone()).await.expect("open");
    sdk.start_session(session("alice")).await.expect("session");
    let receipt = sdk
        .enqueue_message(text("bob", "hi", Some(client_id)))
        .await
        .expect("enqueue");
    assert_eq!(receipt.client_id, client_id);
    assert_eq!(receipt.send_status, SendStatus::Pending);
    drop(sdk);

    let sdk = KimSdk::open(path_s).await.expect("reopen");
    sdk.start_session(session("alice")).await.expect("session");
    let page = sdk
        .load_older(PageCursor {
            dest: "bob".into(),
            before_at: 0,
            before_key: String::new(),
            limit: 50,
            before_id: 0,
        })
        .await
        .expect("load");
    assert_eq!(page.messages.len(), 1);
    assert_eq!(page.messages[0].key, client_id);
    assert_eq!(page.messages[0].send_status, SendStatus::Pending);
}

#[tokio::test]
async fn prune_does_not_drop_outbox_rows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("kim-cache.db");
    let path_s = path.to_string_lossy().into_owned();
    let sdk = KimSdk::open(path_s.clone()).await.expect("open");
    sdk.start_session(session("alice")).await.expect("session");
    for i in 0..401 {
        sdk.enqueue_message(text("bob", &format!("m{i}"), None))
            .await
            .expect("enqueue");
    }
    drop(sdk);

    let sdk = KimSdk::open(path_s).await.expect("reopen");
    sdk.start_session(session("alice")).await.expect("session");
    let page = sdk
        .load_older(PageCursor {
            dest: "bob".into(),
            before_at: 0,
            before_key: String::new(),
            limit: 500,
            before_id: 0,
        })
        .await
        .expect("load");
    assert_eq!(page.messages.len(), 401);
    assert!(page
        .messages
        .iter()
        .all(|m| m.send_status == SendStatus::Pending));
}

#[tokio::test]
async fn enqueue_without_store_is_invalid() {
    let sdk = KimSdk::protocol_only();
    sdk.start_session(session("alice")).await.expect("session");
    let err = sdk
        .enqueue_message(text("bob", "hi", None))
        .await
        .expect_err("store required");
    match err {
        kim_sdk::SdkError::InvalidArgument { message } => {
            assert!(message.contains("store not attached"));
        }
        other => panic!("unexpected {other:?}"),
    }
}
