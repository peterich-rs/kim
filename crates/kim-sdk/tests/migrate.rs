#![allow(clippy::unwrap_used)]

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Executor, Row};

#[test]
fn production_migrate_does_not_import_messages_into_outbox() {
    let src = include_str!("../src/store/migrate.rs");
    assert!(
        !src.contains("INSERT OR IGNORE INTO outbox"),
        "production migrate must not copy Dart pending rows into outbox"
    );
    assert!(
        !src.contains("WHERE m.status IN ('sending', 'failed')"),
        "messages→outbox importer must stay deleted"
    );
}

#[tokio::test]
async fn v4_db_gains_spec_blob_accounts_and_overlay() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let opts = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(opts).await.unwrap();
    pool.execute("CREATE TABLE meta (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)")
        .await
        .unwrap();
    pool.execute("INSERT INTO meta (key, value) VALUES ('schema_version', '4')")
        .await
        .unwrap();
    pool.execute(
        r"
        CREATE TABLE agent_profiles (
          account TEXT NOT NULL,
          profile_id TEXT NOT NULL,
          nickname TEXT NOT NULL,
          server_account TEXT NOT NULL DEFAULT '',
          body_json TEXT NOT NULL,
          key_ciphertext BLOB,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY (account, profile_id)
        )
        ",
    )
    .await
    .unwrap();
    pool.close().await;

    let _sdk = kim_sdk::KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();

    let opts = SqliteConnectOptions::new().filename(&path);
    let pool = sqlx::SqlitePool::connect_with(opts).await.unwrap();
    let version: String = sqlx::query("SELECT value FROM meta WHERE key = 'schema_version'")
        .fetch_one(&pool)
        .await
        .unwrap()
        .try_get("value")
        .unwrap();
    assert_eq!(version, "8");

    let thread_cols = column_names(&pool, "threads").await;
    assert!(
        thread_cols.iter().any(|c| c == "last_message_id"),
        "{thread_cols:?}"
    );

    let cols = column_names(&pool, "agent_profiles").await;
    assert!(cols.iter().any(|c| c == "body_blob"), "{cols:?}");
    assert!(cols.iter().any(|c| c == "placement"), "{cols:?}");
    assert!(cols.iter().any(|c| c == "deleted_at"), "{cols:?}");
    assert!(table_exists(&pool, "provider_accounts").await);
    assert!(table_exists(&pool, "agent_device_overlay").await);
}

async fn column_names(pool: &sqlx::SqlitePool, table: &str) -> Vec<String> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await
        .unwrap();
    let mut names = Vec::new();
    for row in rows {
        names.push(row.try_get::<String, _>("name").unwrap());
    }
    names
}

async fn table_exists(pool: &sqlx::SqlitePool, table: &str) -> bool {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(table)
            .fetch_optional(pool)
            .await
            .unwrap();
    row.is_some()
}
