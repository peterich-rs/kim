use crate::{from_pb_config, from_pb_profile, RoyalPool};
use async_trait::async_trait;
use kim_protocol::pkt::{BotCreateResp, InternalBotCreate, InternalBotUpdate};
use kim_server_data::bot_control::{
    BotControl, BotControlError, CompleteDelete, DeleteIdentity, DeletionLease,
};
use kim_server_data::store::{StoreError, StoreScope};
use kim_server_data::users::{BotPatch, BotRecord, CreateBot};
use std::sync::Arc;

pub struct HttpBotControl {
    pool: Arc<RoyalPool>,
}

impl HttpBotControl {
    pub fn from_pool(pool: Arc<RoyalPool>) -> Self {
        Self { pool }
    }
}

fn control_error(error: StoreError) -> BotControlError {
    match error {
        StoreError::Http { status: 400, .. } => BotControlError::Invalid,
        StoreError::Http { status: 403, .. } => BotControlError::NotOwner,
        StoreError::Http { status: 404, msg } if msg == "bot not found" => {
            BotControlError::NotFound
        }
        StoreError::Http {
            status: 404 | 405 | 501 | 503,
            ..
        } => BotControlError::Unavailable,
        other => BotControlError::Backend(other.to_string()),
    }
}

#[async_trait]
impl BotControl for HttpBotControl {
    async fn create_bot(&self, app: &str, req: &CreateBot) -> Result<BotRecord, BotControlError> {
        self.create_identity(app, req).await
    }
    async fn update_bot(
        &self,
        app: &str,
        owner: &str,
        account: &str,
        patch: &BotPatch,
    ) -> Result<BotRecord, BotControlError> {
        self.update_identity(app, owner, account, patch).await
    }
    async fn delete_identity_or_resume(
        &self,
        app: &str,
        actor: &str,
        bot: &str,
        store_scope: &StoreScope,
    ) -> Result<DeletionLease, BotControlError> {
        self.pool
            .send_json(
                reqwest::Method::POST,
                "/internal/v1/bot/delete/identity",
                Some(&DeleteIdentity {
                    app: app.into(),
                    actor: actor.into(),
                    bot: bot.into(),
                    scope: store_scope.clone(),
                }),
            )
            .await
            .map_err(control_error)
    }
    async fn complete_delete(
        &self,
        app: &str,
        deletion_id: &str,
        actor: &str,
        store_scope: &StoreScope,
    ) -> Result<(), BotControlError> {
        let completed: bool = self
            .pool
            .send_json(
                reqwest::Method::POST,
                "/internal/v1/bot/delete/complete",
                Some(&CompleteDelete {
                    app: app.into(),
                    deletion_id: deletion_id.into(),
                    actor: actor.into(),
                    scope: store_scope.clone(),
                }),
            )
            .await
            .map_err(control_error)?;
        if !completed {
            return Err(BotControlError::Backend(
                "Royal did not confirm deletion completion".into(),
            ));
        }
        Ok(())
    }
}

impl HttpBotControl {
    async fn create_identity(
        &self,
        _app: &str,
        req: &CreateBot,
    ) -> Result<BotRecord, BotControlError> {
        let body = InternalBotCreate {
            owner: req.owner.clone(),
            client_profile_id: req.client_profile_id.clone(),
            nickname: req.nickname.clone(),
            avatar: req.avatar.clone(),
            bio: req.bio.clone(),
            model: req.model.clone(),
            thinking_effort: req.thinking_effort.clone(),
            context_tokens: req.context_tokens,
            visibility: req.visibility.clone(),
        };
        let resp: BotCreateResp = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/bot", Some(&body))
            .await
            .map_err(control_error)?;
        match resp.profile {
            Some(p) => Ok(BotRecord {
                profile: from_pb_profile(p),
                config: from_pb_config(resp.config),
            }),
            None => Err(BotControlError::Backend(
                "bot create missing profile".into(),
            )),
        }
    }
    async fn update_identity(
        &self,
        _app: &str,
        owner: &str,
        account: &str,
        patch: &BotPatch,
    ) -> Result<BotRecord, BotControlError> {
        let body = InternalBotUpdate {
            owner: owner.to_string(),
            account: account.to_string(),
            nickname: patch.nickname.clone(),
            avatar: patch.avatar.clone(),
            bio: patch.bio.clone(),
            model: patch.model.clone(),
            thinking_effort: patch.thinking_effort.clone(),
            context_tokens: patch.context_tokens,
            visibility: patch.visibility.clone(),
        };
        let resp: BotCreateResp = self
            .pool
            .send_pb(reqwest::Method::POST, "/api/v1/bot/update", Some(&body))
            .await
            .map_err(control_error)?;
        match resp.profile {
            Some(p) => Ok(BotRecord {
                profile: from_pb_profile(p),
                config: from_pb_config(resp.config),
            }),
            None => Err(BotControlError::Backend(
                "bot update missing profile".into(),
            )),
        }
    }
}
