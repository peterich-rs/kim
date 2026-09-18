use kim_client::IncomingTalk;
use kim_sdk::{ConversationKey, ConversationVisibility, KimSdk, UnreadPolicy};

fn session(account: &str) -> kim_sdk::StartSession {
    kim_sdk::StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: account.into(),
    }
}

fn talk(dest: &str, sender: &str, body: &str, id: i64) -> IncomingTalk {
    IncomingTalk {
        command: kim_protocol::CMD_CHAT_USER_TALK.into(),
        dest: dest.into(),
        message_id: id,
        sender: sender.into(),
        msg_type: 1,
        body: body.into(),
        extra: String::new(),
        send_time: 1_700_000_000_000,
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
async fn active_bot_reply_keeps_unread_zero() {
    let (_dir, sdk) = open_alice().await;
    sdk.set_conversation_visibility(ConversationVisibility {
        generation: 1,
        foreground: true,
        conversation: Some(ConversationKey {
            dest: "b_bot".into(),
            kind: 0,
        }),
    })
    .await
    .expect("visible");
    sdk.persist_talks(
        vec![talk("b_bot", "b_bot", "hello", 11)],
        UnreadPolicy::IfInserted,
    )
    .await
    .expect("reply");
    let threads = sdk.load_threads().await.expect("threads");
    let row = threads.iter().find(|t| t.id == "b_bot").expect("thread");
    assert_eq!(row.unread, 0);
}

#[tokio::test]
async fn inactive_reply_increments_unread() {
    let (_dir, sdk) = open_alice().await;
    sdk.set_conversation_visibility(ConversationVisibility {
        generation: 1,
        foreground: true,
        conversation: Some(ConversationKey {
            dest: "b_bot".into(),
            kind: 0,
        }),
    })
    .await
    .expect("visible");
    sdk.set_conversation_visibility(ConversationVisibility {
        generation: 2,
        foreground: true,
        conversation: None,
    })
    .await
    .expect("hidden");
    sdk.persist_talks(
        vec![talk("b_bot", "b_bot", "hello", 12)],
        UnreadPolicy::IfInserted,
    )
    .await
    .expect("reply");
    let threads = sdk.load_threads().await.expect("threads");
    let row = threads.iter().find(|t| t.id == "b_bot").expect("thread");
    assert_eq!(row.unread, 1);
}

#[tokio::test]
async fn mark_thread_read_clears_known_unread() {
    let (_dir, sdk) = open_alice().await;
    sdk.persist_talks(vec![talk("bob", "bob", "hi", 21)], UnreadPolicy::IfInserted)
        .await
        .expect("talk");
    sdk.mark_thread_read("bob".into(), 0).await.expect("read");
    let threads = sdk.load_threads().await.expect("threads");
    assert_eq!(threads[0].unread, 0);
}

#[tokio::test]
async fn specified_zero_id_does_not_clear() {
    let (_dir, sdk) = open_alice().await;
    sdk.persist_talks(vec![talk("bob", "bob", "hi", 21)], UnreadPolicy::IfInserted)
        .await
        .expect("talk");
    sdk.mark_read(kim_sdk::ReadMarker {
        dest: "bob".into(),
        kind: 0,
        visible_message_id: 0,
    })
    .await
    .expect("zero");
    let threads = sdk.load_threads().await.expect("threads");
    assert_eq!(threads[0].unread, 1);
}
