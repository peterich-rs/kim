use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::error::{map_sqlx, SdkError};

#[allow(dead_code)]
pub(crate) async fn load(pool: &SqlitePool, account: &str, dest: &str) -> Result<i64, SdkError> {
    let row = sqlx::query(
        "SELECT last_read_message_id FROM read_watermarks WHERE account = ? AND dest = ?",
    )
    .bind(account)
    .bind(dest)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx)?;
    match row {
        Some(r) => r.try_get("last_read_message_id").map_err(map_sqlx),
        None => Ok(0),
    }
}

pub(crate) async fn advance(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    message_id: i64,
    now: i64,
) -> Result<(), SdkError> {
    sqlx::query("UPDATE threads SET unread = 0 WHERE account = ? AND id = ?")
        .bind(account)
        .bind(dest)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    if message_id <= 0 {
        return Ok(());
    }
    sqlx::query(
        r"
        INSERT INTO read_watermarks (account, dest, last_read_message_id, last_read_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(account, dest) DO UPDATE SET
          last_read_message_id = MAX(read_watermarks.last_read_message_id, excluded.last_read_message_id),
          last_read_at = excluded.last_read_at
        ",
    )
    .bind(account)
    .bind(dest)
    .bind(message_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}
