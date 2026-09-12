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
