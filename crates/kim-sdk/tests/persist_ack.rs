use kim_client::IncomingTalk;
use kim_sdk::{KimSdk, StartSession, UnreadPolicy};

#[tokio::test]
async fn persist_talks_keep_does_not_ack_inside_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
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
    sdk.persist_talks(
        vec![IncomingTalk {
            command: "chat.user.talk".into(),
            dest: "bob".into(),
            message_id: 7,
            sender: "bob".into(),
            msg_type: 1,
            body: "hi".into(),
            extra: String::new(),
            send_time: 1_700_000_000_000,
        }],
        UnreadPolicy::Keep,
    )
    .await
    .expect("persist");
    let threads = sdk.load_threads().await.expect("threads");
    assert_eq!(threads[0].unread, 0);
}
