use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::error::{map_sqlx, SdkError};
use crate::store::schema::MEDIA_CACHE_CAP_BYTES;

#[derive(Clone, Debug)]
pub struct MediaCacheRow {
    pub local_path: String,
}

pub(crate) async fn lookup(
    pool: &SqlitePool,
    url: &str,
) -> Result<Option<MediaCacheRow>, SdkError> {
    let row = sqlx::query("SELECT url, local_path, byte_size FROM media_cache WHERE url = ?")
        .bind(url)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx)?;
    match row {
        Some(r) => Ok(Some(MediaCacheRow {
            local_path: r.try_get("local_path").map_err(map_sqlx)?,
        })),
        None => Ok(None),
    }
}

pub(crate) async fn touch(tx: &mut SqliteConnection, url: &str, now: i64) -> Result<(), SdkError> {
    sqlx::query("UPDATE media_cache SET last_access = ? WHERE url = ?")
        .bind(now)
        .bind(url)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn upsert(
    tx: &mut SqliteConnection,
    url: &str,
    local_path: &str,
    byte_size: i64,
    now: i64,
) -> Result<Vec<String>, SdkError> {
    sqlx::query(
        r"
        INSERT INTO media_cache (url, local_path, byte_size, last_access)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(url) DO UPDATE SET
          local_path = excluded.local_path,
          byte_size = excluded.byte_size,
          last_access = excluded.last_access
        ",
    )
    .bind(url)
    .bind(local_path)
    .bind(byte_size)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    evict(tx).await
}

async fn evict(tx: &mut SqliteConnection) -> Result<Vec<String>, SdkError> {
    let total: i64 = sqlx::query("SELECT COALESCE(SUM(byte_size), 0) FROM media_cache")
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .try_get(0)
        .map_err(map_sqlx)?;
    if total <= MEDIA_CACHE_CAP_BYTES {
        return Ok(vec![]);
    }
    let mut evicted = Vec::new();
    let mut remaining = total;
    let rows =
        sqlx::query("SELECT url, local_path, byte_size FROM media_cache ORDER BY last_access ASC")
            .fetch_all(&mut *tx)
            .await
            .map_err(map_sqlx)?;
    for row in rows {
        if remaining <= MEDIA_CACHE_CAP_BYTES {
            break;
        }
        let url: String = row.try_get("url").map_err(map_sqlx)?;
        let path: String = row.try_get("local_path").map_err(map_sqlx)?;
        let size: i64 = row.try_get("byte_size").map_err(map_sqlx)?;
        sqlx::query("DELETE FROM media_cache WHERE url = ?")
            .bind(&url)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        evicted.push(path);
        remaining = remaining.saturating_sub(size);
    }
    Ok(evicted)
}

pub(crate) async fn search_messages(
    pool: &SqlitePool,
    account: &str,
    query: &str,
    dest: Option<&str>,
    cap: i32,
) -> Result<Vec<(String, String, String, String, i64, i64)>, SdkError> {
    let like = format!("%{}%", query.replace('%', r"\%").replace('_', r"\_"));
    let limit = cap.max(1);
    let rows = if let Some(dest) = dest {
        sqlx::query(
            r"
            SELECT dest, key, sender, body, at, message_id
            FROM messages
            WHERE account = ? AND dest = ? AND body LIKE ? ESCAPE '\'
            ORDER BY at DESC
            LIMIT ?
            ",
        )
        .bind(account)
        .bind(dest)
        .bind(&like)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx)?
    } else {
        sqlx::query(
            r"
            SELECT dest, key, sender, body, at, message_id
            FROM messages
            WHERE account = ? AND body LIKE ? ESCAPE '\'
            ORDER BY at DESC
            LIMIT ?
            ",
        )
        .bind(account)
        .bind(&like)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx)?
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push((
            row.try_get("dest").map_err(map_sqlx)?,
            row.try_get("key").map_err(map_sqlx)?,
            row.try_get("sender").map_err(map_sqlx)?,
            row.try_get("body").map_err(map_sqlx)?,
            row.try_get("at").map_err(map_sqlx)?,
            row.try_get("message_id").map_err(map_sqlx)?,
        ));
    }
    Ok(out)
}
