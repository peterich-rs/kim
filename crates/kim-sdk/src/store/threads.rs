use sqlx::SqliteConnection;

use crate::error::{map_sqlx, SdkError};
use crate::timeline::thread_kind_name;

pub(crate) async fn upsert_on_send(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    kind: i32,
    last_body: &str,
    last_at: i64,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        INSERT INTO threads (account, id, kind, title, last_body, last_at, unread, avatar)
        VALUES (?, ?, ?, ?, ?, ?, 0, '')
        ON CONFLICT(account, id) DO UPDATE SET
          last_body = excluded.last_body,
          last_at = CASE WHEN excluded.last_at >= threads.last_at THEN excluded.last_at ELSE threads.last_at END,
          kind = excluded.kind
        ",
    )
    .bind(account)
    .bind(dest)
    .bind(thread_kind_name(kind))
    .bind(dest)
    .bind(last_body)
    .bind(last_at)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}
