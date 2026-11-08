#![allow(clippy::unwrap_used)]
use super::*;
use crate::RoyalState;
use kim_protocol::pkt::Status;
use kim_royal_client::{HttpBotControl, RoyalPool};
use kim_server_data::bot_control::coordinate_delete;
use kim_server_data::idgen::SequenceIdGen;
use kim_server_data::store::{InsertMessage, MessageKind};

fn request(profile: &str) -> CreateBot {
    CreateBot {
        owner: "alice".into(),
        client_profile_id: profile.into(),
        nickname: "Assistant".into(),
        model: "test-model".into(),
        ..Default::default()
    }
}

async fn http(state: RoyalState) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let _ = crate::serve(listener, state).await;
    });
    (base, task)
}

fn rpc_pool(base: &str) -> Arc<RoyalPool> {
    Arc::new(RoyalPool::new(Some(base), &kim_protocol::resolve_internal_hmac_secret("")).unwrap())
}

fn message(dest: &str, client_id: &str) -> InsertMessage {
    InsertMessage {
        sender: "alice".into(),
        dest: dest.into(),
        send_time: 1,
        msg_type: 1,
        body: "test".into(),
        extra: String::new(),
        client_id: client_id.into(),
        online_targets: Vec::new(),
    }
}

#[tokio::test]
async fn mixed_memory_pool_is_rejected_before_identity_deletion() {
    let state = RoyalState::memory(Arc::new(SequenceIdGen::new(65_000)));
    let (a, task_a) = http(state.clone()).await;
    let (b, task_b) = http(RoyalState::memory(Arc::new(SequenceIdGen::new(75_000)))).await;
    let control = HttpBotControl::from_pool(rpc_pool(&a));
    let bot = control
        .create_bot("kim", &request("mixed-memory"))
        .await
        .unwrap()
        .profile
        .account;
    let message_pool = rpc_pool(&a);
    message_pool
        .replace_endpoints(&[
            kim_royal_client::Endpoint {
                id: "a".into(),
                base_url: a.clone(),
            },
            kim_royal_client::Endpoint {
                id: "b".into(),
                base_url: b,
            },
        ])
        .unwrap();
    let (store, _, _, _) = kim_royal_client::http_backends_with_pool(message_pool.clone()).unwrap();
    assert_eq!(
        coordinate_delete(&control, store.as_ref(), "kim", "alice", &bot)
            .await
            .unwrap_err(),
        Status::SystemException
    );
    assert!(state.users.lookup("kim", &bot).await.unwrap().is_some());
    message_pool
        .replace_endpoints(&[kim_royal_client::Endpoint {
            id: "a".into(),
            base_url: a,
        }])
        .unwrap();
    coordinate_delete(&control, store.as_ref(), "kim", "alice", &bot)
        .await
        .unwrap();
    task_a.abort();
    task_b.abort();
}

#[tokio::test]
async fn direct_memory_cleanup_never_purges_the_royal_message_store() {
    use kim_server_data::store::MemoryMessageStore;
    let state = RoyalState::memory(Arc::new(SequenceIdGen::new(35_000)));
    let (base, task) = http(state.clone()).await;
    let control = HttpBotControl::from_pool(rpc_pool(&base));
    let bot = control
        .create_bot("kim", &request("local"))
        .await
        .unwrap()
        .profile
        .account;
    let local = MemoryMessageStore::new(Arc::new(SequenceIdGen::new(40_000)));
    use kim_server_data::store::MessageStore;
    local
        .insert_user("kim", &message(&bot, "local"))
        .await
        .unwrap();
    state
        .store
        .insert_user("kim", &message(&bot, "separate-dataset"))
        .await
        .unwrap();
    coordinate_delete(&control, &local, "kim", "alice", &bot)
        .await
        .unwrap();
    assert!(local
        .insert_user("kim", &message(&bot, "late-write"))
        .await
        .is_err());
    assert!(local
        .history("kim", "alice", &bot, MessageKind::User, 0, 20)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        state
            .store
            .history("kim", "alice", &bot, MessageKind::User, 0, 20)
            .await
            .unwrap()
            .len(),
        1
    );
    task.abort();
}

struct CompleteFailure {
    inner: Arc<dyn BotControl>,
    fail: std::sync::atomic::AtomicBool,
    after_commit: bool,
}

#[async_trait]
impl BotControl for CompleteFailure {
    async fn create_bot(&self, app: &str, req: &CreateBot) -> Result<BotRecord, BotControlError> {
        self.inner.create_bot(app, req).await
    }
    async fn update_bot(
        &self,
        app: &str,
        owner: &str,
        bot: &str,
        patch: &BotPatch,
    ) -> Result<BotRecord, BotControlError> {
        self.inner.update_bot(app, owner, bot, patch).await
    }
    async fn delete_identity_or_resume(
        &self,
        app: &str,
        actor: &str,
        bot: &str,
        scope: &StoreScope,
    ) -> Result<DeletionLease, BotControlError> {
        self.inner
            .delete_identity_or_resume(app, actor, bot, scope)
            .await
    }
    async fn complete_delete(
        &self,
        app: &str,
        id: &str,
        actor: &str,
        scope: &StoreScope,
    ) -> Result<(), BotControlError> {
        let fail = self.fail.swap(false, std::sync::atomic::Ordering::SeqCst);
        if fail && !self.after_commit {
            return Err(BotControlError::NotFound);
        }
        self.inner.complete_delete(app, id, actor, scope).await?;
        if fail {
            return Err(BotControlError::Backend("completion response lost".into()));
        }
        Ok(())
    }
}

#[tokio::test]
async fn completion_interruption_and_lost_done_response_are_retryable() {
    for after_commit in [false, true] {
        let state = RoyalState::memory(Arc::new(SequenceIdGen::new(45_000)));
        let bot = state
            .bots
            .create_bot("kim", &request("complete"))
            .await
            .unwrap()
            .profile
            .account;
        state
            .store
            .insert_user("kim", &message(&bot, "complete"))
            .await
            .unwrap();
        let control = CompleteFailure {
            inner: state.bots.clone(),
            fail: std::sync::atomic::AtomicBool::new(true),
            after_commit,
        };
        assert_eq!(
            coordinate_delete(&control, state.store.as_ref(), "kim", "alice", &bot)
                .await
                .unwrap_err(),
            Status::SystemException
        );
        assert!(state.users.lookup("kim", &bot).await.unwrap().is_none());
        assert!(state
            .store
            .history("kim", "alice", &bot, MessageKind::User, 0, 20)
            .await
            .unwrap()
            .is_empty());
        coordinate_delete(&control, state.store.as_ref(), "kim", "alice", &bot)
            .await
            .unwrap();
        coordinate_delete(&control, state.store.as_ref(), "kim", "alice", &bot)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn scope_capability_and_backend_failure_keep_distinct_statuses() {
    use axum::{routing::get, Router};
    for (http_status, expected) in [
        (501, Status::ServiceUnavailable),
        (503, Status::SystemException),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new().route(
            "/internal/v1/bot/store-scope",
            get(move || async move { axum::http::StatusCode::from_u16(http_status).unwrap() }),
        );
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let control = HttpBotControl::from_pool(rpc_pool(&base));
        let (store, _, _, _) = kim_royal_client::http_backends_with_pool(rpc_pool(&base)).unwrap();
        assert_eq!(
            coordinate_delete(&control, store.as_ref(), "kim", "alice", "b_unknown")
                .await
                .unwrap_err(),
            expected
        );
        server.abort();
    }
}

#[tokio::test]
async fn unsupported_royal_fails_before_deletion_without_legacy_fallback() {
    use axum::{routing::delete, Router};
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let app = Router::new().route(
        "/api/v1/bot",
        delete(move || async move {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            axum::http::StatusCode::OK
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let control = HttpBotControl::from_pool(rpc_pool(&base));
    let (store, _, _, _) = kim_royal_client::http_backends_with_pool(rpc_pool(&base)).unwrap();
    assert_eq!(
        coordinate_delete(&control, store.as_ref(), "kim", "alice", "b_unknown")
            .await
            .unwrap_err(),
        Status::ServiceUnavailable
    );
    let local = kim_server_data::store::MemoryMessageStore::new(Arc::new(SequenceIdGen::new(1)));
    assert_eq!(
        coordinate_delete(&control, &local, "kim", "alice", "b_unknown")
            .await
            .unwrap_err(),
        Status::ServiceUnavailable
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    server.abort();
}

#[tokio::test]
async fn royal_owns_quota_config_and_idempotent_creation() {
    let state = RoyalState::memory(Arc::new(SequenceIdGen::new(55_000)));
    let first = state
        .bots
        .create_bot("kim", &request("same"))
        .await
        .unwrap();
    assert_eq!(
        state
            .bots
            .create_bot("kim", &request("same"))
            .await
            .unwrap()
            .profile
            .account,
        first.profile.account
    );
    let patch = BotPatch {
        nickname: "Updated".into(),
        model: "new-model".into(),
        ..Default::default()
    };
    assert!(matches!(
        state
            .bots
            .update_bot("kim", "bob", &first.profile.account, &patch)
            .await,
        Err(BotControlError::NotOwner)
    ));
    let updated = state
        .bots
        .update_bot("kim", "alice", &first.profile.account, &patch)
        .await
        .unwrap();
    assert_eq!(updated.config.model, "new-model");
    assert_eq!(
        state
            .users
            .profile("kim", &first.profile.account)
            .await
            .unwrap()
            .unwrap()
            .nickname,
        "Updated"
    );
    for index in 1..BOT_MAX_PER_OWNER {
        state
            .bots
            .create_bot("kim", &request(&format!("quota-{index}")))
            .await
            .unwrap();
    }
    assert!(matches!(
        state.bots.create_bot("kim", &request("over-limit")).await,
        Err(BotControlError::Invalid)
    ));
    assert!(state.bots.create_bot("kim", &request("same")).await.is_ok());
}

#[tokio::test]
async fn memory_lifecycle_is_owned_by_royal_and_preserves_other_threads() {
    let state = RoyalState::memory(Arc::new(SequenceIdGen::new(5_000)));
    state.users.upsert("kim", "alice").await.unwrap();
    let (base, task) = http(state.clone()).await;
    let control = HttpBotControl::from_pool(rpc_pool(&base));
    let (store, _, users, _) = kim_royal_client::http_backends_with_pool(rpc_pool(&base)).unwrap();
    let record = control.create_bot("kim", &request("one")).await.unwrap();
    let bot = &record.profile.account;
    assert_eq!(
        control
            .create_bot("kim", &request("one"))
            .await
            .unwrap()
            .profile
            .account,
        *bot
    );
    assert_eq!(
        users
            .lookup("kim", bot)
            .await
            .unwrap()
            .unwrap()
            .owner_account,
        "alice"
    );
    assert!(users
        .update_profile(
            "kim",
            bot,
            &kim_server_data::users::ProfilePatch {
                nickname: "bypass".into(),
                ..Default::default()
            }
        )
        .await
        .is_err());
    store
        .insert_user("kim", &message(bot, "bot-thread"))
        .await
        .unwrap();
    store
        .insert_user("kim", &message("bob", "protected-thread"))
        .await
        .unwrap();
    let scope = store.resolve_scope().await.unwrap();
    assert!(matches!(
        control
            .delete_identity_or_resume("kim", "bob", bot, &scope)
            .await,
        Err(BotControlError::NotOwner)
    ));
    let pending = control
        .delete_identity_or_resume("kim", "alice", bot, &scope)
        .await
        .unwrap();
    assert!(!pending.done);
    assert!(users.lookup("kim", bot).await.unwrap().is_none());
    let wrong = StoreScope::memory(uuid::Uuid::new_v4().to_string());
    assert!(store
        .purge_peer_dm_scoped(&wrong, "kim", "alice", bot)
        .await
        .is_err());
    assert!(matches!(
        control
            .delete_identity_or_resume("kim", "alice", bot, &wrong)
            .await,
        Err(BotControlError::Backend(_))
    ));
    coordinate_delete(&control, store.as_ref(), "kim", "alice", bot)
        .await
        .unwrap();
    coordinate_delete(&control, store.as_ref(), "kim", "alice", bot)
        .await
        .unwrap();
    let done = control
        .delete_identity_or_resume("kim", "alice", bot, &scope)
        .await
        .unwrap();
    assert!(done.done);
    assert_eq!(done.deletion_id, pending.deletion_id);
    assert_eq!(
        control
            .delete_identity_or_resume("kim", "bob", bot, &scope)
            .await
            .unwrap_err()
            .status(),
        Status::NotBotOwner
    );
    assert!(state
        .store
        .history("kim", "alice", bot, MessageKind::User, 0, 20)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        state
            .store
            .history("kim", "alice", "bob", MessageKind::User, 0, 20)
            .await
            .unwrap()
            .len(),
        1
    );
    // A new Memory lifetime must not receive the previous lifetime's Done.
    let other =
        kim_server_data::store::MemoryMessageStore::new(Arc::new(SequenceIdGen::new(9_000)));
    assert_eq!(
        coordinate_delete(&control, &other, "kim", "alice", bot)
            .await
            .unwrap_err(),
        Status::SystemException
    );
    assert_eq!(
        coordinate_delete(&control, store.as_ref(), "kim", "alice", "b_missing")
            .await
            .unwrap_err(),
        Status::UserNotFound
    );
    assert_eq!(
        reqwest::get(format!("{base}/internal/v1/bot/store-scope"))
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    task.abort();
}

#[tokio::test]
async fn purge_failures_keep_authorization_and_preserve_existing_status_codes() {
    use axum::{
        routing::{get, post},
        Json, Router,
    };
    let state = RoyalState::memory(Arc::new(SequenceIdGen::new(15_000)));
    let (base, task) = http(state.clone()).await;
    let control = HttpBotControl::from_pool(rpc_pool(&base));
    let bot = control
        .create_bot("kim", &request("errors"))
        .await
        .unwrap()
        .profile
        .account;
    let scope = state.store.resolve_scope().await.unwrap();
    for (code, expected) in [
        (400, Status::InvalidPacketBody),
        (403, Status::NotBotOwner),
        (404, Status::UserNotFound),
        (500, Status::SystemException),
        (200, Status::SystemException),
    ] {
        let scope = scope.clone();
        let app = Router::new()
            .route(
                "/internal/v1/bot/store-scope",
                get(move || async move { Json(scope) }),
            )
            .route(
                "/internal/v1/bot/purge",
                post(move || async move {
                    (axum::http::StatusCode::from_u16(code).unwrap(), Json(false))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mock_base = format!("http://{}", listener.local_addr().unwrap());
        let mock = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let (store, _, _, _) =
            kim_royal_client::http_backends_with_pool(rpc_pool(&mock_base)).unwrap();
        assert_eq!(
            coordinate_delete(&control, store.as_ref(), "kim", "alice", &bot)
                .await
                .unwrap_err(),
            expected
        );
        let pending = control
            .delete_identity_or_resume(
                "kim",
                "alice",
                &bot,
                &state.store.resolve_scope().await.unwrap(),
            )
            .await
            .unwrap();
        assert!(!pending.done);
        mock.abort();
    }
    let (store, _, _, _) = kim_royal_client::http_backends_with_pool(rpc_pool(&base)).unwrap();
    coordinate_delete(&control, store.as_ref(), "kim", "alice", &bot)
        .await
        .unwrap();
    task.abort();
}

#[tokio::test]
async fn legacy_delete_also_resumes_and_never_cleans_twice_via_http_adapter() {
    use prost::Message;
    let state = RoyalState::memory(Arc::new(SequenceIdGen::new(25_000)));
    let (base, task) = http(state.clone()).await;
    let control = HttpBotControl::from_pool(rpc_pool(&base));
    let bot = control
        .create_bot("kim", &request("legacy"))
        .await
        .unwrap()
        .profile
        .account;
    state
        .store
        .insert_user("kim", &message(&bot, "legacy"))
        .await
        .unwrap();
    let scope = state.store.resolve_scope().await.unwrap();
    control
        .delete_identity_or_resume("kim", "alice", &bot, &scope)
        .await
        .unwrap();
    for _ in 0..2 {
        let body = kim_protocol::pkt::AccountPair {
            account: "alice".into(),
            peer: bot.clone(),
        }
        .encode_to_vec();
        let headers = kim_protocol::sign_internal_hmac(
            kim_protocol::resolve_internal_hmac_secret("").as_bytes(),
            "DELETE",
            "/api/v1/bot",
            &body,
        )
        .unwrap();
        let mut req = reqwest::Client::new()
            .delete(format!("{base}/api/v1/bot"))
            .body(body);
        for (k, v) in headers.pairs() {
            req = req.header(k, v);
        }
        assert!(req.send().await.unwrap().status().is_success());
    }
    assert!(state
        .store
        .history("kim", "alice", &bot, MessageKind::User, 0, 20)
        .await
        .unwrap()
        .is_empty());
    task.abort();
}

#[cfg(feature = "postgres")]
async fn json_rpc<T: serde::de::DeserializeOwned>(
    base: &str,
    method: reqwest::Method,
    path: &str,
    value: serde_json::Value,
) -> T {
    let payload = if method == reqwest::Method::GET {
        Vec::new()
    } else {
        serde_json::to_vec(&value).unwrap()
    };
    let headers = kim_protocol::sign_internal_hmac(
        kim_protocol::resolve_internal_hmac_secret("").as_bytes(),
        method.as_str(),
        path,
        &payload,
    )
    .unwrap();
    let mut req = reqwest::Client::new()
        .request(method, format!("{base}{path}"))
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(60))
        .body(payload);
    for (k, v) in headers.pairs() {
        req = req.header(k, v);
    }
    let response = req.send().await.unwrap();
    let status = response.status();
    let body = response.bytes().await.unwrap();
    assert!(
        status.is_success(),
        "{status}: {}",
        String::from_utf8_lossy(&body)
    );
    serde_json::from_slice(&body).unwrap()
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_receipt_survives_new_authority_and_concurrent_retry() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let pool = kim_server_data::store::connect_pool(&url, Default::default())
        .await
        .unwrap();
    let owner = format!("bot-test-{}", uuid::Uuid::new_v4());
    let idgen: Arc<dyn kim_server_data::idgen::IdGenerator> = Arc::new(SequenceIdGen::new(
        (uuid::Uuid::new_v4().as_u128() & 0x3fff_ffff_ffff_ffff) as i64,
    ));
    let first = PostgresBotIdentity::new(pool.clone(), idgen.clone());
    let req = CreateBot {
        owner: owner.clone(),
        ..request("pg")
    };
    let bot = first.create_bot("kim", &req).await.unwrap().profile.account;
    let backends = kim_server_data::store::open_pg_backends(
        &url,
        None,
        idgen.clone(),
        Default::default(),
        None,
        false,
        None,
    )
    .await
    .unwrap();
    let scope = backends.store.resolve_scope().await.unwrap();
    // Force failure after receipt insertion but before identity deletion commits.
    // Identifier is generated solely from a UUID; values remain bound parameters.
    let blocker = format!("bot_delete_blocker_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TABLE {blocker}(app TEXT,account TEXT,FOREIGN KEY(app,account) REFERENCES users(app,account))"))).execute(&pool).await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {blocker} VALUES('kim',$1)"
    )))
    .bind(&bot)
    .execute(&pool)
    .await
    .unwrap();
    assert!(first
        .delete_identity_or_resume("kim", &owner, &bot, &scope)
        .await
        .is_err());
    let live: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE app='kim' AND account=$1")
        .bind(&bot)
        .fetch_one(&pool)
        .await
        .unwrap();
    let receipts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM bot_deletion WHERE app='kim' AND bot_account=$1")
            .bind(&bot)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((live, receipts), (1, 0));
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP TABLE {blocker}")))
        .execute(&pool)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        first.delete_identity_or_resume("kim", &owner, &bot, &scope),
        first.delete_identity_or_resume("kim", &owner, &bot, &scope)
    );
    let lease = a.unwrap();
    assert_eq!(lease.deletion_id, b.unwrap().deletion_id);
    assert!(!lease.done);
    drop(first);
    let restarted = PostgresBotIdentity::connect(&url, idgen.clone())
        .await
        .unwrap();
    assert_eq!(
        restarted
            .delete_identity_or_resume("kim", &owner, &bot, &scope)
            .await
            .unwrap()
            .deletion_id,
        lease.deletion_id
    );
    assert!(matches!(
        restarted
            .delete_identity_or_resume("kim", "wrong-owner", &bot, &scope)
            .await,
        Err(BotControlError::NotOwner)
    ));
    let wrong = StoreScope {
        kind: "postgres".into(),
        id: uuid::Uuid::new_v4().to_string(),
    };
    assert!(restarted
        .delete_identity_or_resume("kim", &owner, &bot, &wrong)
        .await
        .is_err());
    assert!(restarted
        .complete_delete("kim", &lease.deletion_id, &owner, &wrong)
        .await
        .is_err());
    coordinate_delete(&restarted, backends.store.as_ref(), "kim", &owner, &bot)
        .await
        .unwrap();
    assert!(
        restarted
            .delete_identity_or_resume("kim", &owner, &bot, &scope)
            .await
            .unwrap()
            .done
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE app='kim' AND account=$1")
            .bind(&bot)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM bot_deletion WHERE app='kim' AND bot_account=$1 AND status='done'",
    )
    .bind(&bot)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);

    // Two independently assembled Royal stores prove one persistent PG dataset.
    let peer_backends = kim_server_data::store::open_pg_backends(
        &url,
        None,
        idgen.clone(),
        Default::default(),
        None,
        false,
        None,
    )
    .await
    .unwrap();
    let state_a = RoyalState::with_backends(
        backends.store.clone(),
        backends.groups.clone(),
        backends.users.clone(),
        backends.social.clone(),
        Default::default(),
        Arc::new(crate::MemoryRevocation::new()),
    )
    .with_bots(Arc::new(PostgresBotIdentity::new(
        pool.clone(),
        idgen.clone(),
    )));
    let state_b = RoyalState::with_backends(
        peer_backends.store.clone(),
        peer_backends.groups,
        peer_backends.users,
        peer_backends.social,
        Default::default(),
        Arc::new(crate::MemoryRevocation::new()),
    )
    .with_bots(Arc::new(
        PostgresBotIdentity::connect(&url, idgen).await.unwrap(),
    ));
    let (a, task_a) = http(state_a).await;
    let (b, task_b) = http(state_b).await;
    // The fixture database is reached over an SSH tunnel in local operations checks.
    // Test-side HTTP waits allow that latency; production adapter budgets stay unchanged.
    let scope_a: StoreScope = json_rpc(
        &a,
        reqwest::Method::GET,
        "/internal/v1/bot/store-scope",
        serde_json::Value::Null,
    )
    .await;
    let scope_b: StoreScope = json_rpc(
        &b,
        reqwest::Method::GET,
        "/internal/v1/bot/store-scope",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(scope_a, scope_b);
    let bot = restarted
        .create_bot(
            "kim",
            &CreateBot {
                owner: owner.clone(),
                ..request("pg-http")
            },
        )
        .await
        .unwrap()
        .profile
        .account;
    let mut written = message(&bot, "pg-http");
    written.sender = owner.clone();
    backends.store.insert_user("kim", &written).await.unwrap();
    let protected = format!("protected-{}", uuid::Uuid::new_v4());
    let mut other = message(&protected, "protected");
    other.sender = owner.clone();
    backends.store.insert_user("kim", &other).await.unwrap();
    let body = serde_json::json!({"app":"kim","actor":owner,"bot":bot,"scope":scope_a});
    let lease_a: DeletionLease = json_rpc(
        &a,
        reqwest::Method::POST,
        "/internal/v1/bot/delete/identity",
        body.clone(),
    )
    .await;
    let lease_b: DeletionLease = json_rpc(
        &b,
        reqwest::Method::POST,
        "/internal/v1/bot/delete/identity",
        body.clone(),
    )
    .await;
    assert_eq!(lease_a.deletion_id, lease_b.deletion_id);
    let purged: bool = json_rpc(
        &b,
        reqwest::Method::POST,
        "/internal/v1/bot/purge",
        body.clone(),
    )
    .await;
    assert!(purged);
    let complete = serde_json::json!({"app":"kim","actor":owner,"deletion_id":lease_a.deletion_id,"scope":scope_a});
    let completed: bool = json_rpc(
        &a,
        reqwest::Method::POST,
        "/internal/v1/bot/delete/complete",
        complete,
    )
    .await;
    assert!(completed);
    let done: DeletionLease = json_rpc(
        &b,
        reqwest::Method::POST,
        "/internal/v1/bot/delete/identity",
        body,
    )
    .await;
    assert!(done.done);
    assert!(peer_backends
        .store
        .history("kim", &owner, &bot, MessageKind::User, 0, 20)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        peer_backends
            .store
            .history("kim", &owner, &protected, MessageKind::User, 0, 20)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(peer_backends
        .store
        .insert_user("kim", &written)
        .await
        .is_err());
    let mut later = written.clone();
    later.client_id = "late-in-flight".into();
    assert!(peer_backends
        .store
        .insert_user("kim", &later)
        .await
        .is_err());
    task_a.abort();
    task_b.abort();
    pool.close().await;
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_separate_message_dataset_recovers_each_deletion_boundary() {
    let (Ok(identity_url), Ok(message_url)) = (
        std::env::var("DATABASE_URL"),
        std::env::var("KIM_TEST_MESSAGE_DATABASE_URL"),
    ) else {
        return;
    };
    let idgen: Arc<dyn kim_server_data::idgen::IdGenerator> = Arc::new(SequenceIdGen::new(
        (uuid::Uuid::new_v4().as_u128() & 0x3fff_ffff_ffff_ffff) as i64,
    ));
    let authority = PostgresBotIdentity::connect(&identity_url, idgen.clone())
        .await
        .unwrap();
    let messages = kim_server_data::store::open_pg_backends(
        &message_url,
        None,
        idgen.clone(),
        Default::default(),
        None,
        false,
        None,
    )
    .await
    .unwrap();
    let identity_store = kim_server_data::store::open_pg_backends(
        &identity_url,
        None,
        idgen.clone(),
        Default::default(),
        None,
        false,
        None,
    )
    .await
    .unwrap();
    let message_pool = kim_server_data::store::connect_pool(&message_url, Default::default())
        .await
        .unwrap();
    let identity_scope = identity_store.store.resolve_scope().await.unwrap();
    let scope = messages.store.resolve_scope().await.unwrap();
    assert_ne!(identity_scope, scope);
    let owner = format!("direct-pg-{}", uuid::Uuid::new_v4());
    let bot = authority
        .create_bot(
            "kim",
            &CreateBot {
                owner: owner.clone(),
                ..request("direct-pg")
            },
        )
        .await
        .unwrap()
        .profile
        .account;
    let mut written = message(&bot, "direct-pg-bot");
    written.sender = owner.clone();
    let inserted = messages.store.insert_user("kim", &written).await.unwrap();
    let protected = format!("protected-{}", uuid::Uuid::new_v4());
    let mut other = message(&protected, "direct-pg-protected");
    other.sender = owner.clone();
    let protected_id = messages
        .store
        .insert_user("kim", &other)
        .await
        .unwrap()
        .message_id;
    messages
        .store
        .mark_read("kim", &owner, &protected, MessageKind::User, protected_id)
        .await
        .unwrap();
    // Fail the actual message purge transaction, after identity deletion committed elsewhere.
    let blocker = format!("purge_blocker_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE {blocker}(message_id BIGINT REFERENCES message_content(id))"
    )))
    .execute(&message_pool)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {blocker} VALUES($1)"
    )))
    .bind(inserted.message_id)
    .execute(&message_pool)
    .await
    .unwrap();
    assert_eq!(
        coordinate_delete(&authority, messages.store.as_ref(), "kim", &owner, &bot)
            .await
            .unwrap_err(),
        Status::SystemException
    );
    assert!(identity_store
        .users
        .lookup("kim", &bot)
        .await
        .unwrap()
        .is_none());
    let lease = authority
        .delete_identity_or_resume("kim", &owner, &bot, &scope)
        .await
        .unwrap();
    assert!(!lease.done);
    assert_eq!(
        messages
            .store
            .history("kim", &owner, &bot, MessageKind::User, 0, 20)
            .await
            .unwrap()
            .len(),
        1
    );
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP TABLE {blocker}")))
        .execute(&message_pool)
        .await
        .unwrap();
    drop(authority);
    // Restart after identity deletion, then reject both wrong owner and wrong dataset.
    let authority = PostgresBotIdentity::connect(&identity_url, idgen.clone())
        .await
        .unwrap();
    assert_eq!(
        coordinate_delete(
            &authority,
            messages.store.as_ref(),
            "kim",
            "wrong-owner",
            &bot
        )
        .await
        .unwrap_err(),
        Status::NotBotOwner
    );
    assert_eq!(
        coordinate_delete(
            &authority,
            identity_store.store.as_ref(),
            "kim",
            &owner,
            &bot
        )
        .await
        .unwrap_err(),
        Status::SystemException
    );
    messages
        .store
        .purge_peer_dm_scoped(&scope, "kim", &owner, &bot)
        .await
        .unwrap();
    drop(authority);
    drop(messages);
    // Restart after purge but before completion; recovery must purge the same dataset again.
    let authority = PostgresBotIdentity::connect(&identity_url, idgen.clone())
        .await
        .unwrap();
    let messages = kim_server_data::store::open_pg_backends(
        &message_url,
        None,
        idgen.clone(),
        Default::default(),
        None,
        false,
        None,
    )
    .await
    .unwrap();
    assert_eq!(messages.store.resolve_scope().await.unwrap(), scope);
    assert!(
        !authority
            .delete_identity_or_resume("kim", &owner, &bot, &scope)
            .await
            .unwrap()
            .done
    );
    coordinate_delete(&authority, messages.store.as_ref(), "kim", &owner, &bot)
        .await
        .unwrap();
    drop(authority);
    // Treat the successful Done response as lost and restart the authority once more.
    let authority = PostgresBotIdentity::connect(&identity_url, idgen)
        .await
        .unwrap();
    coordinate_delete(&authority, messages.store.as_ref(), "kim", &owner, &bot)
        .await
        .unwrap();
    assert!(
        authority
            .delete_identity_or_resume("kim", &owner, &bot, &scope)
            .await
            .unwrap()
            .done
    );
    assert!(messages
        .store
        .history("kim", &owner, &bot, MessageKind::User, 0, 20)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        messages
            .store
            .history("kim", &owner, &protected, MessageKind::User, 0, 20)
            .await
            .unwrap()
            .len(),
        1
    );
    let protected_read: i64 = sqlx::query_scalar(
        "SELECT last_read_id FROM conversation_reads WHERE app='kim' AND account=$1 AND peer=$2",
    )
    .bind(&owner)
    .bind(&protected)
    .fetch_one(&message_pool)
    .await
    .unwrap();
    assert_eq!(protected_read, protected_id);
    assert!(messages.store.insert_user("kim", &written).await.is_err());
    message_pool.close().await;
}
