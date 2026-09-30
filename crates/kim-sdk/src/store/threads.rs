use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::error::{map_sqlx, SdkError};
use crate::model::{classify_message, thread_preview, MediaKind, ThreadKind};
use crate::timeline::ThreadView;

pub(crate) struct StoredThread {
    pub id: String,
    pub kind: ThreadKind,
    pub title: String,
    pub avatar: String,
    pub last_body: String,
    pub last_kind: MediaKind,
    pub last_sys: bool,
    pub last_at: i64,
    pub unread: i32,
    pub last_message_id: i64,
}

impl StoredThread {
    pub(crate) fn to_view(&self) -> ThreadView {
        ThreadView {
            id: self.id.clone(),
            kind: self.kind,
            title: self.title.clone(),
            avatar: self.avatar.clone(),
            last_body: self.last_body.clone(),
            preview: thread_preview(self.last_sys, self.last_kind, &self.last_body),
            last_at: self.last_at,
            unread: self.unread,
        }
    }
}

pub(crate) struct SendThread<'a> {
    pub account: &'a str,
    pub dest: &'a str,
    pub kind: ThreadKind,
    pub last_body: &'a str,
    pub last_at: i64,
    pub media: MediaKind,
    pub sys: bool,
}

pub(crate) async fn upsert_on_send(
    tx: &mut SqliteConnection,
    row: SendThread<'_>,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        INSERT INTO threads (
          account, id, kind, title, last_body, last_kind, last_sys, last_at, unread, avatar
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, '')
        ON CONFLICT(account, id) DO UPDATE SET
          last_body = excluded.last_body,
          last_kind = excluded.last_kind,
          last_sys = excluded.last_sys,
          last_at = CASE WHEN excluded.last_at >= threads.last_at THEN excluded.last_at ELSE threads.last_at END,
          kind = excluded.kind
        ",
    )
    .bind(row.account)
    .bind(row.dest)
    .bind(row.kind.as_db())
    .bind(row.dest)
    .bind(row.last_body)
    .bind(row.media.as_db())
    .bind(i32::from(row.sys))
    .bind(row.last_at)
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
    pub thread_kind: ThreadKind,
    pub media_kind: MediaKind,
    pub sys: bool,
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
    let newer = existing.as_ref().is_none_or(|t| msg_at >= t.last_at);
    let last_at = existing
        .as_ref()
        .map(|t| t.last_at.max(msg_at))
        .unwrap_or(msg_at);
    let last_body = if newer {
        incoming.last_body.to_string()
    } else {
        existing
            .as_ref()
            .map(|t| t.last_body.clone())
            .unwrap_or_default()
    };
    let last_kind = if newer {
        incoming.media_kind
    } else {
        existing
            .as_ref()
            .map(|t| t.last_kind)
            .unwrap_or(MediaKind::Text)
    };
    let last_sys = if newer {
        incoming.sys
    } else {
        existing.as_ref().is_some_and(|t| t.last_sys)
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
    let stored = StoredThread {
        id: dest.to_string(),
        kind,
        title,
        avatar,
        last_body,
        last_kind,
        last_sys,
        last_at,
        unread,
        last_message_id,
    };
    upsert_full(tx, account, &stored).await?;
    Ok(stored)
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
    let (last_body, last_kind, last_sys) = if item.last_body.is_empty() {
        (
            prev.as_ref()
                .map(|t| t.last_body.clone())
                .unwrap_or_default(),
            prev.as_ref()
                .map(|t| t.last_kind)
                .unwrap_or(MediaKind::Text),
            prev.as_ref().is_some_and(|t| t.last_sys),
        )
    } else {
        let classified = classify_message(kim_protocol::MESSAGE_TYPE_TEXT, &item.last_body);
        (item.last_body.clone(), classified.kind, false)
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
        kind: ThreadKind::from_wire(item.kind),
        title,
        avatar,
        last_body,
        last_kind,
        last_sys,
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
    kind: ThreadKind,
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
    .bind(kind.as_db())
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
        "SELECT id, kind, title, avatar, last_body, last_kind, last_sys, last_at, unread, last_message_id FROM threads WHERE account = ? AND id = ?",
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
        "SELECT id, kind, title, avatar, last_body, last_kind, last_sys, last_at, unread, last_message_id FROM threads WHERE account = ? ORDER BY last_at DESC",
    )
    .bind(account)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(thread_from_row(&row)?.to_view());
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
        INSERT INTO threads (
          account, id, kind, title, last_body, last_kind, last_sys, last_at, unread, avatar, last_message_id
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(account, id) DO UPDATE SET
          kind = excluded.kind,
          title = excluded.title,
          last_body = excluded.last_body,
          last_kind = excluded.last_kind,
          last_sys = excluded.last_sys,
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
    .bind(t.kind.as_db())
    .bind(&t.title)
    .bind(&t.last_body)
    .bind(t.last_kind.as_db())
    .bind(i32::from(t.last_sys))
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
    let media_raw: String = row.try_get("last_kind").map_err(map_sqlx)?;
    let sys: i64 = row.try_get("last_sys").map_err(map_sqlx)?;
    Ok(StoredThread {
        id: row.try_get("id").map_err(map_sqlx)?,
        kind: ThreadKind::from_db(&kind_raw),
        title: row.try_get("title").map_err(map_sqlx)?,
        avatar: row.try_get("avatar").map_err(map_sqlx)?,
        last_body: row.try_get("last_body").map_err(map_sqlx)?,
        last_kind: MediaKind::from_db(&media_raw)?,
        last_sys: sys != 0,
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
) -> Result<Option<(ThreadKind, i64)>, SdkError> {
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
    Ok(Some((ThreadKind::from_db(&kind_raw), last_message_id)))
}
