use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_protocol::{AccountId, ChannelId, GatewayId};
use kim_router::{Location, SessionError, SessionStorage};

use crate::keys::key_location;

/// In-process session store. One `std::sync::RwLock` covers both maps.
///
/// `add` / `delete` hold the write lock for the whole function (no `.await`
/// under the lock). Reads clone and drop the guard before returning.
/// Location index is a vec per account so web/desktop can overlap.
#[derive(Default)]
pub struct MemorySessionStore {
    inner: RwLock<Inner>,
}

#[derive(Default)]
struct Inner {
    sessions: HashMap<String, Session>,
    locations: HashMap<String, Vec<Location>>,
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

fn loc_of(session: &Session) -> Location {
    Location {
        channel_id: ChannelId::from_trusted(&session.channel_id),
        gate_id: GatewayId::from_trusted(&session.gate_id),
        device: session.device.clone(),
        jti: session.jti.clone(),
    }
}

#[async_trait]
impl SessionStorage for MemorySessionStore {
    async fn add(&self, session: &Session) -> Result<(), SessionError> {
        let loc = loc_of(session);
        let loc_key = key_location(&session.account, "");
        let channel_id = session.channel_id.clone();
        let stored = session.clone();
        let mut inner = self.write();
        let slots = inner.locations.entry(loc_key).or_default();
        slots.retain(|l| l.channel_id != loc.channel_id);
        slots.push(loc);
        inner.sessions.insert(channel_id, stored);
        Ok(())
    }

    async fn delete(
        &self,
        account: &AccountId,
        channel_id: &ChannelId,
    ) -> Result<(), SessionError> {
        let loc_key = key_location(account.as_str(), "");
        let mut inner = self.write();
        inner.sessions.remove(channel_id.as_str());
        if let Some(slots) = inner.locations.get_mut(&loc_key) {
            slots.retain(|l| l.channel_id != *channel_id);
            if slots.is_empty() {
                inner.locations.remove(&loc_key);
            }
        }
        Ok(())
    }

    async fn get(&self, channel_id: &ChannelId) -> Result<Session, SessionError> {
        let session = {
            let inner = self.read();
            inner.sessions.get(channel_id.as_str()).cloned()
        };
        session.ok_or(SessionError::NotFound)
    }

    async fn get_locations(&self, accounts: &[AccountId]) -> Result<Vec<Location>, SessionError> {
        let out = {
            let inner = self.read();
            accounts
                .iter()
                .flat_map(|account| {
                    inner
                        .locations
                        .get(&key_location(account.as_str(), ""))
                        .cloned()
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
        };
        if out.is_empty() {
            Err(SessionError::NotFound)
        } else {
            Ok(out)
        }
    }

    async fn get_location(
        &self,
        account: &AccountId,
        device: &str,
    ) -> Result<Location, SessionError> {
        let loc = {
            let inner = self.read();
            inner
                .locations
                .get(&key_location(account.as_str(), ""))
                .and_then(|slots| {
                    if device.is_empty() {
                        slots.first().cloned()
                    } else {
                        slots.iter().find(|l| l.device == device).cloned()
                    }
                })
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

    fn acc(s: &str) -> AccountId {
        AccountId::from_trusted(s)
    }

    fn ch(s: &str) -> ChannelId {
        ChannelId::from_trusted(s)
    }

    #[tokio::test]
    async fn delete_old_channel_keeps_other_location() {
        let store = MemorySessionStore::new();
        store.add(&session("id1", "alice", "wg-1")).await.unwrap();
        store.add(&session("id2", "alice", "wg-1")).await.unwrap();
        store.delete(&acc("alice"), &ch("id1")).await.unwrap();

        let loc = store.get_location(&acc("alice"), "").await.unwrap();
        assert_eq!(loc.channel_id.as_str(), "id2");
        assert_eq!(loc.gate_id.as_str(), "wg-1");
        assert!(matches!(
            store.get(&ch("id1")).await,
            Err(SessionError::NotFound)
        ));
        let s2 = store.get(&ch("id2")).await.unwrap();
        assert_eq!(s2.channel_id, "id2");
        assert_eq!(s2.account, "alice");
    }

    #[tokio::test]
    async fn two_web_sessions_are_both_listed() {
        let store = MemorySessionStore::new();
        let mut web = session("c1", "alice", "g");
        web.device = "web".into();
        let mut cli = session("c2", "alice", "g");
        cli.device = "cli".into();
        store.add(&web).await.unwrap();
        store.add(&cli).await.unwrap();
        let locs = store.list_locations(&acc("alice")).await.unwrap();
        assert_eq!(locs.len(), 2);
        assert!(locs
            .iter()
            .any(|l| l.channel_id.as_str() == "c1" && l.device == "web"));
        assert!(locs
            .iter()
            .any(|l| l.channel_id.as_str() == "c2" && l.device == "cli"));
    }

    #[tokio::test]
    async fn loc_of_copies_jti() {
        let store = MemorySessionStore::new();
        let mut s = session("c1", "alice", "g");
        s.jti = "jti-alice".into();
        store.add(&s).await.unwrap();
        let locs = store.list_locations(&acc("alice")).await.unwrap();
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].jti, "jti-alice");
    }

    #[tokio::test]
    async fn delete_matching_channel_drops_location() {
        let store = MemorySessionStore::new();
        store.add(&session("id1", "alice", "wg-1")).await.unwrap();
        store.delete(&acc("alice"), &ch("id1")).await.unwrap();
        assert!(matches!(
            store.get_location(&acc("alice"), "").await,
            Err(SessionError::NotFound)
        ));
        assert!(matches!(
            store.get(&ch("id1")).await,
            Err(SessionError::NotFound)
        ));
    }

    #[tokio::test]
    async fn get_locations_skips_missing_all_missing_not_found() {
        let store = MemorySessionStore::new();
        store.add(&session("c1", "a", "g")).await.unwrap();
        let locs = store
            .get_locations(&[acc("a"), acc("missing")])
            .await
            .unwrap();
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].channel_id.as_str(), "c1");

        assert!(matches!(
            store.get_locations(&[acc("x"), acc("y")]).await,
            Err(SessionError::NotFound)
        ));
        assert!(matches!(
            store.get_locations(&[]).await,
            Err(SessionError::NotFound)
        ));
    }

    #[tokio::test]
    async fn get_location_filters_by_device() {
        let store = MemorySessionStore::new();
        let mut s = session("c1", "alice", "g");
        s.device = "phone".into();
        store.add(&s).await.unwrap();

        let loc = store.get_location(&acc("alice"), "").await.unwrap();
        assert_eq!(loc.channel_id.as_str(), "c1");
        assert_eq!(
            store
                .get_location(&acc("alice"), "phone")
                .await
                .unwrap()
                .channel_id
                .as_str(),
            "c1"
        );
        assert!(matches!(
            store.get_location(&acc("alice"), "web").await,
            Err(SessionError::NotFound)
        ));
        assert_eq!(store.get(&ch("c1")).await.unwrap().device, "phone");
    }
}
