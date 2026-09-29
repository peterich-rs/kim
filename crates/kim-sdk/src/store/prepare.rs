use std::path::Path;

use super::schema::SCHEMA_VERSION;
use crate::error::{map_sqlx, SdkError};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareOutcome {
    CreateEmpty { wiped: bool },
    Keep,
}

/// Wipe a client file whose `user_version` is older than this SDK. There is
/// no column-compat migration. Newer than the SDK is a hard error.
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
    match rt.block_on(read_user_version(path)) {
        Ok(v) if v == SCHEMA_VERSION => Ok(PrepareOutcome::Keep),
        Ok(v) if v > SCHEMA_VERSION => Err(SdkError::InvalidArgument {
            message: "store newer than sdk".into(),
        }),
        Ok(_) => {
            wipe_store_files(path)?;
            Ok(PrepareOutcome::CreateEmpty { wiped: true })
        }
        Err(_) => {
            wipe_store_files(path)?;
            Ok(PrepareOutcome::CreateEmpty { wiped: true })
        }
    }
}

fn remove_optional(path: &Path) -> Result<(), SdkError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(SdkError::Disk {
            message: format!("wipe {}: {e}", path.display()),
        }),
    }
}

pub(crate) fn wipe_store_files(path: &Path) -> Result<(), SdkError> {
    remove_optional(path)?;
    let wal = path.with_extension("db-wal");
    let shm = path.with_extension("db-shm");
    let wal_alt = Path::new(&format!("{}-wal", path.display())).to_path_buf();
    let shm_alt = Path::new(&format!("{}-shm", path.display())).to_path_buf();
    remove_optional(&wal)?;
    remove_optional(&shm)?;
    remove_optional(&wal_alt)?;
    remove_optional(&shm_alt)?;
    if path.exists() {
        return Err(SdkError::Disk {
            message: "store wipe failed; file still exists".into(),
        });
    }
    Ok(())
}

async fn read_user_version(path: &Path) -> Result<i64, SdkError> {
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
    let version = sqlx::query_scalar::<_, i64>("PRAGMA user_version")
        .fetch_one(&pool)
        .await;
    pool.close().await;
    version.map_err(map_sqlx)
}
