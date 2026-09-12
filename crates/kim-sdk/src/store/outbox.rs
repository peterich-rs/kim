use sqlx::SqliteConnection;

use crate::command::SendStatus;
use crate::error::{map_sqlx, SdkError};

pub(crate) struct OutboxInsert<'a> {
    pub account: &'a str,
    pub client_id: &'a str,
    pub dest: &'a str,
    pub kind: i32,
    pub payload_type: i32,
    pub body: &'a str,
    pub extra: &'a str,
    pub local_path: &'a str,
    pub mime: &'a str,
    pub width: i32,
    pub height: i32,
    pub byte_size: i64,
    pub batch_id: &'a str,
    pub status: SendStatus,
    pub now: i64,
}

pub(crate) async fn insert(
    tx: &mut SqliteConnection,
    row: OutboxInsert<'_>,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        INSERT INTO outbox (
          account, client_id, dest, kind, payload_type, body, extra, local_path,
          mime, width, height, byte_size, batch_id, status, attempt, next_attempt_at,
          message_id, last_error, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 0, '', ?, ?)
        ON CONFLICT(account, client_id) DO UPDATE SET
          dest = excluded.dest,
          kind = excluded.kind,
          payload_type = excluded.payload_type,
          body = excluded.body,
          extra = excluded.extra,
          local_path = excluded.local_path,
          mime = excluded.mime,
          width = excluded.width,
          height = excluded.height,
          byte_size = excluded.byte_size,
          batch_id = excluded.batch_id,
          status = excluded.status,
          updated_at = excluded.updated_at
        ",
    )
    .bind(row.account)
    .bind(row.client_id)
    .bind(row.dest)
    .bind(row.kind)
    .bind(row.payload_type)
    .bind(row.body)
    .bind(row.extra)
    .bind(row.local_path)
    .bind(row.mime)
    .bind(row.width)
    .bind(row.height)
    .bind(row.byte_size)
    .bind(row.batch_id)
    .bind(row.status.as_str())
    .bind(row.now)
    .bind(row.now)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) struct OutboxRow {
    pub client_id: String,
    pub dest: String,
    pub kind: i32,
    pub payload_type: i32,
    pub body: String,
    pub extra: String,
    pub local_path: String,
    pub mime: String,
    pub width: i32,
    pub height: i32,
    pub byte_size: i64,
    #[allow(dead_code)]
    pub status: String,
    pub attempt: i32,
}

fn map_row(row: sqlx::sqlite::SqliteRow) -> Result<OutboxRow, SdkError> {
    use sqlx::Row;
    Ok(OutboxRow {
        client_id: row.try_get("client_id").map_err(map_sqlx)?,
        dest: row.try_get("dest").map_err(map_sqlx)?,
        kind: row.try_get("kind").map_err(map_sqlx)?,
        payload_type: row.try_get("payload_type").map_err(map_sqlx)?,
        body: row.try_get("body").map_err(map_sqlx)?,
        extra: row.try_get("extra").map_err(map_sqlx)?,
        local_path: row.try_get("local_path").map_err(map_sqlx)?,
        mime: row.try_get("mime").map_err(map_sqlx)?,
        width: row.try_get("width").map_err(map_sqlx)?,
        height: row.try_get("height").map_err(map_sqlx)?,
        byte_size: row.try_get("byte_size").map_err(map_sqlx)?,
        status: row.try_get("status").map_err(map_sqlx)?,
        attempt: row.try_get("attempt").map_err(map_sqlx)?,
    })
}

const ROW_COLS: &str = "client_id, dest, kind, payload_type, body, extra, local_path, mime, \
     width, height, byte_size, status, attempt";

pub(crate) async fn load_due(
    pool: &sqlx::SqlitePool,
    account: &str,
    now: i64,
) -> Result<Vec<OutboxRow>, SdkError> {
    let rows = sqlx::query(&format!(
        "SELECT {ROW_COLS} FROM outbox
         WHERE account = ? AND status = 'pending' AND next_attempt_at <= ?
         ORDER BY created_at ASC, client_id ASC"
    ))
    .bind(account)
    .bind(now)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(map_row(row)?);
    }
    Ok(out)
}

pub(crate) async fn get_row(
    pool: &sqlx::SqlitePool,
    account: &str,
    client_id: &str,
) -> Result<Option<OutboxRow>, SdkError> {
    let row = sqlx::query(&format!(
        "SELECT {ROW_COLS} FROM outbox WHERE account = ? AND client_id = ?"
    ))
    .bind(account)
    .bind(client_id)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx)?;
    row.map(map_row).transpose()
}

pub(crate) async fn alive(
    pool: &sqlx::SqlitePool,
    account: &str,
    client_id: &str,
) -> Result<bool, SdkError> {
    let row = sqlx::query(
        "SELECT 1 FROM outbox WHERE account = ? AND client_id = ? AND status NOT IN ('sent', 'cancelled')",
    )
    .bind(account)
    .bind(client_id)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx)?;
    Ok(row.is_some())
}

pub(crate) async fn requeue(
    tx: &mut SqliteConnection,
    account: &str,
    client_id: &str,
    now: i64,
) -> Result<(), SdkError> {
    let n = sqlx::query(
        "UPDATE outbox SET status = 'pending', next_attempt_at = 0, updated_at = ?
         WHERE account = ? AND client_id = ? AND status IN ('pending', 'failed')",
    )
    .bind(now)
    .bind(account)
    .bind(client_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?
    .rows_affected();
    if n == 0 {
        return Err(SdkError::NotFound {
            what: "outbox".into(),
        });
    }
    sqlx::query(
        "UPDATE messages SET status = 'sending', failed = 0 WHERE account = ? AND key = ?",
    )
    .bind(account)
    .bind(client_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn wake_pending(
    tx: &mut SqliteConnection,
    account: &str,
    now: i64,
) -> Result<(), SdkError> {
    sqlx::query(
        "UPDATE outbox SET next_attempt_at = 0, updated_at = ?
         WHERE account = ? AND status = 'pending'",
    )
    .bind(now)
    .bind(account)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn mark_retry(
    tx: &mut SqliteConnection,
    account: &str,
    client_id: &str,
    attempt: i32,
    next_attempt_at: i64,
    now: i64,
) -> Result<(), SdkError> {
    sqlx::query(
        "UPDATE outbox SET status = 'pending', attempt = ?, next_attempt_at = ?, updated_at = ?
         WHERE account = ? AND client_id = ?",
    )
    .bind(attempt)
    .bind(next_attempt_at)
    .bind(now)
    .bind(account)
    .bind(client_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn mark_sent(
    tx: &mut SqliteConnection,
    account: &str,
    client_id: &str,
    message_id: i64,
    now: i64,
) -> Result<(), SdkError> {
    sqlx::query(
        "UPDATE outbox SET status = 'sent', message_id = ?, updated_at = ? WHERE account = ? AND client_id = ?",
    )
    .bind(message_id)
    .bind(now)
    .bind(account)
    .bind(client_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    sqlx::query(
        "UPDATE messages SET status = 'sent', failed = 0, message_id = CASE WHEN ? != 0 THEN ? ELSE message_id END WHERE account = ? AND key = ?",
    )
    .bind(message_id)
    .bind(message_id)
    .bind(account)
    .bind(client_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn mark_failed(
    tx: &mut SqliteConnection,
    account: &str,
    client_id: &str,
    now: i64,
) -> Result<(), SdkError> {
    sqlx::query(
        "UPDATE outbox SET status = 'failed', updated_at = ? WHERE account = ? AND client_id = ?",
    )
    .bind(now)
    .bind(account)
    .bind(client_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    sqlx::query("UPDATE messages SET status = 'failed', failed = 1 WHERE account = ? AND key = ?")
        .bind(account)
        .bind(client_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn cancel(
    tx: &mut SqliteConnection,
    account: &str,
    client_id: &str,
) -> Result<(), SdkError> {
    sqlx::query("DELETE FROM outbox WHERE account = ? AND client_id = ?")
        .bind(account)
        .bind(client_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query("DELETE FROM messages WHERE account = ? AND key = ?")
        .bind(account)
        .bind(client_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn delete_thread(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
) -> Result<(), SdkError> {
    sqlx::query("DELETE FROM outbox WHERE account = ? AND dest = ?")
        .bind(account)
        .bind(dest)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query("DELETE FROM messages WHERE account = ? AND dest = ?")
        .bind(account)
        .bind(dest)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query("DELETE FROM threads WHERE account = ? AND id = ?")
        .bind(account)
        .bind(dest)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query("DELETE FROM read_watermarks WHERE account = ? AND dest = ?")
        .bind(account)
        .bind(dest)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query("DELETE FROM timeline_meta WHERE account = ? AND dest = ?")
        .bind(account)
        .bind(dest)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}
