use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::error::{map_sqlx, SdkError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceSettings {
    pub ws_url: String,
    pub http_origin: String,
    pub env: String,
    pub locale: String,
    pub account: String,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            ws_url: String::new(),
            http_origin: String::new(),
            env: "prod".into(),
            locale: String::new(),
            account: String::new(),
        }
    }
}

pub(crate) async fn load_device(pool: &SqlitePool) -> Result<DeviceSettings, SdkError> {
    let row = sqlx::query(
        r"
        SELECT ws_url, http_origin, env, locale, account
        FROM settings WHERE account = ''
        ",
    )
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx)?;
    match row {
        Some(r) => Ok(DeviceSettings {
            ws_url: r.try_get("ws_url").map_err(map_sqlx)?,
            http_origin: r.try_get("http_origin").map_err(map_sqlx)?,
            env: r.try_get("env").map_err(map_sqlx)?,
            locale: r.try_get("locale").map_err(map_sqlx)?,
            account: r.try_get("account").map_err(map_sqlx)?,
        }),
        None => Ok(DeviceSettings::default()),
    }
}

pub(crate) async fn upsert_device(
    tx: &mut SqliteConnection,
    row: &DeviceSettings,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        INSERT INTO settings (account, ws_url, http_origin, env, locale, agent_flags)
        VALUES ('', ?, ?, ?, ?, '{}')
        ON CONFLICT(account) DO UPDATE SET
          ws_url = excluded.ws_url,
          http_origin = excluded.http_origin,
          env = excluded.env,
          locale = excluded.locale
        ",
    )
    .bind(&row.ws_url)
    .bind(&row.http_origin)
    .bind(&row.env)
    .bind(&row.locale)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn load_agent_flags(
    pool: &SqlitePool,
) -> Result<crate::model::AgentFlagRow, SdkError> {
    let row = sqlx::query("SELECT agent_flags FROM settings WHERE account = ''")
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx)?;
    match row {
        Some(r) => {
            let raw: String = r.try_get("agent_flags").map_err(map_sqlx)?;
            Ok(crate::model::AgentFlagRow::from_json(&raw))
        }
        None => Ok(crate::model::AgentFlagRow::default()),
    }
}

pub(crate) async fn upsert_agent_flags(
    tx: &mut SqliteConnection,
    flags: &crate::model::AgentFlagRow,
) -> Result<(), SdkError> {
    let flags = flags.to_json();
    sqlx::query(
        r"
        INSERT INTO settings (account, ws_url, http_origin, env, locale, agent_flags)
        VALUES ('', '', '', 'prod', '', ?)
        ON CONFLICT(account) DO UPDATE SET agent_flags = excluded.agent_flags
        ",
    )
    .bind(flags)
    .execute(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn imported_prefs(pool: &SqlitePool) -> Result<bool, SdkError> {
    let row = sqlx::query("SELECT value FROM meta WHERE key = 'imported_prefs'")
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx)?;
    Ok(row.is_some())
}

/// Vendor model cache under `meta` (`catalog_cache.<vendor>`): seeds a new
/// provider account from the last successful `/v1/models` fetch. Dart keeps
/// no copy.
pub(crate) async fn load_catalog_cache(
    pool: &SqlitePool,
    vendor: &str,
) -> Result<Vec<String>, SdkError> {
    let key = format!("catalog_cache.{vendor}");
    let row = sqlx::query("SELECT value FROM meta WHERE key = ?")
        .bind(&key)
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx)?;
    let Some(row) = row else {
        return Ok(Vec::new());
    };
    let raw: String = row.try_get("value").map_err(map_sqlx)?;
    match serde_json::from_str::<Vec<String>>(&raw) {
        Ok(models) => Ok(models),
        Err(_) => Ok(Vec::new()),
    }
}

pub(crate) async fn save_catalog_cache(
    pool: &SqlitePool,
    vendor: &str,
    models: &[String],
) -> Result<(), SdkError> {
    let key = format!("catalog_cache.{vendor}");
    let raw = serde_json::to_string(models).map_err(|err| SdkError::Internal {
        message: err.to_string(),
    })?;
    sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)")
        .bind(&key)
        .bind(&raw)
        .execute(pool)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

/// A resolved directory grant. Picker + security-scoped bookmark resolution
/// stay platform; the granted path itself is Rust-owned state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceGrant {
    pub profile_id: String,
    pub path: String,
    /// macOS security-scoped bookmark bytes (base64). Platform payload;
    /// Rust stores and hands it back, never interprets it.
    pub bookmark: String,
    pub granted_at: i64,
}

pub(crate) async fn upsert_grant(
    pool: &SqlitePool,
    profile_id: &str,
    path: &str,
    bookmark: &str,
) -> Result<(), SdkError> {
    sqlx::query(
        r"
        INSERT INTO workspace_grants (profile_id, path, bookmark, granted_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(profile_id) DO UPDATE SET
          path = excluded.path,
          bookmark = excluded.bookmark,
          granted_at = excluded.granted_at
        ",
    )
    .bind(profile_id)
    .bind(path)
    .bind(bookmark)
    .bind(crate::store::now_ms())
    .execute(pool)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn delete_grant(pool: &SqlitePool, profile_id: &str) -> Result<(), SdkError> {
    sqlx::query("DELETE FROM workspace_grants WHERE profile_id = ?")
        .bind(profile_id)
        .execute(pool)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}

pub(crate) async fn load_grant(
    pool: &SqlitePool,
    profile_id: &str,
) -> Result<Option<WorkspaceGrant>, SdkError> {
    let row = sqlx::query(
        "SELECT profile_id, path, bookmark, granted_at FROM workspace_grants WHERE profile_id = ?",
    )
    .bind(profile_id)
    .fetch_optional(pool)
    .await
    .map_err(map_sqlx)?;
    match row {
        Some(r) => Ok(Some(WorkspaceGrant {
            profile_id: r.try_get("profile_id").map_err(map_sqlx)?,
            path: r.try_get("path").map_err(map_sqlx)?,
            bookmark: r.try_get("bookmark").map_err(map_sqlx)?,
            granted_at: r.try_get("granted_at").map_err(map_sqlx)?,
        })),
        None => Ok(None),
    }
}

pub(crate) async fn load_grants(pool: &SqlitePool) -> Result<Vec<WorkspaceGrant>, SdkError> {
    let rows = sqlx::query("SELECT profile_id, path, bookmark, granted_at FROM workspace_grants")
        .fetch_all(pool)
        .await
        .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(WorkspaceGrant {
            profile_id: r.try_get("profile_id").map_err(map_sqlx)?,
            path: r.try_get("path").map_err(map_sqlx)?,
            bookmark: r.try_get("bookmark").map_err(map_sqlx)?,
            granted_at: r.try_get("granted_at").map_err(map_sqlx)?,
        });
    }
    Ok(out)
}

pub(crate) fn write_grant_index(
    db_path: &std::path::Path,
    grants: &[WorkspaceGrant],
) -> Result<(), SdkError> {
    let Some(parent) = db_path.parent() else {
        return Ok(());
    };
    let dir = parent.join("agent");
    std::fs::create_dir_all(&dir).map_err(|err| SdkError::Disk {
        message: err.to_string(),
    })?;
    let mut body = String::new();
    for grant in grants {
        if grant.profile_id.contains(['\t', '\n']) || grant.path.contains(['\t', '\n']) {
            continue;
        }
        body.push_str(&grant.profile_id);
        body.push('\t');
        body.push_str(&grant.path);
        body.push('\n');
    }
    std::fs::write(dir.join("workspace-grants.txt"), body).map_err(|err| SdkError::Disk {
        message: err.to_string(),
    })
}

pub(crate) async fn mark_imported(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES ('imported_prefs', '1')")
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}
