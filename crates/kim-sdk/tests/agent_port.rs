#![allow(clippy::unwrap_used)]
use std::sync::{Arc, Mutex};

use kim_sdk::{
    AgentPort, AgentProfileRow, KimSdk, MobileAgent, OutgoingPayload, ProtocolClient,
    ScriptedRuntime, SdkError, SendMessageCommand, SessionEpoch, StartSession, UnreadPolicy,
    QUEUE_CAP,
};

struct RecAgent {
    turns: Mutex<Vec<(String, String, i64)>>,
}

#[async_trait::async_trait]
impl AgentPort for RecAgent {
    async fn enqueue_turn(
        &self,
        dest: &str,
        text: &str,
        in_reply_to: i64,
        _epoch: SessionEpoch,
    ) -> Result<(), SdkError> {
        self.turns
            .lock()
            .expect("lock")
            .push((dest.into(), text.into(), in_reply_to));
        Ok(())
    }
    async fn catch_up(&self, _dests: &[String], _epoch: SessionEpoch) -> Result<(), SdkError> {
        Ok(())
    }
}

struct OkProto;

#[async_trait::async_trait]
impl ProtocolClient for OkProto {
    async fn send_message(
        &self,
        _dest: &str,
        _kind: i32,
        _body: &str,
        _extra: &str,
        _payload_type: i32,
        _client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        Ok((9, 1))
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
        Ok(vec![])
    }

    async fn bot_reply(
        &self,
        _dest: &str,
        _body: &str,
        _in_reply_to: i64,
        _client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        Ok((42, 1))
    }

    async fn bot_pending(
        &self,
        _dest: &str,
        _limit: i32,
    ) -> Result<Vec<kim_client::BotPendingItem>, SdkError> {
        Ok(vec![kim_client::BotPendingItem {
            message_id: 123,
            body: "from phone".into(),
            send_time: 1_700_000_000,
        }])
    }
}

#[tokio::test]
async fn sent_text_enqueues_agent_turn() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sdk = KimSdk::open(
        dir.path()
            .join("kim-cache.db")
            .to_string_lossy()
            .into_owned(),
    )
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
    let agent = Arc::new(RecAgent {
        turns: Mutex::new(Vec::new()),
    });
    sdk.set_agent(agent.clone());
    sdk.install_protocol(Arc::new(OkProto));
    sdk.upsert_agent_profile(AgentProfileRow {
        profile_id: "bot".into(),
        nickname: "bot".into(),
        server_account: "b_bot".into(),
        body_json: "{}".into(),
        ..Default::default()
    })
    .await
    .expect("profile");
    sdk.enqueue_message(SendMessageCommand {
        dest: "b_bot".into(),
        kind: 0,
        payload: OutgoingPayload::Text { body: "hi".into() },
        client_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into()),
        batch_id: None,
    })
    .await
    .expect("enqueue");
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let turns = agent.turns.lock().expect("lock").clone();
    assert_eq!(turns, vec![("b_bot".into(), "hi".into(), 9)]);
}

fn start_session() -> StartSession {
    StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: "alice".into(),
    }
}

async fn open_sdk() -> (Arc<KimSdk>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let sdk = KimSdk::open(
        dir.path()
            .join("kim-cache.db")
            .to_string_lossy()
            .into_owned(),
    )
    .await
    .expect("open");
    sdk.start_session(start_session()).await.expect("session");
    (sdk, dir)
}

async fn put_bot(sdk: &KimSdk, dest: &str, profile_id: &str) {
    sdk.upsert_agent_profile(AgentProfileRow {
        profile_id: profile_id.into(),
        nickname: profile_id.into(),
        server_account: dest.into(),
        body_json: "{}".into(),
        ..Default::default()
    })
    .await
    .expect("profile");
}

#[tokio::test]
async fn pump_skips_non_bot_dest() {
    let (sdk, _dir) = open_sdk().await;
    let agent = Arc::new(RecAgent {
        turns: Mutex::new(Vec::new()),
    });
    sdk.set_agent(agent.clone());
    sdk.install_protocol(Arc::new(OkProto));
    sdk.enqueue_message(SendMessageCommand {
        dest: "bob".into(),
        kind: 0,
        payload: OutgoingPayload::Text { body: "hi".into() },
        client_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeee1".into()),
        batch_id: None,
    })
    .await
    .expect("enqueue");
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert!(agent.turns.lock().expect("lock").is_empty());
}

struct HoldRuntime {
    gate: tokio::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

#[async_trait::async_trait]
impl kim_sdk::AgentRuntime for HoldRuntime {
    async fn run_turn(
        &self,
        dest: &str,
        profile_id: &str,
        text: &str,
        _in_reply_to: i64,
        epoch: SessionEpoch,
    ) -> Result<kim_sdk::AgentRunResult, SdkError> {
        if let Some(rx) = self.gate.lock().await.take() {
            let _ = rx.await;
        }
        Ok(kim_sdk::AgentRunResult {
            dest: dest.into(),
            profile_id: profile_id.into(),
            epoch: epoch.0,
            output: text.into(),
            error: None,
        })
    }
}

#[tokio::test]
async fn queue_busy_when_cap_exceeded() {
    let (sdk, _dir) = open_sdk().await;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let runtime = Arc::new(HoldRuntime {
        gate: tokio::sync::Mutex::new(Some(rx)),
    });
    let agent = Arc::new(MobileAgent::new((*sdk).clone(), runtime));
    sdk.set_agent(agent.clone());
    put_bot(&sdk, "b_bot", "bot").await;
    let epoch = SessionEpoch(1);
    // First turn occupies the worker (blocked on gate).
    agent
        .enqueue_turn("b_bot", "hold", 1, epoch)
        .await
        .expect("hold");
    tokio::task::yield_now().await;
    for i in 0..QUEUE_CAP {
        agent
            .enqueue_turn("b_bot", &format!("t{i}"), i as i64 + 2, epoch)
            .await
            .expect("fill");
    }
    let err = sdk
        .persist_talks(vec![owner_talk(99)], UnreadPolicy::IfInserted)
        .await
        .expect_err("busy");
    assert!(matches!(err, SdkError::Busy { queue } if queue == "agent"));
    let _ = tx.send(());
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match sdk
                .persist_talks(vec![owner_talk(99)], UnreadPolicy::IfInserted)
                .await
            {
                Ok(()) => break,
                Err(SdkError::Busy { .. }) => tokio::task::yield_now().await,
                Err(error) => panic!("retry admission failed: {error}"),
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn lru_caps_dest_profile_at_four() {
    let (sdk, _dir) = open_sdk().await;
    let runtime = ScriptedRuntime::new("ok");
    let agent = Arc::new(MobileAgent::new((*sdk).clone(), runtime));
    sdk.set_agent(agent.clone());
    let epoch = SessionEpoch(1);
    for i in 0..5 {
        let dest = format!("b_{i}");
        put_bot(&sdk, &dest, &format!("p{i}")).await;
        agent
            .enqueue_turn(&dest, "hi", 1, epoch)
            .await
            .expect("turn");
    }
    assert_eq!(agent.lru_len(), 4);
}

#[tokio::test]
async fn bot_reply_persists_assistant_line() {
    let (sdk, _dir) = open_sdk().await;
    let runtime = ScriptedRuntime::new("pong");
    let agent = Arc::new(MobileAgent::new((*sdk).clone(), runtime));
    sdk.set_agent(agent.clone());
    sdk.install_protocol(Arc::new(OkProto));
    put_bot(&sdk, "b_bot", "bot").await;
    agent
        .enqueue_turn("b_bot", "hi", 1, SessionEpoch(1))
        .await
        .expect("turn");
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let threads = sdk.load_threads().await.expect("threads");
    let bot = threads
        .iter()
        .find(|t| t.id == "b_bot")
        .expect("bot thread");
    assert_eq!(bot.last_body, "pong");
}

fn owner_talk(message_id: i64) -> kim_client::IncomingTalk {
    kim_client::IncomingTalk {
        command: kim_protocol::CMD_CHAT_USER_TALK.into(),
        dest: "b_bot".into(),
        sender: "alice".into(),
        message_id,
        msg_type: kim_protocol::MESSAGE_TYPE_TEXT,
        body: "from phone".into(),
        extra: String::new(),
        send_time: 1_700_000_000 + message_id,
    }
}

#[tokio::test]
async fn live_owner_message_enqueues_agent_after_persist() {
    let (sdk, _dir) = open_sdk().await;
    let agent = Arc::new(RecAgent {
        turns: Mutex::new(Vec::new()),
    });
    sdk.set_agent(agent.clone());
    put_bot(&sdk, "b_bot", "bot").await;
    sdk.persist_talks(vec![owner_talk(123)], UnreadPolicy::IfInserted)
        .await
        .unwrap();
    assert_eq!(sdk.load_threads().await.unwrap()[0].last_body, "from phone");
    assert_eq!(
        *agent.turns.lock().unwrap(),
        vec![("b_bot".into(), "from phone".into(), 123)]
    );
}

#[tokio::test]
async fn live_agent_trigger_excludes_history_peers_replies_groups_and_media() {
    let (sdk, _dir) = open_sdk().await;
    let agent = Arc::new(RecAgent {
        turns: Mutex::new(Vec::new()),
    });
    sdk.set_agent(agent.clone());
    put_bot(&sdk, "b_bot", "bot").await;
    sdk.persist_talks(vec![owner_talk(1)], UnreadPolicy::Keep)
        .await
        .unwrap();
    let mut peer = owner_talk(2);
    peer.dest = "bob".into();
    let mut reply = owner_talk(3);
    reply.sender = "b_bot".into();
    let mut group = owner_talk(4);
    group.command = kim_protocol::CMD_CHAT_GROUP_TALK.into();
    let mut media = owner_talk(5);
    media.msg_type = kim_protocol::MESSAGE_TYPE_IMAGE;
    let mut empty = owner_talk(6);
    empty.body.clear();
    sdk.persist_talks(
        vec![peer, reply, group, media, empty, owner_talk(0)],
        UnreadPolicy::IfInserted,
    )
    .await
    .unwrap();
    assert!(agent.turns.lock().unwrap().is_empty());
}

async fn wait_agent_done(rx: &mut tokio::sync::mpsc::Receiver<kim_sdk::SessionUpdate>) {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Some(event) = rx.recv().await {
            if matches!(
                event,
                kim_sdk::SessionUpdate::AgentTurn {
                    state: kim_sdk::AgentTurnState::Done,
                    ..
                }
            ) {
                return;
            }
        }
        panic!("agent session closed");
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn live_replay_and_other_enqueue_paths_share_turn_deduplication() {
    let (sdk, _dir) = open_sdk().await;
    put_bot(&sdk, "b_bot", "bot").await;
    sdk.install_protocol(Arc::new(OkProto));
    let runtime = ScriptedRuntime::new("reply");
    let agent = Arc::new(MobileAgent::new((*sdk).clone(), runtime.clone()));
    sdk.set_agent(agent.clone());
    let mut events = sdk.subscribe_session();
    sdk.persist_talks(vec![owner_talk(123)], UnreadPolicy::IfInserted)
        .await
        .unwrap();
    wait_agent_done(&mut events).await;
    // Replayed push and the same turn submitted by outbox/catch-up must not rerun.
    sdk.persist_talks(vec![owner_talk(123)], UnreadPolicy::IfInserted)
        .await
        .unwrap();
    agent
        .catch_up(&["b_bot".into()], SessionEpoch(1))
        .await
        .unwrap();
    agent
        .enqueue_turn("b_bot", "from phone", 123, SessionEpoch(1))
        .await
        .unwrap();
    agent
        .enqueue_turn("b_bot", "next", 124, SessionEpoch(1))
        .await
        .unwrap();
    wait_agent_done(&mut events).await;
    assert_eq!(
        *runtime.turns.lock().unwrap(),
        vec![
            ("b_bot".into(), "from phone".into(), 123),
            ("b_bot".into(), "next".into(), 124),
        ]
    );
}
