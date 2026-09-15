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

pub(crate) async fn imported_prefs(pool: &SqlitePool) -> Result<bool, SdkError> {
    let row = sqlx::query("SELECT value FROM meta WHERE key = 'imported_prefs'")
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx)?;
    Ok(row.is_some())
}

pub(crate) async fn mark_imported(tx: &mut SqliteConnection) -> Result<(), SdkError> {
    sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES ('imported_prefs', '1')")
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(())
}
