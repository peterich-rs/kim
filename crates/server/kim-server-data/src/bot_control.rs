//! Royal identity control, separate from the public IM directory.

use async_trait::async_trait;
use kim_protocol::pkt::Status;
use serde::{Deserialize, Serialize};

use crate::store::{MessageStore, StoreError, StoreScope};
use crate::users::{BotPatch, BotRecord, CreateBot, UserError};

#[derive(Debug, thiserror::Error)]
pub enum BotControlError {
    #[error("bot control unavailable")]
    Unavailable,
    #[error("not bot owner")]
    NotOwner,
    #[error("bot not found")]
    NotFound,
    #[error("invalid bot request")]
    Invalid,
    #[error("{0}")]
    Backend(String),
}

impl From<UserError> for BotControlError {
    fn from(err: UserError) -> Self {
        match err {
            UserError::NotFound => Self::NotFound,
            UserError::NotBotOwner => Self::NotOwner,
            UserError::InvalidProfile | UserError::Limit | UserError::Conflict => Self::Invalid,
            UserError::Backend(msg) => Self::Backend(msg),
        }
    }
}

impl BotControlError {
    pub fn status(&self) -> Status {
        match self {
            Self::Unavailable => Status::ServiceUnavailable,
            Self::NotOwner => Status::NotBotOwner,
            Self::NotFound => Status::UserNotFound,
            Self::Invalid => Status::InvalidPacketBody,
            Self::Backend(_) => Status::SystemException,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeletionLease {
    pub deletion_id: String,
    pub original_owner: String,
    pub cleanup_scope: String,
    pub done: bool,
}

#[derive(Serialize, Deserialize)]
pub struct DeleteIdentity {
    pub app: String,
    pub actor: String,
    pub bot: String,
    pub scope: StoreScope,
}

#[derive(Serialize, Deserialize)]
pub struct CompleteDelete {
    pub app: String,
    pub deletion_id: String,
    pub actor: String,
    pub scope: StoreScope,
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
        store_scope: &StoreScope,
    ) -> Result<DeletionLease, BotControlError>;
    async fn complete_delete(
        &self,
        app: &str,
        deletion_id: &str,
        actor: &str,
        store_scope: &StoreScope,
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
        _: &StoreScope,
    ) -> Result<DeletionLease, BotControlError> {
        Err(BotControlError::Unavailable)
    }
    async fn complete_delete(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &StoreScope,
    ) -> Result<(), BotControlError> {
        Err(BotControlError::Unavailable)
    }
}

pub fn purge_status(err: &StoreError) -> Status {
    match err {
        StoreError::Invalid(_) | StoreError::Http { status: 400, .. } => Status::InvalidPacketBody,
        StoreError::NotFound | StoreError::Http { status: 404, .. } => Status::UserNotFound,
        StoreError::Http { status: 403, .. } => Status::NotBotOwner,
        _ => Status::SystemException,
    }
}

pub async fn coordinate_delete(
    control: &dyn BotControl,
    store: &dyn MessageStore,
    app: &str,
    actor: &str,
    bot: &str,
) -> Result<(), Status> {
    let scope = store
        .resolve_scope()
        .await
        .map_err(|_| Status::ServiceUnavailable)?;
    if !scope.is_scoped() {
        return Err(Status::ServiceUnavailable);
    }
    let lease = control
        .delete_identity_or_resume(app, actor, bot, &scope)
        .await
        .map_err(|e| e.status())?;
    // Check even Done receipts: a different Memory instance must never inherit success.
    if lease.original_owner != actor {
        return Err(Status::NotBotOwner);
    }
    if lease.cleanup_scope != scope.key() {
        return Err(Status::SystemException);
    }
    if lease.done {
        return Ok(());
    }
    store
        .purge_peer_dm_scoped(&scope, app, actor, bot)
        .await
        .map_err(|e| purge_status(&e))?;
    control
        .complete_delete(app, &lease.deletion_id, actor, &scope)
        .await
        .map_err(|error| {
            tracing::warn!(?error, deletion_id = %lease.deletion_id, "bot cleanup completion failed");
            Status::SystemException
        })
}
