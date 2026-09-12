use sqlx::{Row, SqlitePool};

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
