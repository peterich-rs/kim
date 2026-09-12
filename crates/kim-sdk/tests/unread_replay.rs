use kim_client::{InboxItem, IncomingTalk};
use kim_sdk::{KimSdk, StartSession, UnreadPolicy};

fn session(account: &str) -> StartSession {
    StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: account.into(),
    }
}

fn talk(dest: &str, sender: &str, body: &str, id: i64) -> IncomingTalk {
    IncomingTalk {
        command: "chat.user.talk".into(),
        dest: dest.into(),
        message_id: id,
        sender: sender.into(),
        msg_type: 1,
        body: body.into(),
        extra: String::new(),
        send_time: 1_700_000_000_000,
    }
}

fn inbox(dest: &str, unread: i32, last_at: i64) -> InboxItem {
    InboxItem {
        dest: dest.into(),
        kind: 0,
        title: dest.into(),
        avatar: String::new(),
        last_body: "hi".into(),
        last_sender: dest.into(),
        last_message_id: 1,
        last_send_time: last_at,
        unread,
    }
}

async fn open_alice() -> (tempfile::TempDir, std::sync::Arc<KimSdk>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .expect("open");
    sdk.start_session(session("alice")).await.expect("session");
    (dir, sdk)
}

#[tokio::test]
async fn live_duplicate_id_bumps_unread_once() {
    let (_dir, sdk) = open_alice().await;
    let t = talk("bob", "bob", "hi", 9);
    sdk.persist_talks(vec![t.clone()], UnreadPolicy::IfInserted)
        .await
        .expect("first");
    sdk.persist_talks(vec![t], UnreadPolicy::IfInserted)
        .await
        .expect("replay");
    let threads = sdk.load_threads().await.expect("threads");
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].unread, 1);
}

#[tokio::test]
async fn sync_duplicate_id_does_not_bump_unread() {
    let (_dir, sdk) = open_alice().await;
    let t = talk("bob", "bob", "hi", 9);
    sdk.persist_talks(vec![t.clone()], UnreadPolicy::Keep)
        .await
        .expect("first");
    sdk.persist_talks(vec![t], UnreadPolicy::Keep)
        .await
        .expect("replay");
    let threads = sdk.load_threads().await.expect("threads");
    assert_eq!(threads[0].unread, 0);
}

#[tokio::test]
async fn inbox_local_read_wins() {
    let (_dir, sdk) = open_alice().await;
    sdk.persist_inbox(vec![inbox("bob", 0, 2_000)])
        .await
        .expect("local read");
    sdk.persist_inbox(vec![inbox("bob", 5, 1_000)])
        .await
        .expect("server stale");
    let threads = sdk.load_threads().await.expect("threads");
    assert_eq!(threads[0].unread, 0);
}

#[tokio::test]
async fn inbox_does_not_clear_unread_without_read_marker() {
    let (_dir, sdk) = open_alice().await;
    sdk.persist_inbox(vec![inbox("bob", 4, 2_000)])
        .await
        .expect("inbox");
    let threads = sdk.load_threads().await.expect("threads");
    assert_eq!(threads[0].unread, 4);
}
