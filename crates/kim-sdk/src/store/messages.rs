use sqlx::{Row, SqliteConnection, SqlitePool};

use super::schema::MAX_MESSAGES;
use crate::command::{PageCursor, SendStatus};
use crate::error::{map_sqlx, SdkError};
use crate::timeline::{kind_from_name, kind_name, MessageView};

pub(crate) struct OwnInsert<'a> {
    pub account: &'a str,
    pub dest: &'a str,
    pub key: &'a str,
    pub sender: &'a str,
    pub body: &'a str,
    pub at: i64,
    pub kind: i32,
    pub width: i32,
    pub height: i32,
    pub batch_id: &'a str,
    pub status: SendStatus,
    pub local_path: &'a str,
    pub thread_kind: i32,
}

pub(crate) async fn insert_own(
    tx: &mut SqliteConnection,
    row: OwnInsert<'_>,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        INSERT INTO messages (
          account, dest, key, sender, body, at, sys, failed, kind, width, height,
          message_id, batch_id, status, local_path, thread_kind
        ) VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, 0, ?, ?, ?, ?)
        ON CONFLICT(account, dest, key) DO UPDATE SET
          body = excluded.body,
          at = excluded.at,
          failed = excluded.failed,
          kind = excluded.kind,
          width = excluded.width,
          height = excluded.height,
          batch_id = excluded.batch_id,
          status = excluded.status,
          local_path = excluded.local_path,
          thread_kind = excluded.thread_kind
        ",
    )
    .bind(row.account)
    .bind(row.dest)
    .bind(row.key)
    .bind(row.sender)
    .bind(row.body)
    .bind(row.at)
    .bind(i32::from(row.status == SendStatus::Failed))
    .bind(kind_name(row.kind))
    .bind(row.width)
    .bind(row.height)
    .bind(row.batch_id)
    .bind(row.status.message_status())
    .bind(row.local_path)
    .bind(row.thread_kind)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn prune(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        DELETE FROM messages
        WHERE account = ? AND dest = ?
          AND key NOT IN (
            SELECT client_id FROM outbox
            WHERE account = ? AND dest = ?
              AND status NOT IN ('sent', 'cancelled')
          )
          AND key NOT IN (
            SELECT key FROM messages
            WHERE account = ? AND dest = ?
            ORDER BY at DESC, key DESC
            LIMIT ?
          )
        ",
    )
    .bind(account)
    .bind(dest)
    .bind(account)
    .bind(dest)
    .bind(account)
    .bind(dest)
    .bind(MAX_MESSAGES)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn load_page(
    pool: &SqlitePool,
    account: &str,
    cursor: &PageCursor,
) -> Result<(Vec<MessageView>, bool), SdkError> {
    let limit = if cursor.limit <= 0 { 50 } else { cursor.limit };
    let fetch = i64::from(limit) + 1;
    let rows = if cursor.before_at == 0 && cursor.before_key.is_empty() {
        sqlx::query(
            r"
            SELECT key, dest, sender, body, at, sys, kind, width, height,
                   message_id, batch_id, status, local_path
            FROM messages
            WHERE account = ? AND dest = ?
            ORDER BY at DESC, key DESC
            LIMIT ?
            ",
        )
        .bind(account)
        .bind(&cursor.dest)
        .bind(fetch)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx)?
    } else {
        sqlx::query(
            r"
            SELECT key, dest, sender, body, at, sys, kind, width, height,
                   message_id, batch_id, status, local_path
            FROM messages
            WHERE account = ? AND dest = ?
              AND (at < ? OR (at = ? AND key < ?))
            ORDER BY at DESC, key DESC
            LIMIT ?
            ",
        )
        .bind(account)
        .bind(&cursor.dest)
        .bind(cursor.before_at)
        .bind(cursor.before_at)
        .bind(&cursor.before_key)
        .bind(fetch)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx)?
    };
    let has_more = rows.len() as i64 > i64::from(limit);
    let take = if has_more { limit as usize } else { rows.len() };
    let mut out = Vec::with_capacity(take);
    for row in rows.into_iter().take(take) {
        out.push(row_to_view(&row)?);
    }
    Ok((out, has_more))
}

fn row_to_view(row: &sqlx::sqlite::SqliteRow) -> Result<MessageView, SdkError> {
    let kind_raw: String = row.try_get("kind").map_err(map_sqlx)?;
    let status_raw: String = row.try_get("status").map_err(map_sqlx)?;
    let batch: String = row.try_get("batch_id").map_err(map_sqlx)?;
    let local: String = row.try_get("local_path").map_err(map_sqlx)?;
    let sys: i64 = row.try_get("sys").map_err(map_sqlx)?;
    Ok(MessageView {
        key: row.try_get("key").map_err(map_sqlx)?,
        dest: row.try_get("dest").map_err(map_sqlx)?,
        sender: row.try_get("sender").map_err(map_sqlx)?,
        body: row.try_get("body").map_err(map_sqlx)?,
        local_path: if local.is_empty() { None } else { Some(local) },
        at: row.try_get("at").map_err(map_sqlx)?,
        sys: sys != 0,
        kind: kind_from_name(&kind_raw),
        width: row.try_get("width").map_err(map_sqlx)?,
        height: row.try_get("height").map_err(map_sqlx)?,
        message_id: row.try_get("message_id").map_err(map_sqlx)?,
        batch_id: if batch.is_empty() { None } else { Some(batch) },
        send_status: SendStatus::from_db(&status_raw),
    })
}
