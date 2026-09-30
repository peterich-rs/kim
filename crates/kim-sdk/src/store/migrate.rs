use sqlx::sqlite::SqliteConnection;
use sqlx::{Connection, Executor, SqlitePool};

use super::schema;
use crate::error::{map_sqlx, SdkError};

pub async fn migrate(pool: &SqlitePool) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    let mut tx = conn.begin().await.map_err(map_sqlx)?;
    run(&mut tx).await?;
    tx.commit().await.map_err(map_sqlx)?;
    Ok(())
}

async fn run(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    tx.execute(schema::CREATE_META).await.map_err(map_sqlx)?;
    tx.execute(schema::CREATE_THREADS).await.map_err(map_sqlx)?;
    tx.execute(schema::CREATE_MESSAGES)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::IDX_MESSAGES_BY_ID)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::IDX_MESSAGES_MID_UNIQUE)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::IDX_MESSAGES_BY_THREAD_KEY)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::IDX_MESSAGES_PENDING)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_OUTBOX).await.map_err(map_sqlx)?;
    tx.execute(schema::IDX_OUTBOX_DUE).await.map_err(map_sqlx)?;
    tx.execute(schema::CREATE_SYNC_CURSORS)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_READ_WATERMARKS)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_CONVERSATION_READ_STATE)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_TIMELINE_META)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_CONTACTS)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_SETTINGS)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_AGENT_PROFILES)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_AGENT_PERMISSIONS)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_MEDIA_CACHE)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_PROVIDER_ACCOUNTS)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_AGENT_DEVICE_OVERLAY)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_WORKSPACE_GRANTS)
        .await
        .map_err(map_sqlx)?;
    set_schema_version(tx, schema::SCHEMA_VERSION).await?;
    let pragma = format!("PRAGMA user_version = {}", schema::SCHEMA_VERSION);
    tx.execute(sqlx::AssertSqlSafe(pragma))
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn set_schema_version(tx: &mut SqliteConnection, version: i64) -> Result<(), SdkError> {
    sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?)")
        .bind(version.to_string())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}
