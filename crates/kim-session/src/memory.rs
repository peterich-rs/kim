use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_router::{Location, SessionError, SessionStorage};

use crate::keys::key_location;

/// In-process session store. One `std::sync::RwLock` covers both maps.
///
/// `add` / `delete` hold the write lock for the whole function (no `.await`
/// under the lock). Reads clone and drop the guard before returning.
#[derive(Default)]
pub struct MemorySessionStore {
    inner: RwLock<Inner>,
}

#[derive(Default)]
struct Inner {
    sessions: HashMap<String, Session>,
    locations: HashMap<String, Location>,
}

impl MemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }
}

#[async_trait]
impl SessionStorage for MemorySessionStore {
    async fn add(&self, session: &Session) -> Result<(), SessionError> {
        let mut inner = self.write();
        let loc = Location {
            channel_id: session.channel_id.clone(),
            gate_id: session.gate_id.clone(),
        };
        inner
            .locations
            .insert(key_location(&session.account, &session.device), loc);
        inner
            .sessions
            .insert(session.channel_id.clone(), session.clone());
        Ok(())
    }

    async fn delete(&self, account: &str, channel_id: &str) -> Result<(), SessionError> {
        let mut inner = self.write();
        inner.sessions.remove(channel_id);
        let loc_key = key_location(account, "");
        match inner.locations.get(&loc_key) {
            Some(loc) if loc.channel_id == channel_id => {
                inner.locations.remove(&loc_key);
            }
            Some(_) => {
                tracing::debug!("keep location, newer channel");
            }
            None => {}
        }
        Ok(())
    }

    async fn get(&self, channel_id: &str) -> Result<Session, SessionError> {
        let session = {
            let inner = self.read();
            inner.sessions.get(channel_id).cloned()
        };
        session.ok_or(SessionError::NotFound)
    }

    async fn get_locations(&self, accounts: &[String]) -> Result<Vec<Location>, SessionError> {
        let out = {
            let inner = self.read();
            accounts
                .iter()
                .filter_map(|account| inner.locations.get(&key_location(account, "")).cloned())
                .collect::<Vec<_>>()
        };
        if out.is_empty() {
            Err(SessionError::NotFound)
        } else {
            Ok(out)
        }
    }

    async fn get_location(&self, account: &str, device: &str) -> Result<Location, SessionError> {
        let loc = {
            let inner = self.read();
            inner.locations.get(&key_location(account, device)).cloned()
        };
        loc.ok_or(SessionError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(channel_id: &str, account: &str, gate_id: &str) -> Session {
        Session {
            channel_id: channel_id.into(),
            gate_id: gate_id.into(),
            account: account.into(),
            ..Session::default()
        }
    }

    #[tokio::test]
    async fn delete_old_channel_keeps_newer_location() {
        let store = MemorySessionStore::new();
        store.add(&session("id1", "alice", "wg-1")).await.unwrap();
        store.add(&session("id2", "alice", "wg-1")).await.unwrap();
        store.delete("alice", "id1").await.unwrap();

        let loc = store.get_location("alice", "").await.unwrap();
        assert_eq!(loc.channel_id, "id2");
        assert_eq!(loc.gate_id, "wg-1");
        assert!(matches!(
            store.get("id1").await,
            Err(SessionError::NotFound)
        ));
        let s2 = store.get("id2").await.unwrap();
        assert_eq!(s2.channel_id, "id2");
        assert_eq!(s2.account, "alice");
    }

    #[tokio::test]
    async fn delete_matching_channel_drops_location() {
        let store = MemorySessionStore::new();
        store.add(&session("id1", "alice", "wg-1")).await.unwrap();
        store.delete("alice", "id1").await.unwrap();
        assert!(matches!(
            store.get_location("alice", "").await,
            Err(SessionError::NotFound)
        ));
        assert!(matches!(
            store.get("id1").await,
            Err(SessionError::NotFound)
        ));
    }

    #[tokio::test]
    async fn get_locations_skips_missing_all_missing_not_found() {
        let store = MemorySessionStore::new();
        store.add(&session("c1", "a", "g")).await.unwrap();
        let locs = store
            .get_locations(&["a".into(), "missing".into()])
            .await
            .unwrap();
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].channel_id, "c1");

        assert!(matches!(
            store.get_locations(&["x".into(), "y".into()]).await,
            Err(SessionError::NotFound)
        ));
        assert!(matches!(
            store.get_locations(&[]).await,
            Err(SessionError::NotFound)
        ));
    }

    #[tokio::test]
    async fn location_key_uses_device_when_set() {
        let store = MemorySessionStore::new();
        let mut s = session("c1", "alice", "g");
        s.device = "phone".into();
        store.add(&s).await.unwrap();
        assert!(matches!(
            store.get_location("alice", "").await,
            Err(SessionError::NotFound)
        ));
        let loc = store.get_location("alice", "phone").await.unwrap();
        assert_eq!(loc.channel_id, "c1");
    }
}
