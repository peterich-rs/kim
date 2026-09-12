use crate::events::{InboxItem, IncomingTalk};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnreadPolicy {
    Keep,
    IfInserted,
}

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("busy")]
    Busy,
    #[error("storage full")]
    StorageFull,
    #[error("disk error")]
    Disk { message: String },
    #[error("stale epoch")]
    StaleEpoch,
}

#[async_trait::async_trait]
pub trait PersistHook: Send + Sync {
    async fn persist_talks(
        &self,
        talks: &[IncomingTalk],
        policy: UnreadPolicy,
    ) -> Result<(), PersistError>;

    async fn persist_inbox(&self, items: &[InboxItem]) -> Result<(), PersistError>;
}
