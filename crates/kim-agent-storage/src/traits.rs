use crate::addresses::{ListAddr, ValueAddr};
use async_trait::async_trait;
use kim_agent_types::{Entry, EntryId, JsonValue, UsageRow};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Write {
    InsertEntry(Entry),
    InsertUsage(UsageRow),
    SetValue {
        address: ValueAddr,
        value: JsonValue,
    },
    DeleteValue {
        address: ValueAddr,
    },
    AppendList {
        address: ListAddr,
        element: JsonValue,
    },
    DeleteList {
        address: ListAddr,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitResult {
    pub first_seq: u64,
    pub seqs: Vec<u64>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("duplicate entry id {0}")]
    DuplicateEntry(EntryId),
    #[error("duplicate usage id")]
    DuplicateUsage,
    #[error("not found")]
    NotFound,
    #[error("corrupt: {0}")]
    Corrupt(String),
    #[error("backend: {0}")]
    Backend(String),
}

pub type StorageResult<T> = Result<T, StorageError>;

#[async_trait]
pub trait Storage: Send + Sync {
    async fn commit(&self, writes: Vec<Write>) -> StorageResult<CommitResult>;
    async fn get_entry(&self, id: EntryId) -> StorageResult<Option<Entry>>;
    async fn get_value(&self, addr: &ValueAddr) -> StorageResult<Option<JsonValue>>;
    async fn get_list(&self, addr: &ListAddr) -> StorageResult<Vec<JsonValue>>;
    async fn get_usage(&self, id: kim_agent_types::UsageId) -> StorageResult<Option<UsageRow>>;
    async fn scan_entries(&self) -> StorageResult<Vec<Entry>>;
}

pub fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
