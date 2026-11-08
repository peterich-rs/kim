use super::*;
use kim_protocol::PROFILE_KIND_BOT;
use kim_server_data::directory::base36_upper;
use kim_server_data::idgen::IdGenerator;
use kim_server_data::social::{MemorySocialDirectory, SocialDirectory};
use kim_server_data::users::{
    normalize_bot_config, valid_client_profile_id, validate_patch, BotConfig, MemoryUserDirectory,
    ProfilePatch, UserDirectory, UserError, UserPresence, UserProfile,
};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

#[derive(Clone)]
struct BotEntry {
    record: BotRecord,
    owner: String,
    profile_id: String,
}

#[derive(Default)]
struct State {
    bots: HashMap<(String, String), BotEntry>,
    receipts: HashMap<(String, String), DeletionLease>,
}

/// The bot projection and receipts share one lock and one lifetime.
pub struct MemoryBotIdentity {
    users: Arc<MemoryUserDirectory>,
    social: Arc<MemorySocialDirectory>,
    idgen: Arc<dyn IdGenerator>,
    state: Mutex<State>,
    lifecycle: tokio::sync::Mutex<()>,
}

impl MemoryBotIdentity {
    pub fn new(
        users: Arc<MemoryUserDirectory>,
        social: Arc<MemorySocialDirectory>,
        idgen: Arc<dyn IdGenerator>,
    ) -> Self {
        Self {
            users,
            social,
            idgen,
            state: Mutex::new(State::default()),
            lifecycle: tokio::sync::Mutex::new(()),
        }
    }
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn entry(&self, app: &str, bot: &str) -> Option<BotEntry> {
        self.state().bots.get(&(app.into(), bot.into())).cloned()
    }
}

#[async_trait]
impl BotControl for MemoryBotIdentity {
    async fn create_bot(&self, app: &str, req: &CreateBot) -> Result<BotRecord, BotControlError> {
        let _lifecycle = self.lifecycle.lock().await;
        if !valid_client_profile_id(&req.client_profile_id) || req.owner.is_empty() {
            return Err(BotControlError::Invalid);
        }
        let config = normalize_bot_config(&req.config())?;
        let record = {
            let mut state = self.state();
            if let Some(entry) = state
                .bots
                .iter_mut()
                .find(|((row_app, _), entry)| {
                    row_app == app
                        && entry.owner == req.owner
                        && entry.profile_id == req.client_profile_id
                })
                .map(|(_, entry)| entry)
            {
                entry.record.config = config;
                entry.record.clone()
            } else {
                let count = state
                    .bots
                    .iter()
                    .filter(|((row_app, _), entry)| row_app == app && entry.owner == req.owner)
                    .count();
                if count >= BOT_MAX_PER_OWNER as usize {
                    return Err(BotControlError::Invalid);
                }
                let account = format!(
                    "b_{}",
                    base36_upper(self.idgen.next_id().map_err(UserError::from)?)
                );
                let patch = validate_patch(&ProfilePatch {
                    nickname: if req.nickname.trim().is_empty() {
                        account.clone()
                    } else {
                        req.nickname.clone()
                    },
                    avatar: req.avatar.clone(),
                    bio: req.bio.clone(),
                })?;
                let record = BotRecord {
                    profile: UserProfile {
                        account: account.clone(),
                        nickname: patch.nickname,
                        avatar: patch.avatar,
                        bio: patch.bio,
                        kind: PROFILE_KIND_BOT,
                    },
                    config,
                };
                let key = (app.into(), account);
                if state.receipts.contains_key(&key) || state.bots.contains_key(&key) {
                    return Err(BotControlError::Invalid);
                }
                state.bots.insert(
                    key,
                    BotEntry {
                        record: record.clone(),
                        owner: req.owner.clone(),
                        profile_id: req.client_profile_id.clone(),
                    },
                );
                record
            }
        };
        self.social
            .ensure_friends(app, &req.owner, &record.profile.account)
            .await
            .map_err(|e| BotControlError::Backend(e.to_string()))?;
        Ok(record)
    }
    async fn update_bot(
        &self,
        app: &str,
        owner: &str,
        account: &str,
        patch: &BotPatch,
    ) -> Result<BotRecord, BotControlError> {
        let _lifecycle = self.lifecycle.lock().await;
        let mut state = self.state();
        let entry = state
            .bots
            .get_mut(&(app.into(), account.into()))
            .ok_or(BotControlError::NotFound)?;
        if entry.owner != owner {
            return Err(BotControlError::NotOwner);
        }
        let profile = validate_patch(&patch.profile_patch())?;
        let config = normalize_bot_config(&patch.config())?;
        entry.record.profile.nickname = profile.nickname;
        entry.record.profile.avatar = profile.avatar;
        entry.record.profile.bio = profile.bio;
        entry.record.config = config;
        Ok(entry.record.clone())
    }
    async fn delete_identity_or_resume(
        &self,
        app: &str,
        actor: &str,
        bot: &str,
        scope: &StoreScope,
    ) -> Result<DeletionLease, BotControlError> {
        let _lifecycle = self.lifecycle.lock().await;
        validate(actor, bot, scope)?;
        let lease = {
            let mut state = self.state();
            let key = (app.into(), bot.into());
            if let Some(lease) = state.receipts.get(&key) {
                verify(lease, actor, scope)?;
                lease.clone()
            } else {
                let entry = state.bots.get(&key).ok_or(BotControlError::NotFound)?;
                if entry.owner != actor {
                    return Err(BotControlError::NotOwner);
                }
                let lease = DeletionLease {
                    deletion_id: uuid::Uuid::new_v4().to_string(),
                    original_owner: actor.into(),
                    cleanup_scope: scope.key(),
                    done: false,
                };
                state.receipts.insert(key.clone(), lease.clone());
                state.bots.remove(&key);
                lease
            }
        };
        self.social
            .remove(app, actor, bot)
            .await
            .map_err(|e| BotControlError::Backend(e.to_string()))?;
        Ok(lease)
    }
    async fn complete_delete(
        &self,
        app: &str,
        deletion_id: &str,
        actor: &str,
        scope: &StoreScope,
    ) -> Result<(), BotControlError> {
        let mut state = self.state();
        let lease = state
            .receipts
            .iter_mut()
            .find(|((row_app, _), lease)| row_app == app && lease.deletion_id == deletion_id)
            .map(|(_, lease)| lease)
            .ok_or(BotControlError::NotFound)?;
        verify(lease, actor, scope)?;
        lease.done = true;
        Ok(())
    }
}

#[async_trait]
impl UserDirectory for MemoryBotIdentity {
    async fn upsert(&self, app: &str, account: &str) -> Result<(), UserError> {
        if self.entry(app, account).is_some() {
            return Ok(());
        }
        if account.starts_with("b_") && !self.users.exists(app, account).await? {
            return Err(UserError::InvalidProfile);
        }
        self.users.upsert(app, account).await
    }
    async fn exists(&self, app: &str, account: &str) -> Result<bool, UserError> {
        if self.entry(app, account).is_some() {
            return Ok(true);
        }
        self.users.exists(app, account).await
    }
    async fn profile(&self, app: &str, account: &str) -> Result<Option<UserProfile>, UserError> {
        if let Some(entry) = self.entry(app, account) {
            return Ok(Some(entry.record.profile));
        }
        self.users.profile(app, account).await
    }
    async fn update_profile(
        &self,
        app: &str,
        account: &str,
        patch: &ProfilePatch,
    ) -> Result<UserProfile, UserError> {
        if self.entry(app, account).is_some() {
            return Err(UserError::InvalidProfile);
        }
        self.users.update_profile(app, account, patch).await
    }
    async fn profiles(
        &self,
        app: &str,
        accounts: &[String],
    ) -> Result<Vec<UserProfile>, UserError> {
        let mut profiles = Vec::new();
        for account in accounts {
            if let Some(profile) = self.profile(app, account).await? {
                profiles.push(profile);
            }
        }
        Ok(profiles)
    }
    async fn search(
        &self,
        app: &str,
        query: &str,
        exclude: &[String],
        limit: usize,
    ) -> Result<Vec<UserProfile>, UserError> {
        self.users.search(app, query, exclude, limit).await
    }
    async fn bot_config(
        &self,
        app: &str,
        owner: &str,
        account: &str,
    ) -> Result<BotConfig, UserError> {
        let entry = self.entry(app, account).ok_or(UserError::NotFound)?;
        if entry.owner != owner {
            return Err(UserError::NotBotOwner);
        }
        Ok(entry.record.config)
    }
    async fn bot_owner(&self, app: &str, account: &str) -> Result<Option<String>, UserError> {
        Ok(self.entry(app, account).map(|entry| entry.owner))
    }
    async fn lookup(&self, app: &str, account: &str) -> Result<Option<UserPresence>, UserError> {
        if let Some(entry) = self.entry(app, account) {
            return Ok(Some(UserPresence {
                exists: true,
                kind: PROFILE_KIND_BOT,
                owner_account: entry.owner,
            }));
        }
        self.users.lookup(app, account).await
    }
}
