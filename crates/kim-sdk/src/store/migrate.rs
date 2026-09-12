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

    let version = schema_version(tx).await?;
    if version < 1 {
        migrate_v1(tx).await?;
        set_schema_version(tx, 1).await?;
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
        INSERT OR IGNORE INTO outbox (
          account, client_id, dest, kind, payload_type, body, extra,
          local_path, mime, width, height, byte_size, batch_id, status,
          message_id, created_at, updated_at
        )
        SELECT m.account, m.key, m.dest,
               CASE IFNULL(t.kind, 'user') WHEN 'group' THEN 1 ELSE 0 END,
               CASE m.kind WHEN 'image' THEN 2 WHEN 'video' THEN 4 ELSE 1 END,
               m.body, '', IFNULL(m.local_path, ''), '', m.width, m.height, 0,
               IFNULL(m.batch_id, ''),
               CASE m.status WHEN 'failed' THEN 'failed' ELSE 'pending' END,
               m.message_id, m.at, m.at
        FROM messages m
        LEFT JOIN threads t ON t.account = m.account AND t.id = m.dest
        WHERE m.status IN ('sending', 'failed')
        ",
    )
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
    tx.execute(sql.as_str()).await.map_err(map_sqlx)?;
    Ok(())
}

async fn column_exists(
    tx: &mut SqliteConnection,
    table: &str,
    column: &str,
) -> Result<bool, SdkError> {
    let sql = format!("PRAGMA table_info({table})");
    let rows = sqlx::query(&sql)
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
