use sqlx::{Row, SqlitePool};

use crate::error::{map_sqlx, SdkError};

#[allow(dead_code)]
pub(crate) async fn load(pool: &SqlitePool, account: &str, name: &str) -> Result<i64, SdkError> {
    let row = sqlx::query("SELECT cursor FROM sync_cursors WHERE account = ? AND name = ?")
        .bind(account)
        .bind(name)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx)?;
    match row {
        Some(r) => r.try_get("cursor").map_err(map_sqlx),
        None => Ok(0),
    }
}
