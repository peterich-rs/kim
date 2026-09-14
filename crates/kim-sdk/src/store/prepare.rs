use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::Row;

use super::schema::SCHEMA_VERSION;
use crate::error::{map_sqlx, SdkError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareOutcome {
    CreateEmpty { wiped: bool },
    Keep,
}

/// Wipe Dart-shaped or unreadable files. Keep kim-sdk v1+. Newer than SDK is a hard error.
pub fn prepare_store_file(path: &Path) -> Result<PrepareOutcome, SdkError> {
    if !path.exists() {
        return Ok(PrepareOutcome::CreateEmpty { wiped: false });
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| SdkError::Internal {
            message: format!("runtime: {e}"),
        })?;
    match rt.block_on(read_schema_version(path)) {
        Ok(v) if v < 1 => {
            wipe_store_files(path)?;
            Ok(PrepareOutcome::CreateEmpty { wiped: true })
        }
        Ok(v) if v <= SCHEMA_VERSION => Ok(PrepareOutcome::Keep),
        Ok(_) => Err(SdkError::InvalidArgument {
            message: "store newer than sdk".into(),
        }),
        Err(_) => {
            wipe_store_files(path)?;
            Ok(PrepareOutcome::CreateEmpty { wiped: true })
        }
    }
}

pub(crate) fn wipe_store_files(path: &Path) -> Result<(), SdkError> {
    let _ = std::fs::remove_file(path);
    let wal = path.with_extension("db-wal");
    let shm = path.with_extension("db-shm");
    // `kim-cache.db-wal` when path already ends in `.db`.
    let wal_alt = Path::new(&format!("{}-wal", path.display())).to_path_buf();
    let shm_alt = Path::new(&format!("{}-shm", path.display())).to_path_buf();
    let _ = std::fs::remove_file(wal);
    let _ = std::fs::remove_file(shm);
    let _ = std::fs::remove_file(wal_alt);
    let _ = std::fs::remove_file(shm_alt);
    Ok(())
}

async fn read_schema_version(path: &Path) -> Result<i64, SdkError> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(map_sqlx)?;
    let row = sqlx::query("SELECT value FROM meta WHERE key = 'schema_version'")
        .fetch_optional(&pool)
        .await;
    pool.close().await;
    let row = row.map_err(map_sqlx)?;
    match row {
        Some(r) => {
            let raw: String = r.try_get("value").map_err(map_sqlx)?;
            raw.parse::<i64>().map_err(|_| SdkError::InvalidArgument {
                message: "schema_version unparseable".into(),
            })
        }
        None => Ok(0),
    }
}
