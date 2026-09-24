use sqlx::sqlite::SqliteConnection;
use sqlx::{Connection, Executor, Row, SqlitePool};

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
    ensure_column(tx, "threads", "avatar", "TEXT NOT NULL DEFAULT ''").await?;
    ensure_column(tx, "messages", "message_id", "INTEGER NOT NULL DEFAULT 0").await?;
    ensure_column(tx, "messages", "batch_id", "TEXT NOT NULL DEFAULT ''").await?;
    ensure_column(tx, "messages", "status", "TEXT NOT NULL DEFAULT 'sent'").await?;
    ensure_column(tx, "messages", "local_path", "TEXT NOT NULL DEFAULT ''").await?;
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
    tx.execute(schema::CREATE_CONVERSATION_READ_STATE)
        .await
        .map_err(map_sqlx)?;

    let version = schema_version(tx).await?;
    if version < 1 {
        migrate_v1(tx).await?;
        set_schema_version(tx, 1).await?;
    }
    if version < 2 {
        migrate_v2(tx).await?;
        set_schema_version(tx, 2).await?;
    }
    if version < 3 {
        migrate_v3(tx).await?;
        set_schema_version(tx, 3).await?;
    }
    if version < 4 {
        migrate_v4(tx).await?;
        set_schema_version(tx, 4).await?;
    }
    if version < 5 {
        migrate_v5(tx).await?;
        set_schema_version(tx, 5).await?;
    }
    if version < 6 {
        migrate_v6(tx).await?;
        set_schema_version(tx, 6).await?;
    }
    if version < 7 {
        migrate_v7(tx).await?;
        set_schema_version(tx, 7).await?;
    }
    if version < 8 {
        migrate_v8(tx).await?;
        set_schema_version(tx, 8).await?;
    }
    Ok(())
}

async fn migrate_v1(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    ensure_column(tx, "messages", "thread_kind", "INTEGER NOT NULL DEFAULT 0").await?;
    tx.execute(schema::CREATE_OUTBOX).await.map_err(map_sqlx)?;
    tx.execute(schema::IDX_OUTBOX_DUE).await.map_err(map_sqlx)?;
    tx.execute(schema::CREATE_SYNC_CURSORS)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_READ_WATERMARKS)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_TIMELINE_META)
        .await
        .map_err(map_sqlx)?;
    tx.execute(
        r"
        UPDATE messages SET thread_kind = (
          SELECT CASE t.kind WHEN 'group' THEN 1 ELSE 0 END
          FROM threads t
          WHERE t.account = messages.account AND t.id = messages.dest
        )
        WHERE EXISTS (
          SELECT 1 FROM threads t
          WHERE t.account = messages.account AND t.id = messages.dest
        )
        ",
    )
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

async fn migrate_v2(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    tx.execute(schema::CREATE_CONTACTS)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_SETTINGS)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn migrate_v3(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    tx.execute(schema::CREATE_AGENT_PROFILES)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_AGENT_PERMISSIONS)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn migrate_v4(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    tx.execute(schema::CREATE_MEDIA_CACHE)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn migrate_v5(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    ensure_column(tx, "agent_profiles", "body_blob", "BLOB").await?;
    ensure_column(
        tx,
        "agent_profiles",
        "placement",
        "TEXT NOT NULL DEFAULT 'local'",
    )
    .await?;
    tx.execute(schema::CREATE_PROVIDER_ACCOUNTS)
        .await
        .map_err(map_sqlx)?;
    tx.execute(schema::CREATE_AGENT_DEVICE_OVERLAY)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn migrate_v6(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    ensure_column(tx, "agent_profiles", "deleted_at", "INTEGER").await?;
    Ok(())
}

async fn migrate_v7(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    ensure_column(
        tx,
        "threads",
        "last_message_id",
        "INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
    Ok(())
}

async fn migrate_v8(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    tx.execute(schema::CREATE_READ_WATERMARKS)
        .await
        .map_err(map_sqlx)?;
    ensure_column(tx, "read_watermarks", "kind", "INTEGER NOT NULL DEFAULT 0").await?;
    ensure_column(
        tx,
        "read_watermarks",
        "confirmed_message_id",
        "INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
    ensure_column(
        tx,
        "read_watermarks",
        "retry_count",
        "INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
    ensure_column(
        tx,
        "read_watermarks",
        "next_retry_at",
        "INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
    ensure_column(
        tx,
        "read_watermarks",
        "last_error",
        "TEXT NOT NULL DEFAULT ''",
    )
    .await?;
    tx.execute(schema::CREATE_CONVERSATION_READ_STATE)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn schema_version(tx: &mut SqliteConnection) -> Result<i64, SdkError> {
    let row = sqlx::query("SELECT value FROM meta WHERE key = 'schema_version'")
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    match row {
        Some(r) => {
            let raw: String = r.try_get("value").map_err(map_sqlx)?;
            Ok(raw.parse().unwrap_or(0))
        }
        None => Ok(0),
    }
}

async fn set_schema_version(tx: &mut SqliteConnection, version: i64) -> Result<(), SdkError> {
    sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?)")
        .bind(version.to_string())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn ensure_column(
    tx: &mut SqliteConnection,
    table: &str,
    column: &str,
    spec: &str,
) -> Result<(), SdkError> {
    if column_exists(tx, table, column).await? {
        return Ok(());
    }
    let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {spec}");
    // Identifiers come from this crate's migrations, not user input.
    tx.execute(sqlx::AssertSqlSafe(sql))
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn column_exists(
    tx: &mut SqliteConnection,
    table: &str,
    column: &str,
) -> Result<bool, SdkError> {
    let sql = format!("PRAGMA table_info({table})");
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    for row in rows {
        let name: String = row.try_get("name").map_err(map_sqlx)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}
