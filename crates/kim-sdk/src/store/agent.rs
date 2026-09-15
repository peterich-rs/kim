use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::agent::{AgentProfileRow, DeviceOverlayRow, ProviderAccountRow};
use crate::error::{map_sqlx, SdkError};

pub(crate) async fn upsert_profile(
    tx: &mut SqliteConnection,
    account: &str,
    row: &AgentProfileRow,
    now: i64,
) -> Result<(), SdkError> {
    let updated_at = if row.updated_at > 0 {
        row.updated_at
    } else {
        now
    };
    let placement = if row.placement.trim().is_empty() {
        "local"
    } else {
        row.placement.trim()
    };
    sqlx::query(
        r"
        INSERT INTO agent_profiles (
          account, profile_id, nickname, server_account, body_json, body_blob, placement,
          key_ciphertext, updated_at, deleted_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        ON CONFLICT(account, profile_id) DO UPDATE SET
          nickname = excluded.nickname,
          server_account = excluded.server_account,
          body_json = excluded.body_json,
          body_blob = excluded.body_blob,
          placement = excluded.placement,
          updated_at = excluded.updated_at,
          deleted_at = excluded.deleted_at
        ",
    )
    .bind(account)
    .bind(&row.profile_id)
    .bind(&row.nickname)
    .bind(&row.server_account)
    .bind(&row.body_json)
    .bind(&row.body_blob)
    .bind(placement)
    .bind(updated_at)
    .bind(if row.deleted_at > 0 {
        Some(row.deleted_at)
    } else {
        None
    })
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn delete_profile(
    tx: &mut SqliteConnection,
    account: &str,
    profile_id: &str,
    now: i64,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        UPDATE agent_profiles
        SET deleted_at = ?, updated_at = ?
        WHERE account = ? AND profile_id = ?
        ",
    )
    .bind(now)
    .bind(now)
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
        SELECT profile_id, nickname, server_account, body_json, body_blob, placement,
               updated_at, deleted_at
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
        let blob: Option<Vec<u8>> = row.try_get("body_blob").map_err(map_sqlx)?;
        out.push(AgentProfileRow {
            profile_id: row.try_get("profile_id").map_err(map_sqlx)?,
            nickname: row.try_get("nickname").map_err(map_sqlx)?,
            server_account: row.try_get("server_account").map_err(map_sqlx)?,
            body_json: row.try_get("body_json").map_err(map_sqlx)?,
            body_blob: blob.unwrap_or_default(),
            placement: {
                let raw: String = row.try_get("placement").map_err(map_sqlx)?;
                if raw.trim().is_empty() {
                    "local".into()
                } else {
                    raw
                }
            },
            updated_at: row.try_get("updated_at").map_err(map_sqlx)?,
            deleted_at: row
                .try_get::<Option<i64>, _>("deleted_at")
                .map_err(map_sqlx)?
                .unwrap_or(0),
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
          AND (deleted_at IS NULL OR deleted_at = 0)
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
          AND (deleted_at IS NULL OR deleted_at = 0)
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
          account, profile_id, nickname, server_account, body_json, body_blob, placement,
          key_ciphertext, updated_at, deleted_at
        )
        SELECT ?, profile_id, nickname, server_account, body_json, body_blob, placement,
               key_ciphertext, updated_at, deleted_at
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
    sqlx::query(
        r"
        INSERT OR IGNORE INTO provider_accounts (
          account, id, vendor_id, base_url, key_ref, display_name, models_json,
          key_ciphertext, updated_at, deleted_at
        )
        SELECT ?, id, vendor_id, base_url, key_ref, display_name, models_json,
               key_ciphertext, updated_at, deleted_at
        FROM provider_accounts WHERE account = ?
        ",
    )
    .bind(to)
    .bind(from)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    sqlx::query("DELETE FROM provider_accounts WHERE account = ?")
        .bind(from)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    sqlx::query(
        r"
        INSERT OR IGNORE INTO agent_device_overlay (
          account, profile_id, workspace_path, workspace_bookmark, user_agents_skills
        )
        SELECT ?, profile_id, workspace_path, workspace_bookmark, user_agents_skills
        FROM agent_device_overlay WHERE account = ?
        ",
    )
    .bind(to)
    .bind(from)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    sqlx::query("DELETE FROM agent_device_overlay WHERE account = ?")
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

pub(crate) async fn upsert_provider_account(
    tx: &mut SqliteConnection,
    account: &str,
    row: &ProviderAccountRow,
    now: i64,
) -> Result<(), SdkError> {
    let updated_at = if row.updated_at > 0 {
        row.updated_at
    } else {
        now
    };
    let deleted_at = if row.deleted_at > 0 {
        Some(row.deleted_at)
    } else {
        None
    };
    sqlx::query(
        r"
        INSERT INTO provider_accounts (
          account, id, vendor_id, base_url, key_ref, display_name, models_json,
          key_ciphertext, updated_at, deleted_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        ON CONFLICT(account, id) DO UPDATE SET
          vendor_id = excluded.vendor_id,
          base_url = excluded.base_url,
          key_ref = excluded.key_ref,
          display_name = excluded.display_name,
          models_json = excluded.models_json,
          updated_at = excluded.updated_at,
          deleted_at = excluded.deleted_at
        ",
    )
    .bind(account)
    .bind(&row.id)
    .bind(&row.vendor_id)
    .bind(&row.base_url)
    .bind(&row.key_ref)
    .bind(&row.display_name)
    .bind(&row.models_json)
    .bind(updated_at)
    .bind(deleted_at)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn delete_provider_account(
    tx: &mut SqliteConnection,
    account: &str,
    id: &str,
    now: i64,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        UPDATE provider_accounts SET deleted_at = ?, updated_at = ?
        WHERE account = ? AND id = ?
        ",
    )
    .bind(now)
    .bind(now)
    .bind(account)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn load_provider_accounts(
    pool: &SqlitePool,
    account: &str,
) -> Result<Vec<ProviderAccountRow>, SdkError> {
    let rows = sqlx::query(
        r"
        SELECT id, vendor_id, base_url, key_ref, display_name, models_json, updated_at, deleted_at
        FROM provider_accounts
        WHERE account = ? AND (deleted_at IS NULL OR deleted_at = 0)
        ORDER BY display_name, id
        ",
    )
    .bind(account)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let deleted: Option<i64> = row.try_get("deleted_at").map_err(map_sqlx)?;
        out.push(ProviderAccountRow {
            id: row.try_get("id").map_err(map_sqlx)?,
            vendor_id: row.try_get("vendor_id").map_err(map_sqlx)?,
            base_url: row.try_get("base_url").map_err(map_sqlx)?,
            key_ref: row.try_get("key_ref").map_err(map_sqlx)?,
            display_name: row.try_get("display_name").map_err(map_sqlx)?,
            models_json: row.try_get("models_json").map_err(map_sqlx)?,
            updated_at: row.try_get("updated_at").map_err(map_sqlx)?,
            deleted_at: deleted.unwrap_or(0),
        });
    }
    Ok(out)
}

pub(crate) async fn load_provider_accounts_all(
    pool: &SqlitePool,
    account: &str,
) -> Result<Vec<ProviderAccountRow>, SdkError> {
    let rows = sqlx::query(
        r"
        SELECT id, vendor_id, base_url, key_ref, display_name, models_json, updated_at, deleted_at
        FROM provider_accounts WHERE account = ?
        ORDER BY display_name, id
        ",
    )
    .bind(account)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let deleted: Option<i64> = row.try_get("deleted_at").map_err(map_sqlx)?;
        out.push(ProviderAccountRow {
            id: row.try_get("id").map_err(map_sqlx)?,
            vendor_id: row.try_get("vendor_id").map_err(map_sqlx)?,
            base_url: row.try_get("base_url").map_err(map_sqlx)?,
            key_ref: row.try_get("key_ref").map_err(map_sqlx)?,
            display_name: row.try_get("display_name").map_err(map_sqlx)?,
            models_json: row.try_get("models_json").map_err(map_sqlx)?,
            updated_at: row.try_get("updated_at").map_err(map_sqlx)?,
            deleted_at: deleted.unwrap_or(0),
        });
    }
    Ok(out)
}

pub(crate) async fn upsert_overlay(
    tx: &mut SqliteConnection,
    account: &str,
    row: &DeviceOverlayRow,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        INSERT INTO agent_device_overlay (
          account, profile_id, workspace_path, workspace_bookmark, user_agents_skills
        ) VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(account, profile_id) DO UPDATE SET
          workspace_path = excluded.workspace_path,
          workspace_bookmark = excluded.workspace_bookmark,
          user_agents_skills = excluded.user_agents_skills
        ",
    )
    .bind(account)
    .bind(&row.profile_id)
    .bind(&row.workspace_path)
    .bind(&row.workspace_bookmark)
    .bind(&row.user_agents_skills)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn load_overlay(
    pool: &SqlitePool,
    account: &str,
    profile_id: &str,
) -> Result<Option<DeviceOverlayRow>, SdkError> {
    let row = sqlx::query(
        r"
        SELECT profile_id, workspace_path, workspace_bookmark, user_agents_skills
        FROM agent_device_overlay WHERE account = ? AND profile_id = ?
        ",
    )
    .bind(account)
    .bind(profile_id)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx)?;
    match row {
        Some(r) => Ok(Some(DeviceOverlayRow {
            profile_id: r.try_get("profile_id").map_err(map_sqlx)?,
            workspace_path: r.try_get("workspace_path").map_err(map_sqlx)?,
            workspace_bookmark: r.try_get("workspace_bookmark").map_err(map_sqlx)?,
            user_agents_skills: r.try_get("user_agents_skills").map_err(map_sqlx)?,
        })),
        None => Ok(None),
    }
}
