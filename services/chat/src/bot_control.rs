//! Bot identity writes go through Royal. Chat only coordinates cleanup on the store it actually holds.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use kim_protocol::pkt::Status;

use kim_server_data::store::{MessageStore, StoreError};
use kim_server_data::users::{BotPatch, BotRecord, CreateBot, UserDirectory, UserError};

#[derive(Debug, thiserror::Error)]
pub enum BotControlError {
    #[error("bot control unavailable")]
    Unavailable,
    #[error("not bot owner")]
    NotOwner,
    #[error("bot not found")]
    NotFound,
    #[error("{0}")]
    Backend(String),
}

impl From<UserError> for BotControlError {
    fn from(err: UserError) -> Self {
        match err {
            UserError::NotFound => Self::NotFound,
            UserError::NotBotOwner => Self::NotOwner,
            other => Self::Backend(other.to_string()),
        }
    }
}

impl BotControlError {
    pub fn status(&self) -> Status {
        match self {
            Self::Unavailable => Status::ServiceUnavailable,
            Self::NotOwner => Status::NotBotOwner,
            Self::NotFound => Status::UserNotFound,
            Self::Backend(_) => Status::SystemException,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeletionLease {
    pub deletion_id: String,
    pub original_owner: String,
    pub cleanup_scope: String,
    pub done: bool,
}

#[async_trait]
pub trait BotControl: Send + Sync {
    async fn create_bot(&self, app: &str, req: &CreateBot) -> Result<BotRecord, BotControlError>;
    async fn update_bot(
        &self,
        app: &str,
        owner: &str,
        account: &str,
        patch: &BotPatch,
    ) -> Result<BotRecord, BotControlError>;
    async fn delete_identity_or_resume(
        &self,
        app: &str,
        actor: &str,
        bot: &str,
        store_scope: &str,
    ) -> Result<DeletionLease, BotControlError>;
    async fn complete_delete(
        &self,
        deletion_id: &str,
        actor: &str,
        store_scope: &str,
    ) -> Result<(), BotControlError>;
}

pub struct UnavailableBotControl;

#[async_trait]
impl BotControl for UnavailableBotControl {
    async fn create_bot(&self, _: &str, _: &CreateBot) -> Result<BotRecord, BotControlError> {
        Err(BotControlError::Unavailable)
    }
    async fn update_bot(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &BotPatch,
    ) -> Result<BotRecord, BotControlError> {
        Err(BotControlError::Unavailable)
    }
    async fn delete_identity_or_resume(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<DeletionLease, BotControlError> {
        Err(BotControlError::Unavailable)
    }
    async fn complete_delete(&self, _: &str, _: &str, _: &str) -> Result<(), BotControlError> {
        Err(BotControlError::Unavailable)
    }
}

#[derive(Clone)]
struct Receipt {
    deletion_id: String,
    original_owner: String,
    cleanup_scope: String,
    done: bool,
}

/// Test/local adapter over one directory. Production Chat uses Royal HTTP, not this writer.
pub struct DirectoryBotControl {
    users: std::sync::Arc<dyn UserDirectory>,
    receipts: Mutex<HashMap<(String, String), Receipt>>,
    next_id: std::sync::atomic::AtomicU64,
}

impl DirectoryBotControl {
    pub fn new(users: std::sync::Arc<dyn UserDirectory>) -> Self {
        Self {
            users,
            receipts: Mutex::new(HashMap::new()),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<(String, String), Receipt>> {
        self.receipts.lock().unwrap_or_else(|err| err.into_inner())
    }
}

#[async_trait]
impl BotControl for DirectoryBotControl {
    async fn create_bot(&self, app: &str, req: &CreateBot) -> Result<BotRecord, BotControlError> {
        self.users.create_bot(app, req).await.map_err(Into::into)
    }

    async fn update_bot(
        &self,
        app: &str,
        owner: &str,
        account: &str,
        patch: &BotPatch,
    ) -> Result<BotRecord, BotControlError> {
        self.users
            .update_bot(app, owner, account, patch)
            .await
            .map_err(Into::into)
    }

    async fn delete_identity_or_resume(
        &self,
        app: &str,
        actor: &str,
        bot: &str,
        store_scope: &str,
    ) -> Result<DeletionLease, BotControlError> {
        let key = (app.to_string(), bot.to_string());
        if let Some(existing) = self.lock().get(&key).cloned() {
            if existing.original_owner != actor {
                return Err(BotControlError::NotOwner);
            }
            return Ok(DeletionLease {
                deletion_id: existing.deletion_id,
                original_owner: existing.original_owner,
                cleanup_scope: existing.cleanup_scope,
                done: existing.done,
            });
        }
        let presence = self
            .users
            .lookup(app, bot)
            .await
            .map_err(BotControlError::from)?;
        let Some(presence) = presence.filter(|row| row.exists) else {
            return Err(BotControlError::NotFound);
        };
        if presence.owner_account != actor {
            return Err(BotControlError::NotOwner);
        }
        let deletion_id = format!(
            "del-{}",
            self.next_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let receipt = Receipt {
            deletion_id: deletion_id.clone(),
            original_owner: actor.to_string(),
            cleanup_scope: store_scope.to_string(),
            done: false,
        };
        self.lock().insert(key, receipt);
        self.users
            .delete_bot(app, actor, bot)
            .await
            .map_err(BotControlError::from)?;
        Ok(DeletionLease {
            deletion_id,
            original_owner: actor.to_string(),
            cleanup_scope: store_scope.to_string(),
            done: false,
        })
    }

    async fn complete_delete(
        &self,
        deletion_id: &str,
        actor: &str,
        store_scope: &str,
    ) -> Result<(), BotControlError> {
        let mut guard = self.lock();
        let Some(receipt) = guard
            .values_mut()
            .find(|row| row.deletion_id == deletion_id)
        else {
            return Err(BotControlError::NotFound);
        };
        if receipt.original_owner != actor || receipt.cleanup_scope != store_scope {
            return Err(BotControlError::NotOwner);
        }
        receipt.done = true;
        Ok(())
    }
}

pub async fn coordinate_delete(
    control: &dyn BotControl,
    store: &dyn MessageStore,
    app: &str,
    actor: &str,
    bot: &str,
) -> Result<(), Status> {
    let scope = store.store_scope();
    let lease = control
        .delete_identity_or_resume(app, actor, bot, &scope.id)
        .await
        .map_err(|err| err.status())?;
    if lease.done {
        return Ok(());
    }
    if let Err(err) = store.purge_peer_dm_scoped(&scope, app, actor, bot).await {
        let _ = err;
        return Err(Status::SystemException);
    }
    control
        .complete_delete(&lease.deletion_id, actor, &scope.id)
        .await
        .map_err(|err| err.status())?;
    Ok(())
}

pub fn purge_status(err: &StoreError) -> Status {
    match err {
        StoreError::Invalid(_) => Status::InvalidPacketBody,
        StoreError::NotFound => Status::UserNotFound,
        StoreError::Http { status: 400, .. } => Status::InvalidPacketBody,
        StoreError::Http { status: 403, .. } => Status::NotBotOwner,
        StoreError::Http { status: 404, .. } => Status::UserNotFound,
        _ => Status::SystemException,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_server_data::idgen::SequenceIdGen;
    use kim_server_data::store::{MemoryMessageStore, StoreScope};
    use kim_server_data::users::MemoryUserDirectory;
    use std::sync::Arc;

    #[tokio::test]
    async fn purge_failure_keeps_pending_and_retry_skips_missing_row() {
        let users = Arc::new(
            MemoryUserDirectory::new()
                .with_social(Arc::new(
                    kim_server_data::social::MemorySocialDirectory::new(),
                ))
                .with_idgen(Arc::new(SequenceIdGen::new(1))),
        );
        users.upsert("kim", "alice").await.unwrap();
        let created = users
            .create_bot(
                "kim",
                &CreateBot {
                    owner: "alice".into(),
                    client_profile_id: "goose".into(),
                    nickname: "助手".into(),
                    avatar: String::new(),
                    bio: String::new(),
                    model: String::new(),
                    thinking_effort: String::new(),
                    context_tokens: None,
                    visibility: String::new(),
                },
            )
            .await
            .unwrap();
        let control = DirectoryBotControl::new(users);
        let store = MemoryMessageStore::new(Arc::new(SequenceIdGen::new(2)));
        let wrong = StoreScope::memory("other-instance");
        let err = store
            .purge_peer_dm_scoped(&wrong, "kim", "alice", &created.profile.account)
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::Backend(_)));

        let lease = control
            .delete_identity_or_resume("kim", "alice", &created.profile.account, store.scope_id())
            .await
            .unwrap();
        assert!(!lease.done);
        let missing = control
            .delete_identity_or_resume("kim", "bob", &created.profile.account, store.scope_id())
            .await
            .unwrap_err();
        assert!(matches!(missing, BotControlError::NotOwner));
        let again = control
            .delete_identity_or_resume("kim", "alice", &created.profile.account, store.scope_id())
            .await
            .unwrap();
        assert_eq!(again.deletion_id, lease.deletion_id);
        store
            .purge_peer_dm_scoped(
                &store.store_scope(),
                "kim",
                "alice",
                &created.profile.account,
            )
            .await
            .unwrap();
        control
            .complete_delete(&lease.deletion_id, "alice", store.scope_id())
            .await
            .unwrap();
        let done = control
            .delete_identity_or_resume("kim", "alice", &created.profile.account, store.scope_id())
            .await
            .unwrap();
        assert!(done.done);
    }
}
