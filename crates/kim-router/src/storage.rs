use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_protocol::{AccountId, ChannelId};
use thiserror::Error;

use crate::location::Location;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session not found")]
    NotFound,
    #[error("truncated location")]
    Truncated,
    #[error("invalid utf-8 in location")]
    InvalidUtf8,
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
    async fn delete(&self, account: &AccountId, channel_id: &ChannelId)
        -> Result<(), SessionError>;
    async fn get(&self, channel_id: &ChannelId) -> Result<Session, SessionError>;
    async fn get_locations(&self, accounts: &[AccountId]) -> Result<Vec<Location>, SessionError>;
    async fn get_location(
        &self,
        account: &AccountId,
        device: &str,
    ) -> Result<Location, SessionError>;
    /// Every live location for `account`. Default forwards to [`get_locations`].
    async fn list_locations(&self, account: &AccountId) -> Result<Vec<Location>, SessionError> {
        self.get_locations(std::slice::from_ref(account)).await
    }
}
