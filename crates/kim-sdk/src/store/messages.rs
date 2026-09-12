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

#[derive(Clone, Debug)]
pub(crate) struct StoredMsg {
    pub key: String,
    pub dest: String,
    pub sender: String,
    pub body: String,
    pub at: i64,
    pub sys: bool,
    pub kind: String,
    pub width: i32,
    pub height: i32,
    pub message_id: i64,
    pub batch_id: String,
    pub status: String,
    pub local_path: String,
    pub thread_kind: i32,
}

pub(crate) struct ApplyOutcome {
    pub dest: String,
    #[allow(dead_code)]
    pub inserted: bool,
    pub unread_delta: i32,
    pub msg: StoredMsg,
}

pub(crate) async fn apply_talk(
    tx: &mut SqliteConnection,
    account: &str,
    talk: &kim_client::IncomingTalk,
    policy: crate::sync::UnreadPolicy,
) -> Result<Option<ApplyOutcome>, SdkError> {
    let dest = if talk.dest.is_empty() {
        talk.sender.clone()
    } else {
        talk.dest.clone()
    };
    if dest.is_empty() || talk.body.is_empty() {
        return Ok(None);
    }
    let sender = if talk.sender.is_empty() {
        dest.clone()
    } else {
        talk.sender.clone()
    };
    let (width, height) = parse_image_extra(&talk.extra);
    let kind = match talk.msg_type {
        kim_protocol::MESSAGE_TYPE_IMAGE => "image",
        kim_protocol::MESSAGE_TYPE_VIDEO => "video",
        _ => "text",
    };
    let key = crate::ids::incoming_message_key(talk.message_id, talk.send_time, &sender);
    let incoming = StoredMsg {
        key: key.clone(),
        dest: dest.clone(),
        sender: sender.clone(),
        body: talk.body.clone(),
        at: super::send_time_ms(talk.send_time),
        sys: false,
        kind: kind.to_string(),
        width,
        height,
        message_id: talk.message_id,
        batch_id: String::new(),
        status: "sent".into(),
        local_path: String::new(),
        thread_kind: if talk.command.contains("group") {
            kim_protocol::INBOX_KIND_GROUP
        } else {
            kim_protocol::INBOX_KIND_USER
        },
    };
    let by_mid = if incoming.message_id != 0 {
        find_by_mid(tx, account, &dest, incoming.message_id).await?
    } else {
        None
    };
    let by_key = find_by_key(tx, account, &dest, &incoming.key).await?;
    let (survivor, loser) = match (by_mid, by_key) {
        (Some(mid), Some(key_row)) if mid.key != key_row.key => {
            if crate::ids::prefer_key(&key_row.key, &mid.key) == key_row.key {
                (Some(key_row), Some(mid))
            } else {
                (Some(mid), Some(key_row))
            }
        }
        (mid, key_row) => (mid.or(key_row), None),
    };
    let inserted = survivor.is_none();
    let merged = match survivor {
        None => incoming,
        Some(prev) => merge_stored(prev, incoming),
    };
    if let Some(loser) = loser {
        if loser.key != merged.key {
            delete_key(tx, account, &dest, &loser.key).await?;
        }
    }
    if let Some(surv) = find_by_key(tx, account, &dest, &merged.key).await? {
        if surv.key != merged.key {
            delete_key(tx, account, &dest, &surv.key).await?;
        }
    }
    put_msg(tx, account, &merged).await?;
    let unread_delta = match policy {
        crate::sync::UnreadPolicy::IfInserted
            if inserted && !merged.sys && merged.sender != account =>
        {
            1
        }
        _ => 0,
    };
    Ok(Some(ApplyOutcome {
        dest,
        inserted,
        unread_delta,
        msg: merged,
    }))
}

async fn find_by_mid(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    mid: i64,
) -> Result<Option<StoredMsg>, SdkError> {
    let row = sqlx::query(
        r"
        SELECT key, dest, sender, body, at, sys, kind, width, height,
               message_id, batch_id, status, local_path, thread_kind
        FROM messages
        WHERE account = ? AND dest = ? AND message_id = ? AND message_id != 0
        LIMIT 1
        ",
    )
    .bind(account)
    .bind(dest)
    .bind(mid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    row.map(|r| stored_from_row(&r)).transpose()
}

async fn find_by_key(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    key: &str,
) -> Result<Option<StoredMsg>, SdkError> {
    let row = sqlx::query(
        r"
        SELECT key, dest, sender, body, at, sys, kind, width, height,
               message_id, batch_id, status, local_path, thread_kind
        FROM messages
        WHERE account = ? AND dest = ? AND key = ?
        LIMIT 1
        ",
    )
    .bind(account)
    .bind(dest)
    .bind(key)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    row.map(|r| stored_from_row(&r)).transpose()
}

async fn delete_key(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    key: &str,
) -> Result<(), SdkError> {
    sqlx::query("DELETE FROM messages WHERE account = ? AND dest = ? AND key = ?")
        .bind(account)
        .bind(dest)
        .bind(key)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

async fn put_msg(tx: &mut SqliteConnection, account: &str, m: &StoredMsg) -> Result<(), SdkError> {
    sqlx::query(
        r"
        INSERT INTO messages (
          account, dest, key, sender, body, at, sys, failed, kind, width, height,
          message_id, batch_id, status, local_path, thread_kind
        ) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(account, dest, key) DO UPDATE SET
          sender = excluded.sender,
          body = excluded.body,
          at = excluded.at,
          sys = excluded.sys,
          kind = excluded.kind,
          width = excluded.width,
          height = excluded.height,
          message_id = CASE WHEN excluded.message_id != 0 THEN excluded.message_id ELSE messages.message_id END,
          batch_id = CASE WHEN excluded.batch_id != '' THEN excluded.batch_id ELSE messages.batch_id END,
          status = excluded.status,
          local_path = CASE WHEN excluded.local_path != '' THEN excluded.local_path ELSE messages.local_path END,
          thread_kind = excluded.thread_kind
        ",
    )
    .bind(account)
    .bind(&m.dest)
    .bind(&m.key)
    .bind(&m.sender)
    .bind(&m.body)
    .bind(m.at)
    .bind(i32::from(m.sys))
    .bind(&m.kind)
    .bind(m.width)
    .bind(m.height)
    .bind(m.message_id)
    .bind(&m.batch_id)
    .bind(&m.status)
    .bind(&m.local_path)
    .bind(m.thread_kind)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

fn merge_stored(prev: StoredMsg, next: StoredMsg) -> StoredMsg {
    let body = if is_remote_url(&prev.body) && !is_remote_url(&next.body) {
        prev.body
    } else {
        next.body
    };
    StoredMsg {
        key: prev.key,
        dest: prev.dest,
        sender: if next.sender.is_empty() {
            prev.sender
        } else {
            next.sender
        },
        body,
        at: if next.at == 0 { prev.at } else { next.at },
        sys: next.sys,
        kind: next.kind,
        width: if next.width == 0 {
            prev.width
        } else {
            next.width
        },
        height: if next.height == 0 {
            prev.height
        } else {
            next.height
        },
        message_id: if next.message_id == 0 {
            prev.message_id
        } else {
            next.message_id
        },
        batch_id: if next.batch_id.is_empty() {
            prev.batch_id
        } else {
            next.batch_id
        },
        status: next.status,
        local_path: if next.local_path.is_empty() {
            prev.local_path
        } else {
            next.local_path
        },
        thread_kind: if next.thread_kind == 0 {
            prev.thread_kind
        } else {
            next.thread_kind
        },
    }
}

fn stored_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<StoredMsg, SdkError> {
    let sys: i64 = row.try_get("sys").map_err(map_sqlx)?;
    Ok(StoredMsg {
        key: row.try_get("key").map_err(map_sqlx)?,
        dest: row.try_get("dest").map_err(map_sqlx)?,
        sender: row.try_get("sender").map_err(map_sqlx)?,
        body: row.try_get("body").map_err(map_sqlx)?,
        at: row.try_get("at").map_err(map_sqlx)?,
        sys: sys != 0,
        kind: row.try_get("kind").map_err(map_sqlx)?,
        width: row.try_get("width").map_err(map_sqlx)?,
        height: row.try_get("height").map_err(map_sqlx)?,
        message_id: row.try_get("message_id").map_err(map_sqlx)?,
        batch_id: row.try_get("batch_id").map_err(map_sqlx)?,
        status: row.try_get("status").map_err(map_sqlx)?,
        local_path: row.try_get("local_path").map_err(map_sqlx)?,
        thread_kind: row.try_get("thread_kind").map_err(map_sqlx)?,
    })
}

fn parse_image_extra(extra: &str) -> (i32, i32) {
    let raw = extra.trim();
    if !raw.starts_with('{') {
        return (0, 0);
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return (0, 0);
    };
    let w = v.get("w").and_then(serde_json::Value::as_i64).unwrap_or(0);
    let h = v.get("h").and_then(serde_json::Value::as_i64).unwrap_or(0);
    (
        w.clamp(0, i64::from(i32::MAX)) as i32,
        h.clamp(0, i64::from(i32::MAX)) as i32,
    )
}

fn is_remote_url(body: &str) -> bool {
    body.starts_with("http://") || body.starts_with("https://")
}
