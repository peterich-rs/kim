use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_router::{Location, SessionError, SessionStorage};

/// Write-through location/session cache. Miss fill uses N `get_location` calls
/// because `Location` has no account field.
pub struct CachedSessionStore {
    inner: Arc<dyn SessionStorage>,
    sessions: Mutex<HashMap<String, Session>>,
    locs: Mutex<HashMap<String, Location>>,
}

impl CachedSessionStore {
    pub fn wrap(inner: Arc<dyn SessionStorage>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            sessions: Mutex::new(HashMap::new()),
            locs: Mutex::new(HashMap::new()),
        })
    }

    fn sessions(&self) -> MutexGuard<'_, HashMap<String, Session>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn locs(&self) -> MutexGuard<'_, HashMap<String, Location>> {
        self.locs.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[async_trait]
impl SessionStorage for CachedSessionStore {
    async fn add(&self, session: &Session) -> Result<(), SessionError> {
        self.inner.add(session).await?;
        let loc = Location {
            channel_id: session.channel_id.clone(),
            gate_id: session.gate_id.clone(),
        };
        self.locs().insert(session.account.clone(), loc);
        self.sessions()
            .insert(session.channel_id.clone(), session.clone());
        Ok(())
    }

    async fn delete(&self, account: &str, channel_id: &str) -> Result<(), SessionError> {
        self.inner.delete(account, channel_id).await?;
        self.sessions().remove(channel_id);
        let mut locs = self.locs();
        if locs
            .get(account)
            .is_some_and(|l| l.channel_id == channel_id)
        {
            locs.remove(account);
        }
        Ok(())
    }

    async fn get(&self, channel_id: &str) -> Result<Session, SessionError> {
        if let Some(s) = self.sessions().get(channel_id).cloned() {
            return Ok(s);
        }
        let s = self.inner.get(channel_id).await?;
        self.sessions().insert(channel_id.to_string(), s.clone());
        Ok(s)
    }

    async fn get_locations(&self, accounts: &[String]) -> Result<Vec<Location>, SessionError> {
        let mut hits = Vec::new();
        let mut misses = Vec::new();
        {
            let locs = self.locs();
            for acc in accounts {
                match locs.get(acc) {
                    Some(l) => hits.push(l.clone()),
                    None => misses.push(acc.clone()),
                }
            }
        }
        for acc in misses {
            match self.get_location(&acc, "").await {
                Ok(l) => hits.push(l),
                Err(SessionError::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        if hits.is_empty() {
            Err(SessionError::NotFound)
        } else {
            Ok(hits)
        }
    }

    async fn get_location(&self, account: &str, device: &str) -> Result<Location, SessionError> {
        if let Some(l) = self.locs().get(account).cloned() {
            return Ok(l);
        }
        let loc = self.inner.get_location(account, device).await?;
        self.locs().insert(account.to_string(), loc.clone());
        Ok(loc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemorySessionStore;

    fn session(channel_id: &str, account: &str, gate_id: &str) -> Session {
        Session {
            channel_id: channel_id.into(),
            gate_id: gate_id.into(),
            account: account.into(),
            ..Session::default()
        }
    }

    #[tokio::test]
    async fn miss_fill_and_partial_hits() {
        let inner = Arc::new(MemorySessionStore::new());
        inner.add(&session("c1", "alice", "g")).await.unwrap();
        inner.add(&session("c2", "bob", "g")).await.unwrap();
        let cache = CachedSessionStore::wrap(inner);
        let locs = cache
            .get_locations(&["alice".into(), "bob".into(), "carol".into()])
            .await
            .unwrap();
        assert_eq!(locs.len(), 2);
        let again = cache.get_location("alice", "").await.unwrap();
        assert_eq!(again.channel_id, "c1");
    }

    #[tokio::test]
    async fn other_fails_whole_call() {
        struct Boom;
        #[async_trait]
        impl SessionStorage for Boom {
            async fn add(&self, _: &Session) -> Result<(), SessionError> {
                Ok(())
            }
            async fn delete(&self, _: &str, _: &str) -> Result<(), SessionError> {
                Ok(())
            }
            async fn get(&self, _: &str) -> Result<Session, SessionError> {
                Err(SessionError::Other("boom".into()))
            }
            async fn get_locations(&self, _: &[String]) -> Result<Vec<Location>, SessionError> {
                Err(SessionError::Other("boom".into()))
            }
            async fn get_location(&self, _: &str, _: &str) -> Result<Location, SessionError> {
                Err(SessionError::Other("boom".into()))
            }
        }
        let cache = CachedSessionStore::wrap(Arc::new(Boom));
        let err = cache.get_locations(&["x".into()]).await.unwrap_err();
        assert!(matches!(err, SessionError::Other(_)));
    }
}
