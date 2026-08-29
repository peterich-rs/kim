use std::sync::Arc;

use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_router::{Location, SessionError, SessionStorage};

/// Write both stores; read the primary. Always compiled (Memory+Memory in tests).
pub struct DualWriteStore {
    primary: Arc<dyn SessionStorage>,
    mirror: Arc<dyn SessionStorage>,
}

impl DualWriteStore {
    pub fn new(primary: Arc<dyn SessionStorage>, mirror: Arc<dyn SessionStorage>) -> Arc<Self> {
        Arc::new(Self { primary, mirror })
    }
}

#[async_trait]
impl SessionStorage for DualWriteStore {
    async fn add(&self, session: &Session) -> Result<(), SessionError> {
        self.primary.add(session).await?;
        if let Err(err) = self.mirror.add(session).await {
            tracing::warn!(%err, "mirror add failed");
        }
        Ok(())
    }

    async fn delete(&self, account: &str, channel_id: &str) -> Result<(), SessionError> {
        self.primary.delete(account, channel_id).await?;
        if let Err(err) = self.mirror.delete(account, channel_id).await {
            tracing::warn!(%err, "mirror delete failed");
        }
        Ok(())
    }

    async fn get(&self, channel_id: &str) -> Result<Session, SessionError> {
        self.primary.get(channel_id).await
    }

    async fn get_locations(&self, accounts: &[String]) -> Result<Vec<Location>, SessionError> {
        self.primary.get_locations(accounts).await
    }

    async fn get_location(&self, account: &str, device: &str) -> Result<Location, SessionError> {
        self.primary.get_location(account, device).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemorySessionStore;

    fn session(channel_id: &str, account: &str) -> Session {
        Session {
            channel_id: channel_id.into(),
            gate_id: "g".into(),
            account: account.into(),
            ..Session::default()
        }
    }

    #[tokio::test]
    async fn memory_plus_memory_roundtrip() {
        let a = Arc::new(MemorySessionStore::new());
        let b = Arc::new(MemorySessionStore::new());
        let dual = DualWriteStore::new(a.clone(), b.clone());
        dual.add(&session("c1", "alice")).await.unwrap();
        assert_eq!(a.get("c1").await.unwrap().account, "alice");
        assert_eq!(b.get("c1").await.unwrap().account, "alice");
        dual.delete("alice", "c1").await.unwrap();
        assert!(matches!(a.get("c1").await, Err(SessionError::NotFound)));
        assert!(matches!(b.get("c1").await, Err(SessionError::NotFound)));
    }
}
