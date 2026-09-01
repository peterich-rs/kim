use async_trait::async_trait;
use kim_protocol::pkt::Session;
use thiserror::Error;

use crate::location::Location;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session not found")]
    NotFound,
    #[error("{0}")]
    Other(String),
}

/// Session store used by Chat receive and login handlers.
///
/// `delete` must be atomic: always remove `login:sn:v2:{channel_id}`; remove loc
/// only when it still points at that `channel_id` (one write lock / Redis Lua,
/// not GET-then-DEL). `get_location` / `get_locations` with no hits → `NotFound`.
/// Callers pass `device = ""` in this milestone.
#[async_trait]
pub trait SessionStorage: Send + Sync {
    async fn add(&self, session: &Session) -> Result<(), SessionError>;
    async fn delete(&self, account: &str, channel_id: &str) -> Result<(), SessionError>;
    async fn get(&self, channel_id: &str) -> Result<Session, SessionError>;
    async fn get_locations(&self, accounts: &[String]) -> Result<Vec<Location>, SessionError>;
    async fn get_location(&self, account: &str, device: &str) -> Result<Location, SessionError>;
    /// Every live location for `account`. Default forwards to [`get_locations`].
    async fn list_locations(&self, account: &str) -> Result<Vec<Location>, SessionError> {
        self.get_locations(&[account.to_string()]).await
    }
}
