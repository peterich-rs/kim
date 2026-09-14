#![allow(clippy::unwrap_used)]

use kim_sdk::{prepare_store_file, KimSdk, PrepareOutcome, SdkError};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::Executor;

#[tokio::test]
async fn dart_shaped_db_without_schema_version_is_wiped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let opts = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(opts).await.unwrap();
    pool.execute("CREATE TABLE meta (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)")
        .await
        .unwrap();
    pool.execute("INSERT INTO meta (key, value) VALUES ('prefs_imported', '1')")
        .await
        .unwrap();
    pool.execute(
        "CREATE TABLE messages (account TEXT, dest TEXT, key TEXT, status TEXT, body TEXT)",
    )
    .await
    .unwrap();
    pool.execute("INSERT INTO messages VALUES ('alice','bob','k1','sending','hi')")
        .await
        .unwrap();
    pool.close().await;

    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert!(sdk.store_wipe_total() >= 1);
}

#[tokio::test]
async fn kim_sdk_v1_db_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    sdk.start_session(kim_sdk::StartSession {
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
            message_id: 1,
            sender: "bob".into(),
            msg_type: 1,
            body: "kept".into(),
            extra: String::new(),
            send_time: 1,
        }],
        kim_sdk::UnreadPolicy::Keep,
    )
    .await
    .unwrap();
    drop(sdk);

    let outcome = tokio::task::spawn_blocking({
        let path = path.clone();
        move || prepare_store_file(&path)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(outcome, PrepareOutcome::Keep);

    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(sdk.store_wipe_total(), 0);
    sdk.start_session(kim_sdk::StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: "alice".into(),
    })
    .await
    .unwrap();
    let threads = sdk.load_threads().await.unwrap();
    assert_eq!(threads[0].last_body, "kept");
}

#[tokio::test]
async fn newer_than_sdk_is_hard_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let opts = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(opts).await.unwrap();
    pool.execute("CREATE TABLE meta (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)")
        .await
        .unwrap();
    pool.execute("INSERT INTO meta (key, value) VALUES ('schema_version', '99')")
        .await
        .unwrap();
    pool.close().await;

    let err = tokio::task::spawn_blocking({
        let path = path.clone();
        move || prepare_store_file(&path)
    })
    .await
    .unwrap()
    .expect_err("newer store");
    assert!(matches!(err, SdkError::InvalidArgument { .. }));
    assert!(path.exists());
}

#[tokio::test]
async fn wipe_errors_if_primary_still_exists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    std::fs::create_dir(&path).unwrap();
    let err = tokio::task::spawn_blocking({
        let path = path.clone();
        move || prepare_store_file(&path)
    })
    .await
    .unwrap()
    .expect_err("directory cannot be wiped as a file");
    assert!(matches!(err, SdkError::Disk { .. }));
    assert!(path.exists());
}

#[tokio::test]
async fn non_sqlite_file_is_wiped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    std::fs::write(&path, b"not a database").unwrap();
    let outcome = tokio::task::spawn_blocking({
        let path = path.clone();
        move || prepare_store_file(&path)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(outcome, PrepareOutcome::CreateEmpty { wiped: true });
}
