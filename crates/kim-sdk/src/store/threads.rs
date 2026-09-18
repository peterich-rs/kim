use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::error::{map_sqlx, SdkError};
use crate::timeline::{thread_kind_from_name, thread_kind_name, ThreadView};

pub(crate) struct StoredThread {
    pub id: String,
    pub kind: i32,
    pub title: String,
    pub avatar: String,
    pub last_body: String,
    pub last_at: i64,
    pub unread: i32,
    pub last_message_id: i64,
}

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

pub(crate) struct IncomingApply<'a> {
    pub dest: &'a str,
    pub last_body: &'a str,
    pub last_at: i64,
    pub unread_delta: i32,
    pub thread_kind: i32,
    pub last_message_id: i64,
}

pub(crate) async fn apply_incoming(
    tx: &mut SqliteConnection,
    account: &str,
    incoming: IncomingApply<'_>,
) -> Result<StoredThread, SdkError> {
    let dest = incoming.dest;
    let existing = find(tx, account, dest).await?;
    let msg_at = incoming.last_at;
    let last_at = existing
        .as_ref()
        .map(|t| t.last_at.max(msg_at))
        .unwrap_or(msg_at);
    let last_body = if existing.as_ref().is_none_or(|t| msg_at >= t.last_at) {
        incoming.last_body.to_string()
    } else {
        existing
            .as_ref()
            .map(|t| t.last_body.clone())
            .unwrap_or_default()
    };
    let unread = (existing.as_ref().map(|t| t.unread).unwrap_or(0) + incoming.unread_delta).max(0);
    let kind = existing
        .as_ref()
        .map(|t| t.kind)
        .unwrap_or(incoming.thread_kind);
    let title = existing
        .as_ref()
        .map(|t| t.title.clone())
        .unwrap_or_else(|| dest.to_string());
    let avatar = existing
        .as_ref()
        .map(|t| t.avatar.clone())
        .unwrap_or_default();
    let last_message_id = existing
        .as_ref()
        .map(|t| t.last_message_id)
        .unwrap_or(0)
        .max(incoming.last_message_id);
    upsert_full(
        tx,
        account,
        &StoredThread {
            id: dest.to_string(),
            kind,
            title: title.clone(),
            avatar: avatar.clone(),
            last_body: last_body.clone(),
            last_at,
            unread,
            last_message_id,
        },
    )
    .await?;
    Ok(StoredThread {
        id: dest.to_string(),
        kind,
        title,
        avatar,
        last_body,
        last_at,
        unread,
        last_message_id,
    })
}

pub(crate) async fn persist_inbox_item(
    tx: &mut SqliteConnection,
    account: &str,
    item: &kim_client::InboxItem,
) -> Result<StoredThread, SdkError> {
    let prev = find(tx, account, &item.dest).await?;
    let incoming_at = super::send_time_ms(item.last_send_time);
    let local_read = super::watermarks::effective_read(tx, account, &item.dest).await?;
    let unread = merged_unread(prev.as_ref(), item, incoming_at, local_read);
    let title = if item.title.is_empty() {
        prev.as_ref()
            .map(|t| t.title.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| item.dest.clone())
    } else {
        item.title.clone()
    };
    let last_body = if item.last_body.is_empty() {
        prev.as_ref()
            .map(|t| t.last_body.clone())
            .unwrap_or_default()
    } else {
        item.last_body.clone()
    };
    let last_at = if item.last_send_time == 0 {
        prev.as_ref().map(|t| t.last_at).unwrap_or(0)
    } else {
        incoming_at
    };
    let avatar = if item.avatar.is_empty() {
        prev.as_ref().map(|t| t.avatar.clone()).unwrap_or_default()
    } else {
        item.avatar.clone()
    };
    let last_message_id = item
        .last_message_id
        .max(prev.as_ref().map(|t| t.last_message_id).unwrap_or(0));
    let thread = StoredThread {
        id: item.dest.clone(),
        kind: item.kind,
        title,
        avatar,
        last_body,
        last_at,
        unread,
        last_message_id,
    };
    upsert_full(tx, account, &thread).await?;
    Ok(thread)
}

fn merged_unread(
    prev: Option<&StoredThread>,
    item: &kim_client::InboxItem,
    incoming_at: i64,
    local_read: i64,
) -> i32 {
    let effective_read = local_read.max(item.last_read_message_id);
    let known_tip = item
        .max_message_id
        .max(item.last_message_id)
        .max(prev.map(|t| t.last_message_id).unwrap_or(0));
    if effective_read > 0 && known_tip > 0 && effective_read >= known_tip {
        return 0;
    }
    if local_read > item.last_read_message_id {
        if let Some(prev) = prev {
            if prev.unread == 0 {
                return 0;
            }
        }
    }
    let _ = incoming_at;
    item.unread
}

pub(crate) async fn ensure(
    tx: &mut SqliteConnection,
    account: &str,
    id: &str,
    kind: i32,
) -> Result<StoredThread, SdkError> {
    sqlx::query(
        r"
        INSERT INTO threads (account, id, kind, title, last_body, last_at, unread, avatar)
        VALUES (?, ?, ?, ?, '', 0, 0, '')
        ON CONFLICT(account, id) DO NOTHING
        ",
    )
    .bind(account)
    .bind(id)
    .bind(thread_kind_name(kind))
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    find(tx, account, id)
        .await?
        .ok_or_else(|| SdkError::Internal {
            message: "ensure thread missing after insert".into(),
        })
}

pub(crate) async fn find(
    tx: &mut SqliteConnection,
    account: &str,
    id: &str,
) -> Result<Option<StoredThread>, SdkError> {
    let row = sqlx::query(
        "SELECT id, kind, title, avatar, last_body, last_at, unread, last_message_id FROM threads WHERE account = ? AND id = ?",
    )
    .bind(account)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    row.map(|r| thread_from_row(&r)).transpose()
}

pub(crate) async fn load_all(
    pool: &SqlitePool,
    account: &str,
) -> Result<Vec<ThreadView>, SdkError> {
    let rows = sqlx::query(
        "SELECT id, kind, title, avatar, last_body, last_at, unread, last_message_id FROM threads WHERE account = ? ORDER BY last_at DESC",
    )
    .bind(account)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let t = thread_from_row(&row)?;
        out.push(ThreadView {
            id: t.id,
            kind: t.kind,
            title: t.title,
            avatar: t.avatar,
            last_body: t.last_body,
            last_at: t.last_at,
            unread: t.unread,
        });
    }
    Ok(out)
}

async fn upsert_full(
    tx: &mut SqliteConnection,
    account: &str,
    t: &StoredThread,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        INSERT INTO threads (account, id, kind, title, last_body, last_at, unread, avatar, last_message_id)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(account, id) DO UPDATE SET
          kind = excluded.kind,
          title = excluded.title,
          last_body = excluded.last_body,
          last_at = excluded.last_at,
          unread = excluded.unread,
          avatar = CASE WHEN excluded.avatar != '' THEN excluded.avatar ELSE threads.avatar END,
          last_message_id = CASE
            WHEN excluded.last_message_id >= threads.last_message_id THEN excluded.last_message_id
            ELSE threads.last_message_id
          END
        ",
    )
    .bind(account)
    .bind(&t.id)
    .bind(thread_kind_name(t.kind))
    .bind(&t.title)
    .bind(&t.last_body)
    .bind(t.last_at)
    .bind(t.unread)
    .bind(&t.avatar)
    .bind(t.last_message_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

fn thread_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<StoredThread, SdkError> {
    let kind_raw: String = row.try_get("kind").map_err(map_sqlx)?;
    Ok(StoredThread {
        id: row.try_get("id").map_err(map_sqlx)?,
        kind: thread_kind_from_name(&kind_raw),
        title: row.try_get("title").map_err(map_sqlx)?,
        avatar: row.try_get("avatar").map_err(map_sqlx)?,
        last_body: row.try_get("last_body").map_err(map_sqlx)?,
        last_at: row.try_get("last_at").map_err(map_sqlx)?,
        unread: row.try_get("unread").map_err(map_sqlx)?,
        last_message_id: row.try_get("last_message_id").map_err(map_sqlx)?,
    })
}

/// Inbox tip for `dest`, if the thread row exists.
pub(crate) async fn server_tip(
    pool: &SqlitePool,
    account: &str,
    dest: &str,
) -> Result<Option<(i32, i64)>, SdkError> {
    let row = sqlx::query("SELECT kind, last_message_id FROM threads WHERE account = ? AND id = ?")
        .bind(account)
        .bind(dest)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let kind_raw: String = row.try_get("kind").map_err(map_sqlx)?;
    let last_message_id: i64 = row.try_get("last_message_id").map_err(map_sqlx)?;
    Ok(Some((thread_kind_from_name(&kind_raw), last_message_id)))
}
