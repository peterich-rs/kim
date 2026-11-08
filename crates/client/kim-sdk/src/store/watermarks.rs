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

pub(crate) async fn peek(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
) -> Result<(i32, i64), SdkError> {
    let unread = sqlx::query_scalar::<_, i32>(
        "SELECT IFNULL((SELECT unread FROM threads WHERE account = ? AND id = ?), 0)",
    )
    .bind(account)
    .bind(dest)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    let last_read = effective_read(tx, account, dest).await?;
    Ok((unread, last_read))
}

pub(crate) async fn effective_read(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
) -> Result<i64, SdkError> {
    let local: i64 = sqlx::query_scalar(
        "SELECT IFNULL((SELECT last_read_message_id FROM read_watermarks WHERE account = ? AND dest = ?), 0)",
    )
    .bind(account)
    .bind(dest)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    let server: i64 = sqlx::query_scalar(
        "SELECT IFNULL((SELECT server_read_id FROM conversation_read_state WHERE account = ? AND dest = ?), 0)",
    )
    .bind(account)
    .bind(dest)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(local.max(server))
}

pub(crate) async fn known_tip(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
) -> Result<i64, SdkError> {
    let thread_tip: i64 = sqlx::query_scalar(
        "SELECT IFNULL((SELECT last_message_id FROM threads WHERE account = ? AND id = ?), 0)",
    )
    .bind(account)
    .bind(dest)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    let msg_tip: i64 = sqlx::query_scalar(
        "SELECT IFNULL((SELECT MAX(message_id) FROM messages WHERE account = ? AND dest = ?), 0)",
    )
    .bind(account)
    .bind(dest)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    let state_tip: i64 = sqlx::query_scalar(
        "SELECT IFNULL((SELECT known_max_message_id FROM conversation_read_state WHERE account = ? AND dest = ?), 0)",
    )
    .bind(account)
    .bind(dest)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(thread_tip.max(msg_tip).max(state_tip))
}

pub(crate) async fn bump_known_max(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    kind: i32,
    message_id: i64,
) -> Result<(), SdkError> {
    if message_id <= 0 {
        return Ok(());
    }
    sqlx::query(
        r"
        INSERT INTO conversation_read_state (
          account, dest, kind, known_max_message_id, local_generation
        ) VALUES (?, ?, ?, ?, 1)
        ON CONFLICT(account, dest) DO UPDATE SET
          kind = excluded.kind,
          known_max_message_id = MAX(conversation_read_state.known_max_message_id, excluded.known_max_message_id),
          local_generation = conversation_read_state.local_generation + 1
        ",
    )
    .bind(account)
    .bind(dest)
    .bind(kind)
    .bind(message_id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn local_generation(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
) -> Result<i64, SdkError> {
    sqlx::query_scalar(
        "SELECT IFNULL((SELECT local_generation FROM conversation_read_state WHERE account = ? AND dest = ?), 0)",
    )
    .bind(account)
    .bind(dest)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx)
}

/// Advance the local watermark without blindly clearing unread.
pub(crate) async fn advance_id(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    kind: i32,
    message_id: i64,
    now: i64,
) -> Result<i64, SdkError> {
    if message_id <= 0 {
        return effective_read(tx, account, dest).await;
    }
    sqlx::query(
        r"
        INSERT INTO read_watermarks (account, dest, last_read_message_id, last_read_at, kind)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(account, dest) DO UPDATE SET
          last_read_message_id = MAX(read_watermarks.last_read_message_id, excluded.last_read_message_id),
          last_read_at = excluded.last_read_at,
          kind = excluded.kind
        ",
    )
    .bind(account)
    .bind(dest)
    .bind(message_id)
    .bind(now)
    .bind(kind)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    bump_known_max(tx, account, dest, kind, message_id).await?;
    effective_read(tx, account, dest).await
}

pub(crate) async fn queue_sync(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    kind: i32,
    message_id: i64,
    now: i64,
) -> Result<(), SdkError> {
    if message_id <= 0 {
        return Ok(());
    }
    sqlx::query(
        r"
        INSERT INTO read_watermarks (
          account, dest, last_read_message_id, last_read_at, kind,
          confirmed_message_id, retry_count, next_retry_at, last_error
        ) VALUES (?, ?, ?, ?, ?, 0, 0, ?, '')
        ON CONFLICT(account, dest) DO UPDATE SET
          last_read_message_id = MAX(read_watermarks.last_read_message_id, excluded.last_read_message_id),
          last_read_at = excluded.last_read_at,
          kind = excluded.kind,
          next_retry_at = CASE
            WHEN read_watermarks.last_read_message_id > read_watermarks.confirmed_message_id
              AND read_watermarks.next_retry_at > 0
            THEN MIN(read_watermarks.next_retry_at, excluded.next_retry_at)
            ELSE excluded.next_retry_at
          END
        ",
    )
    .bind(account)
    .bind(dest)
    .bind(message_id)
    .bind(now)
    .bind(kind)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn confirm(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    confirmed_id: i64,
    server: Option<&kim_client::ConversationReadState>,
) -> Result<(), SdkError> {
    if confirmed_id <= 0 {
        return Ok(());
    }
    sqlx::query(
        r"
        UPDATE read_watermarks
           SET confirmed_message_id = MAX(confirmed_message_id, ?),
               retry_count = 0,
               next_retry_at = 0,
               last_error = ''
         WHERE account = ? AND dest = ?
        ",
    )
    .bind(confirmed_id)
    .bind(account)
    .bind(dest)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    if let Some(state) = server {
        apply_server_state(tx, account, dest, state, true).await?;
    }
    Ok(())
}

pub(crate) async fn apply_server_state(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    state: &kim_client::ConversationReadState,
    from_own_ack: bool,
) -> Result<(), SdkError> {
    if dest.is_empty() && state.dest.is_empty() {
        return Ok(());
    }
    let dest = if dest.is_empty() {
        state.dest.as_str()
    } else {
        dest
    };
    let local_read = effective_read(tx, account, dest).await?;
    let stored_version: i64 = sqlx::query_scalar(
        "SELECT IFNULL((SELECT server_version FROM conversation_read_state WHERE account = ? AND dest = ?), 0)",
    )
    .bind(account)
    .bind(dest)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    let incoming_version = i64::try_from(state.version).unwrap_or(i64::MAX);
    let version_ok = incoming_version == 0 || incoming_version >= stored_version;
    sqlx::query(
        r"
        INSERT INTO conversation_read_state (
          account, dest, kind, server_version, server_read_id, server_max_message_id,
          server_unread, known_max_message_id, local_generation
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1)
        ON CONFLICT(account, dest) DO UPDATE SET
          kind = excluded.kind,
          server_version = CASE
            WHEN excluded.server_version = 0 THEN conversation_read_state.server_version
            WHEN excluded.server_version >= conversation_read_state.server_version THEN excluded.server_version
            ELSE conversation_read_state.server_version
          END,
          server_read_id = MAX(conversation_read_state.server_read_id, excluded.server_read_id),
          server_max_message_id = MAX(conversation_read_state.server_max_message_id, excluded.server_max_message_id),
          server_unread = CASE
            WHEN excluded.server_version = 0 THEN conversation_read_state.server_unread
            WHEN excluded.server_version >= conversation_read_state.server_version THEN excluded.server_unread
            ELSE conversation_read_state.server_unread
          END,
          known_max_message_id = MAX(conversation_read_state.known_max_message_id, excluded.known_max_message_id)
        ",
    )
    .bind(account)
    .bind(dest)
    .bind(state.kind)
    .bind(incoming_version)
    .bind(state.last_read_message_id)
    .bind(state.max_message_id)
    .bind(state.unread)
    .bind(state.max_message_id.max(state.last_read_message_id))
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    if state.last_read_message_id > 0 {
        sqlx::query(
            r"
            INSERT INTO read_watermarks (account, dest, last_read_message_id, last_read_at, kind)
            VALUES (?, ?, ?, 0, ?)
            ON CONFLICT(account, dest) DO UPDATE SET
              last_read_message_id = MAX(read_watermarks.last_read_message_id, excluded.last_read_message_id),
              kind = excluded.kind
            ",
        )
        .bind(account)
        .bind(dest)
        .bind(state.last_read_message_id)
        .bind(state.kind)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    }
    let effective = effective_read(tx, account, dest).await?;
    let tip = known_tip(tx, account, dest).await?;
    if effective >= tip && tip > 0 {
        set_unread(tx, account, dest, 0).await?;
    } else if version_ok && from_own_ack && state.last_read_message_id >= local_read {
        set_unread(tx, account, dest, state.unread.max(0)).await?;
    } else if version_ok && state.last_read_message_id >= local_read && effective >= tip {
        set_unread(tx, account, dest, 0).await?;
    }
    let _ = (from_own_ack, local_read);
    Ok(())
}

pub(crate) async fn set_unread(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    unread: i32,
) -> Result<(), SdkError> {
    sqlx::query("UPDATE threads SET unread = ? WHERE account = ? AND id = ?")
        .bind(unread.max(0))
        .bind(account)
        .bind(dest)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn mark_due_failed(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    now: i64,
    error: &str,
) -> Result<(), SdkError> {
    let retry: i64 = sqlx::query_scalar(
        "SELECT IFNULL((SELECT retry_count FROM read_watermarks WHERE account = ? AND dest = ?), 0)",
    )
    .bind(account)
    .bind(dest)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    let shift = u32::try_from(retry.clamp(0, 6)).unwrap_or(0);
    let delay = 1_000i64.saturating_mul(1i64 << shift).clamp(1_000, 60_000);
    let jitter = (now.unsigned_abs() % 250) as i64;
    sqlx::query(
        r"
        UPDATE read_watermarks
           SET retry_count = retry_count + 1,
               next_retry_at = ?,
               last_error = ?
         WHERE account = ? AND dest = ?
        ",
    )
    .bind(now.saturating_add(delay).saturating_add(jitter))
    .bind(error)
    .bind(account)
    .bind(dest)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

#[derive(Clone, Debug)]
pub(crate) struct DueRead {
    pub dest: String,
    pub kind: i32,
    pub message_id: i64,
}

pub(crate) async fn mark_due_failed_on_pool(
    pool: &SqlitePool,
    account: &str,
    dest: &str,
    error: &str,
) -> Result<(), SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .map_err(map_sqlx)?;
    let result = mark_due_failed(&mut conn, account, dest, super::now_ms(), error).await;
    match result {
        Ok(()) => {
            sqlx::query("COMMIT")
                .execute(&mut *conn)
                .await
                .map_err(map_sqlx)?;
            Ok(())
        }
        Err(err) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            Err(err)
        }
    }
}

pub(crate) async fn due(
    pool: &SqlitePool,
    account: &str,
    now: i64,
) -> Result<Vec<DueRead>, SdkError> {
    let rows = sqlx::query(
        r"
        SELECT dest, kind, last_read_message_id
          FROM read_watermarks
         WHERE account = ?
           AND last_read_message_id > confirmed_message_id
           AND last_read_message_id > 0
           AND (next_retry_at = 0 OR next_retry_at <= ?)
         ORDER BY last_read_at ASC
         LIMIT 20
        ",
    )
    .bind(account)
    .bind(now)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(DueRead {
            dest: row.try_get("dest").map_err(map_sqlx)?,
            kind: row.try_get("kind").map_err(map_sqlx)?,
            message_id: row.try_get("last_read_message_id").map_err(map_sqlx)?,
        });
    }
    Ok(out)
}

/// Legacy entry used by specified-ID reads. Does not clear newer unread.
pub(crate) async fn advance(
    tx: &mut SqliteConnection,
    account: &str,
    dest: &str,
    message_id: i64,
    now: i64,
) -> Result<(), SdkError> {
    if message_id <= 0 {
        return Ok(());
    }
    let kind: i32 = sqlx::query_scalar(
        "SELECT IFNULL((SELECT CASE kind WHEN 'group' THEN 1 ELSE 0 END FROM threads WHERE account = ? AND id = ?), 0)",
    )
    .bind(account)
    .bind(dest)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    let watermark = advance_id(tx, account, dest, kind, message_id, now).await?;
    queue_sync(tx, account, dest, kind, watermark, now).await?;
    let tip = known_tip(tx, account, dest).await?;
    if watermark >= tip {
        set_unread(tx, account, dest, 0).await?;
    }
    Ok(())
}
