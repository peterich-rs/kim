use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::agent::AgentProfileRow;
use crate::error::{map_sqlx, SdkError};

pub(crate) async fn upsert_profile(
    tx: &mut SqliteConnection,
    account: &str,
    row: &AgentProfileRow,
    now: i64,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        INSERT INTO agent_profiles (
          account, profile_id, nickname, server_account, body_json, key_ciphertext, updated_at
        ) VALUES (?, ?, ?, ?, ?, NULL, ?)
        ON CONFLICT(account, profile_id) DO UPDATE SET
          nickname = excluded.nickname,
          server_account = excluded.server_account,
          body_json = excluded.body_json,
          updated_at = excluded.updated_at
        ",
    )
    .bind(account)
    .bind(&row.profile_id)
    .bind(&row.nickname)
    .bind(&row.server_account)
    .bind(&row.body_json)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn delete_profile(
    tx: &mut SqliteConnection,
    account: &str,
    profile_id: &str,
) -> Result<(), SdkError> {
    sqlx::query("DELETE FROM agent_profiles WHERE account = ? AND profile_id = ?")
        .bind(account)
        .bind(profile_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query("DELETE FROM agent_permissions WHERE account = ? AND profile_id = ?")
        .bind(account)
        .bind(profile_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn load_all(
    pool: &SqlitePool,
    account: &str,
) -> Result<Vec<AgentProfileRow>, SdkError> {
    let rows = sqlx::query(
        r"
        SELECT profile_id, nickname, server_account, body_json
        FROM agent_profiles WHERE account = ?
        ORDER BY nickname, profile_id
        ",
    )
    .bind(account)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(AgentProfileRow {
            profile_id: row.try_get("profile_id").map_err(map_sqlx)?,
            nickname: row.try_get("nickname").map_err(map_sqlx)?,
            server_account: row.try_get("server_account").map_err(map_sqlx)?,
            body_json: row.try_get("body_json").map_err(map_sqlx)?,
        });
    }
    Ok(out)
}

pub(crate) async fn profile_id_for_dest(
    pool: &SqlitePool,
    account: &str,
    dest: &str,
) -> Result<Option<String>, SdkError> {
    let row = sqlx::query(
        r"
        SELECT profile_id FROM agent_profiles
        WHERE account = ? AND server_account = ? AND server_account != ''
        LIMIT 1
        ",
    )
    .bind(account)
    .bind(dest)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx)?;
    match row {
        Some(r) => Ok(Some(r.try_get("profile_id").map_err(map_sqlx)?)),
        None => Ok(None),
    }
}

pub(crate) async fn owned_dests(pool: &SqlitePool, account: &str) -> Result<Vec<String>, SdkError> {
    let rows = sqlx::query(
        r"
        SELECT server_account FROM agent_profiles
        WHERE account = ? AND server_account != ''
        ",
    )
    .bind(account)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(row.try_get("server_account").map_err(map_sqlx)?);
    }
    Ok(out)
}

pub(crate) async fn imported(pool: &SqlitePool) -> Result<bool, SdkError> {
    let row = sqlx::query("SELECT value FROM meta WHERE key = 'imported_agent_profiles'")
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx)?;
    Ok(row.is_some())
}

pub(crate) async fn rekey(tx: &mut SqliteConnection, from: &str, to: &str) -> Result<(), SdkError> {
    if from == to {
        return Ok(());
    }
    sqlx::query(
        r"
        INSERT OR IGNORE INTO agent_profiles (
          account, profile_id, nickname, server_account, body_json, key_ciphertext, updated_at
        )
        SELECT ?, profile_id, nickname, server_account, body_json, key_ciphertext, updated_at
        FROM agent_profiles WHERE account = ?
        ",
    )
    .bind(to)
    .bind(from)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    sqlx::query("DELETE FROM agent_profiles WHERE account = ?")
        .bind(from)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query(
        r"
        INSERT OR IGNORE INTO agent_permissions (
          account, profile_id, tool, decision, updated_at
        )
        SELECT ?, profile_id, tool, decision, updated_at
        FROM agent_permissions WHERE account = ?
        ",
    )
    .bind(to)
    .bind(from)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    sqlx::query("DELETE FROM agent_permissions WHERE account = ?")
        .bind(from)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn mark_imported(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES ('imported_agent_profiles', '1')")
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}
