use crate::addresses::{ListAddr, ValueAddr};
use crate::traits::{now_ms, CommitResult, Storage, StorageError, StorageResult, Write};
use async_trait::async_trait;
use kim_agent_types::{Entry, EntryId, JsonValue, UsageId, UsageRow};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

pub struct SqliteStorage {
    pool: SqlitePool,
}

impl SqliteStorage {
    pub async fn open(path: &str) -> StorageResult<Self> {
        let opts = SqliteConnectOptions::from_str(path)
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        let s = Self { pool };
        s.migrate().await?;
        Ok(s)
    }

    pub async fn open_in_memory() -> StorageResult<Self> {
        Self::open("sqlite::memory:").await
    }

    async fn migrate(&self) -> StorageResult<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS entries (
                id TEXT PRIMARY KEY,
                parent_id TEXT,
                seq INTEGER NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS values_store (
                namespace TEXT NOT NULL,
                key TEXT NOT NULL,
                payload TEXT NOT NULL,
                PRIMARY KEY (namespace, key)
            );
            CREATE TABLE IF NOT EXISTS lists_store (
                namespace TEXT NOT NULL,
                key TEXT NOT NULL,
                idx INTEGER NOT NULL,
                payload TEXT NOT NULL,
                PRIMARY KEY (namespace, key, idx)
            );
            CREATE TABLE IF NOT EXISTS usage_store (
                id TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            );
            INSERT OR IGNORE INTO meta(key, value) VALUES('next_seq', 0);
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    async fn commit(&self, writes: Vec<Write>) -> StorageResult<CommitResult> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let row = sqlx::query("SELECT value FROM meta WHERE key = 'next_seq'")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        let mut next_seq: i64 = row.get(0);
        let ts = now_ms();
        let first_seq = (next_seq + 1) as u64;
        let mut seqs = Vec::new();

        for w in writes {
            next_seq += 1;
            seqs.push(next_seq as u64);
            match w {
                Write::InsertEntry(mut entry) => {
                    let id = entry.id();
                    entry.base_mut().seq = next_seq as u64;
                    entry.base_mut().timestamp_ms = ts;
                    let payload = serde_json::to_string(&entry)
                        .map_err(|e| StorageError::Corrupt(e.to_string()))?;
                    let parent = entry.parent_id().map(|p| p.to_string());
                    let res = sqlx::query(
                        "INSERT INTO entries(id, parent_id, seq, timestamp_ms, payload) VALUES(?,?,?,?,?)",
                    )
                    .bind(id.to_string())
                    .bind(parent)
                    .bind(next_seq)
                    .bind(ts as i64)
                    .bind(payload)
                    .execute(&mut *tx)
                    .await;
                    if let Err(e) = res {
                        if e.to_string().contains("UNIQUE") {
                            return Err(StorageError::DuplicateEntry(id));
                        }
                        return Err(StorageError::Backend(e.to_string()));
                    }
                }
                Write::InsertUsage(row) => {
                    let payload = serde_json::to_string(&row)
                        .map_err(|e| StorageError::Corrupt(e.to_string()))?;
                    let res = sqlx::query("INSERT INTO usage_store(id, payload) VALUES(?,?)")
                        .bind(row.id.to_string())
                        .bind(payload)
                        .execute(&mut *tx)
                        .await;
                    if let Err(e) = res {
                        if e.to_string().contains("UNIQUE") {
                            return Err(StorageError::DuplicateUsage);
                        }
                        return Err(StorageError::Backend(e.to_string()));
                    }
                }
                Write::SetValue { address, value } => {
                    let payload = serde_json::to_string(&value)
                        .map_err(|e| StorageError::Corrupt(e.to_string()))?;
                    sqlx::query(
                        r#"INSERT INTO values_store(namespace, key, payload) VALUES(?,?,?)
                           ON CONFLICT(namespace, key) DO UPDATE SET payload=excluded.payload"#,
                    )
                    .bind(&address.namespace)
                    .bind(&address.key)
                    .bind(payload)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                }
                Write::DeleteValue { address } => {
                    sqlx::query("DELETE FROM values_store WHERE namespace=? AND key=?")
                        .bind(&address.namespace)
                        .bind(&address.key)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| StorageError::Backend(e.to_string()))?;
                }
                Write::AppendList { address, element } => {
                    let row = sqlx::query(
                        "SELECT COALESCE(MAX(idx), -1) FROM lists_store WHERE namespace=? AND key=?",
                    )
                    .bind(&address.namespace)
                    .bind(&address.key)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                    let idx: i64 = row.get::<i64, _>(0) + 1;
                    let payload = serde_json::to_string(&element)
                        .map_err(|e| StorageError::Corrupt(e.to_string()))?;
                    sqlx::query(
                        "INSERT INTO lists_store(namespace, key, idx, payload) VALUES(?,?,?,?)",
                    )
                    .bind(&address.namespace)
                    .bind(&address.key)
                    .bind(idx)
                    .bind(payload)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                }
                Write::DeleteList { address } => {
                    sqlx::query("DELETE FROM lists_store WHERE namespace=? AND key=?")
                        .bind(&address.namespace)
                        .bind(&address.key)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| StorageError::Backend(e.to_string()))?;
                }
            }
        }

        sqlx::query("UPDATE meta SET value=? WHERE key='next_seq'")
            .bind(next_seq)
            .execute(&mut *tx)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        Ok(CommitResult {
            first_seq,
            seqs,
            timestamp_ms: ts,
        })
    }

    async fn get_entry(&self, id: EntryId) -> StorageResult<Option<Entry>> {
        let row = sqlx::query("SELECT payload FROM entries WHERE id=?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        match row {
            None => Ok(None),
            Some(r) => {
                let payload: String = r.get(0);
                let entry: Entry = serde_json::from_str(&payload)
                    .map_err(|e| StorageError::Corrupt(e.to_string()))?;
                Ok(Some(entry))
            }
        }
    }

    async fn get_value(&self, addr: &ValueAddr) -> StorageResult<Option<JsonValue>> {
        let row = sqlx::query("SELECT payload FROM values_store WHERE namespace=? AND key=?")
            .bind(&addr.namespace)
            .bind(&addr.key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        match row {
            None => Ok(None),
            Some(r) => {
                let payload: String = r.get(0);
                let v: JsonValue = serde_json::from_str(&payload)
                    .map_err(|e| StorageError::Corrupt(e.to_string()))?;
                Ok(Some(v))
            }
        }
    }

    async fn get_list(&self, addr: &ListAddr) -> StorageResult<Vec<JsonValue>> {
        let rows = sqlx::query(
            "SELECT payload FROM lists_store WHERE namespace=? AND key=? ORDER BY idx ASC",
        )
        .bind(&addr.namespace)
        .bind(&addr.key)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Backend(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows {
            let payload: String = r.get(0);
            let v: JsonValue =
                serde_json::from_str(&payload).map_err(|e| StorageError::Corrupt(e.to_string()))?;
            out.push(v);
        }
        Ok(out)
    }

    async fn get_usage(&self, id: UsageId) -> StorageResult<Option<UsageRow>> {
        let row = sqlx::query("SELECT payload FROM usage_store WHERE id=?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        match row {
            None => Ok(None),
            Some(r) => {
                let payload: String = r.get(0);
                let u: UsageRow = serde_json::from_str(&payload)
                    .map_err(|e| StorageError::Corrupt(e.to_string()))?;
                Ok(Some(u))
            }
        }
    }

    async fn scan_entries(&self) -> StorageResult<Vec<Entry>> {
        let rows = sqlx::query("SELECT payload FROM entries ORDER BY seq ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows {
            let payload: String = r.get(0);
            let e: Entry =
                serde_json::from_str(&payload).map_err(|e| StorageError::Corrupt(e.to_string()))?;
            out.push(e);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addresses::ValueAddr;
    use kim_agent_types::{AgentMessage, EntryBase, OperationId};
    use tempfile::tempdir;

    #[tokio::test]
    async fn sqlite_atomic_commit_and_resume_values() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("s.db");
        let path_str = format!("sqlite://{}", path.display());

        let op = OperationId::new();
        let entry_id = EntryId::new();
        {
            let store = SqliteStorage::open(&path_str).await.unwrap();
            let entry = Entry::Message {
                base: EntryBase {
                    id: entry_id,
                    parent_id: None,
                    seq: 0,
                    timestamp_ms: 0,
                },
                message: AgentMessage::User { text: "hi".into() },
            };
            store
                .commit(vec![
                    Write::InsertEntry(entry),
                    Write::SetValue {
                        address: ValueAddr::branch_tip("main"),
                        value: serde_json::json!(entry_id.to_string()),
                    },
                    Write::SetValue {
                        address: ValueAddr::op_state(op),
                        value: serde_json::json!({"phase":"starting"}),
                    },
                ])
                .await
                .unwrap();
        }

        let store = SqliteStorage::open(&path_str).await.unwrap();
        let tip = store
            .get_value(&ValueAddr::branch_tip("main"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(tip.as_str().unwrap(), entry_id.to_string());
        let e = store.get_entry(entry_id).await.unwrap().unwrap();
        assert_eq!(e.id(), entry_id);
        let st = store.get_value(&ValueAddr::op_state(op)).await.unwrap();
        assert!(st.is_some());
    }
}
