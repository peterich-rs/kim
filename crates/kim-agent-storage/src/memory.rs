use crate::addresses::{ListAddr, ValueAddr};
use crate::traits::{now_ms, CommitResult, Storage, StorageError, StorageResult, Write};
use async_trait::async_trait;
use kim_agent_types::{Entry, EntryId, JsonValue, UsageId, UsageRow};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct Inner {
    next_seq: u64,
    entries: HashMap<EntryId, Entry>,
    values: HashMap<(String, String), JsonValue>,
    lists: HashMap<(String, String), Vec<JsonValue>>,
    usage: HashMap<UsageId, UsageRow>,
}

pub struct MemoryStorage {
    inner: Mutex<Inner>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Storage for MemoryStorage {
    async fn commit(&self, writes: Vec<Write>) -> StorageResult<CommitResult> {
        let mut g = self
            .inner
            .lock()
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        let ts = now_ms();
        let first_seq = g.next_seq + 1;
        let mut seqs = Vec::with_capacity(writes.len());
        // Apply on a scratch then swap — all-or-none
        let mut entries = g.entries.clone();
        let mut values = g.values.clone();
        let mut lists = g.lists.clone();
        let mut usage = g.usage.clone();
        let mut next_seq = g.next_seq;

        for w in writes {
            next_seq += 1;
            seqs.push(next_seq);
            match w {
                Write::InsertEntry(mut entry) => {
                    let id = entry.id();
                    if entries.contains_key(&id) {
                        return Err(StorageError::DuplicateEntry(id));
                    }
                    entry.base_mut().seq = next_seq;
                    entry.base_mut().timestamp_ms = ts;
                    entries.insert(id, entry);
                }
                Write::InsertUsage(row) => {
                    if usage.contains_key(&row.id) {
                        return Err(StorageError::DuplicateUsage);
                    }
                    usage.insert(row.id, row);
                }
                Write::SetValue { address, value } => {
                    values.insert((address.namespace, address.key), value);
                }
                Write::DeleteValue { address } => {
                    values.remove(&(address.namespace, address.key));
                }
                Write::AppendList { address, element } => {
                    lists
                        .entry((address.namespace, address.key))
                        .or_default()
                        .push(element);
                }
                Write::DeleteList { address } => {
                    lists.remove(&(address.namespace, address.key));
                }
            }
        }

        g.entries = entries;
        g.values = values;
        g.lists = lists;
        g.usage = usage;
        g.next_seq = next_seq;

        Ok(CommitResult {
            first_seq,
            seqs,
            timestamp_ms: ts,
        })
    }

    async fn get_entry(&self, id: EntryId) -> StorageResult<Option<Entry>> {
        let g = self
            .inner
            .lock()
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(g.entries.get(&id).cloned())
    }

    async fn get_value(&self, addr: &ValueAddr) -> StorageResult<Option<JsonValue>> {
        let g = self
            .inner
            .lock()
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(g.values
            .get(&(addr.namespace.clone(), addr.key.clone()))
            .cloned())
    }

    async fn get_list(&self, addr: &ListAddr) -> StorageResult<Vec<JsonValue>> {
        let g = self
            .inner
            .lock()
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(g.lists
            .get(&(addr.namespace.clone(), addr.key.clone()))
            .cloned()
            .unwrap_or_default())
    }

    async fn get_usage(&self, id: UsageId) -> StorageResult<Option<UsageRow>> {
        let g = self
            .inner
            .lock()
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(g.usage.get(&id).cloned())
    }

    async fn scan_entries(&self) -> StorageResult<Vec<Entry>> {
        let g = self
            .inner
            .lock()
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        let mut v: Vec<_> = g.entries.values().cloned().collect();
        v.sort_by_key(|e| e.base().seq);
        Ok(v)
    }
}
