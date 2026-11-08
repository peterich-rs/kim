//! Royal owns bot lifecycle and deletion receipts. No HTTP waits occur in a PG transaction.

use async_trait::async_trait;
use kim_server_data::bot_control::{BotControl, BotControlError, DeletionLease};
use kim_server_data::store::StoreScope;
use kim_server_data::users::{BotPatch, BotRecord, CreateBot};
use std::sync::Arc;

fn validate(actor: &str, bot: &str, scope: &StoreScope) -> Result<(), BotControlError> {
    if actor.is_empty() || bot.is_empty() || actor == bot || !scope.is_scoped() {
        return Err(BotControlError::Invalid);
    }
    Ok(())
}

fn verify(lease: &DeletionLease, actor: &str, scope: &StoreScope) -> Result<(), BotControlError> {
    if lease.original_owner != actor {
        return Err(BotControlError::NotOwner);
    }
    if lease.cleanup_scope != scope.key() {
        return Err(BotControlError::Backend("cleanup scope mismatch".into()));
    }
    Ok(())
}

const BOT_MAX_PER_OWNER: u32 = 20;

mod memory;
pub use memory::MemoryBotIdentity;

#[cfg(feature = "postgres")]
mod pg {
    use super::*;
    use kim_protocol::PROFILE_KIND_BOT;
    use kim_server_data::directory::base36_upper;
    use kim_server_data::idgen::IdGenerator;
    use kim_server_data::users::{
        normalize_bot_config, valid_client_profile_id, validate_patch, BotConfig, ProfilePatch,
        UserError, UserProfile, AVATAR_MAX_CHARS, BIO_MAX_CHARS,
    };

    pub struct PostgresBotIdentity {
        pool: sqlx::PgPool,
        idgen: Option<Arc<dyn IdGenerator>>,
    }

    fn pg_err(error: sqlx::Error) -> UserError {
        UserError::Backend(error.to_string())
    }
    fn backend(error: impl std::fmt::Display) -> BotControlError {
        BotControlError::Backend(error.to_string())
    }
    type ReceiptRow = (String, String, String, String);

    fn lease(row: ReceiptRow) -> DeletionLease {
        DeletionLease {
            deletion_id: row.0,
            original_owner: row.1,
            cleanup_scope: row.2,
            done: row.3 == "done",
        }
    }

    impl PostgresBotIdentity {
        pub fn new(pool: sqlx::PgPool, idgen: Arc<dyn IdGenerator>) -> Self {
            Self {
                pool,
                idgen: Some(idgen),
            }
        }
        pub async fn connect(url: &str, idgen: Arc<dyn IdGenerator>) -> Result<Self, UserError> {
            let pool = kim_server_data::store::connect_pool(
                url,
                kim_server_data::store::PoolOpts::default(),
            )
            .await
            .map_err(|e| UserError::Backend(e.to_string()))?;
            Ok(Self::new(pool, idgen))
        }
    }

    impl PostgresBotIdentity {
        async fn create_identity(
            &self,
            app: &str,
            req: &CreateBot,
        ) -> Result<BotRecord, UserError> {
            let idgen = self
                .idgen
                .as_ref()
                .ok_or_else(|| UserError::Backend("create_bot requires idgen".into()))?;
            if !valid_client_profile_id(&req.client_profile_id) {
                return Err(UserError::InvalidProfile);
            }
            if req.owner.is_empty() {
                return Err(UserError::InvalidProfile);
            }
            let config = normalize_bot_config(&req.config())?;
            let mut tx = self.pool.begin().await.map_err(pg_err)?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('bot-owner:' || $1 || ':' || $2, 0))").bind(app).bind(&req.owner).execute(&mut *tx).await.map_err(pg_err)?;
            let existing: Option<(String, String, String, String, String)> = sqlx::query_as(
                "SELECT account, nickname, avatar, bio, kind FROM users
             WHERE app = $1 AND owner_account = $2 AND client_profile_id = $3 AND kind = 'bot'",
            )
            .bind(app)
            .bind(&req.owner)
            .bind(&req.client_profile_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(pg_err)?;
            if let Some((account, nickname, avatar, bio, kind)) = existing {
                let (a, b) = kim_server_data::social::ordered_pair(&req.owner, &account);
                sqlx::query(
                    "INSERT INTO friendships (app, account_a, account_b)
                 VALUES ($1, $2, $3)
                 ON CONFLICT DO NOTHING",
                )
                .bind(app)
                .bind(a)
                .bind(b)
                .execute(&mut *tx)
                .await
                .map_err(pg_err)?;
                upsert_bot_config_tx(&mut tx, app, &account, &config).await?;
                tx.commit().await.map_err(pg_err)?;
                return Ok(BotRecord {
                    profile: row_profile(account, nickname, avatar, bio, kind),
                    config,
                });
            }
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM users WHERE app = $1 AND owner_account = $2 AND kind = 'bot'",
            )
            .bind(app)
            .bind(&req.owner)
            .fetch_one(&mut *tx)
            .await
            .map_err(pg_err)?;
            if count >= i64::from(BOT_MAX_PER_OWNER) {
                return Err(UserError::Limit);
            }
            let account = format!("b_{}", base36_upper(idgen.next_id()?));
            let retired: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM bot_deletion WHERE app=$1 AND bot_account=$2)",
            )
            .bind(app)
            .bind(&account)
            .fetch_one(&mut *tx)
            .await
            .map_err(pg_err)?;
            if retired {
                return Err(UserError::Conflict);
            }
            let nickname = if req.nickname.trim().is_empty() {
                account.clone()
            } else {
                validate_patch(&ProfilePatch {
                    nickname: req.nickname.clone(),
                    avatar: req.avatar.clone(),
                    bio: req.bio.clone(),
                })?
                .nickname
            };
            let avatar = req.avatar.trim().to_string();
            let bio = req.bio.trim().to_string();
            if avatar.chars().count() > AVATAR_MAX_CHARS || bio.chars().count() > BIO_MAX_CHARS {
                return Err(UserError::InvalidProfile);
            }
            sqlx::query(
            "INSERT INTO users (app, account, nickname, avatar, bio, kind, owner_account, client_profile_id)
             VALUES ($1, $2, $3, $4, $5, 'bot', $6, $7)",
        )
        .bind(app)
        .bind(&account)
        .bind(&nickname)
        .bind(&avatar)
        .bind(&bio)
        .bind(&req.owner)
        .bind(&req.client_profile_id)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
            let (a, b) = kim_server_data::social::ordered_pair(&req.owner, &account);
            sqlx::query(
                "INSERT INTO friendships (app, account_a, account_b)
             VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
            )
            .bind(app)
            .bind(a)
            .bind(b)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
            upsert_bot_config_tx(&mut tx, app, &account, &config).await?;
            tx.commit().await.map_err(pg_err)?;
            Ok(BotRecord {
                profile: UserProfile {
                    account,
                    nickname,
                    avatar,
                    bio,
                    kind: PROFILE_KIND_BOT,
                },
                config,
            })
        }
        async fn update_identity(
            &self,
            app: &str,
            owner: &str,
            account: &str,
            patch: &BotPatch,
        ) -> Result<BotRecord, UserError> {
            let mut tx = self.pool.begin().await.map_err(pg_err)?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('bot-owner:' || $1 || ':' || $2, 0))").bind(app).bind(owner).execute(&mut *tx).await.map_err(pg_err)?;
            let identity: Option<(String, String)> = sqlx::query_as(
                "SELECT kind,owner_account FROM users WHERE app=$1 AND account=$2 FOR UPDATE",
            )
            .bind(app)
            .bind(account)
            .fetch_optional(&mut *tx)
            .await
            .map_err(pg_err)?;
            let (kind, original_owner) = identity.ok_or(UserError::NotFound)?;
            if kind != "bot" {
                return Err(UserError::NotFound);
            }
            if original_owner != owner {
                return Err(UserError::NotBotOwner);
            }
            let profile_patch = validate_patch(&patch.profile_patch())?;
            let config = normalize_bot_config(&patch.config())?;

            let row: Option<(String, String, String, String, String)> = sqlx::query_as(
                "UPDATE users SET nickname = $3, avatar = $4, bio = $5
             WHERE app = $1 AND account = $2 AND kind = 'bot' AND owner_account = $6
             RETURNING account, nickname, avatar, bio, kind",
            )
            .bind(app)
            .bind(account)
            .bind(&profile_patch.nickname)
            .bind(&profile_patch.avatar)
            .bind(&profile_patch.bio)
            .bind(owner)
            .fetch_optional(&mut *tx)
            .await
            .map_err(pg_err)?;
            let (account, nickname, avatar, bio, kind) = row.ok_or(UserError::NotFound)?;
            upsert_bot_config_tx(&mut tx, app, &account, &config).await?;
            tx.commit().await.map_err(pg_err)?;
            Ok(BotRecord {
                profile: row_profile(account, nickname, avatar, bio, kind),
                config,
            })
        }
    }

    fn row_profile(
        account: String,
        nickname: String,
        avatar: String,
        bio: String,
        kind: String,
    ) -> UserProfile {
        UserProfile {
            nickname: if nickname.is_empty() {
                account.clone()
            } else {
                nickname
            },
            account,
            avatar,
            bio,
            kind: kim_server_data::users::kind_from_db(&kind),
        }
    }

    async fn upsert_bot_config_tx(
        conn: &mut sqlx::PgConnection,
        app: &str,
        account: &str,
        cfg: &BotConfig,
    ) -> Result<(), UserError> {
        // sqlx 0.8: Executor is on &mut PgConnection, not &mut Transaction.
        sqlx::query(
        "INSERT INTO bot_config (app, bot_account, model, thinking_effort, context_tokens, visibility, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, now())
         ON CONFLICT (app, bot_account) DO UPDATE SET
           model = EXCLUDED.model,
           thinking_effort = EXCLUDED.thinking_effort,
           context_tokens = EXCLUDED.context_tokens,
           visibility = EXCLUDED.visibility,
           updated_at = now()",
    )
    .bind(app)
    .bind(account)
    .bind(&cfg.model)
    .bind(&cfg.thinking_effort)
    .bind(cfg.context_tokens)
    .bind(&cfg.visibility)
    .execute(&mut *conn)
    .await
    .map_err(pg_err)?;
        Ok(())
    }

    #[async_trait]
    impl BotControl for PostgresBotIdentity {
        async fn create_bot(
            &self,
            app: &str,
            req: &CreateBot,
        ) -> Result<BotRecord, BotControlError> {
            self.create_identity(app, req).await.map_err(Into::into)
        }
        async fn update_bot(
            &self,
            app: &str,
            owner: &str,
            account: &str,
            patch: &BotPatch,
        ) -> Result<BotRecord, BotControlError> {
            self.update_identity(app, owner, account, patch)
                .await
                .map_err(Into::into)
        }
        async fn delete_identity_or_resume(
            &self,
            app: &str,
            actor: &str,
            bot: &str,
            scope: &StoreScope,
        ) -> Result<DeletionLease, BotControlError> {
            validate(actor, bot, scope)?;
            let mut tx = self.pool.begin().await.map_err(backend)?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('bot-owner:' || $1 || ':' || $2, 0))").bind(app).bind(actor).execute(&mut *tx).await.map_err(backend)?;
            // Also serializes retries after the live identity row has gone away.
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('bot-delete:' || $1 || ':' || $2, 0))")
                .bind(app).bind(bot).execute(&mut *tx).await.map_err(backend)?;
            let existing: Option<ReceiptRow> = sqlx::query_as("SELECT deletion_id, original_owner, cleanup_scope, status FROM bot_deletion WHERE app=$1 AND bot_account=$2 FOR UPDATE")
                .bind(app).bind(bot).fetch_optional(&mut *tx).await.map_err(backend)?;
            if let Some(row) = existing {
                let lease = lease(row);
                verify(&lease, actor, scope)?;
                tx.commit().await.map_err(backend)?;
                return Ok(lease);
            }
            let identity: Option<(String, String)> = sqlx::query_as(
                "SELECT kind, owner_account FROM users WHERE app=$1 AND account=$2 FOR UPDATE",
            )
            .bind(app)
            .bind(bot)
            .fetch_optional(&mut *tx)
            .await
            .map_err(backend)?;
            let (kind, owner) = identity.ok_or(BotControlError::NotFound)?;
            if kind != "bot" {
                return Err(BotControlError::NotFound);
            }
            if owner != actor {
                return Err(BotControlError::NotOwner);
            }
            let result = DeletionLease {
                deletion_id: uuid::Uuid::new_v4().to_string(),
                original_owner: owner,
                cleanup_scope: scope.key(),
                done: false,
            };
            sqlx::query("INSERT INTO bot_deletion (deletion_id,app,bot_account,original_owner,cleanup_scope,status) VALUES($1,$2,$3,$4,$5,'pending')")
                .bind(&result.deletion_id).bind(app).bind(bot).bind(actor).bind(&result.cleanup_scope).execute(&mut *tx).await.map_err(backend)?;
            let (a, b) = kim_server_data::social::ordered_pair(actor, bot);
            sqlx::query("DELETE FROM friendships WHERE app=$1 AND account_a=$2 AND account_b=$3")
                .bind(app)
                .bind(a)
                .bind(b)
                .execute(&mut *tx)
                .await
                .map_err(backend)?;
            sqlx::query("DELETE FROM friend_requests WHERE app=$1 AND ((from_account=$2 AND to_account=$3) OR (from_account=$3 AND to_account=$2))")
                .bind(app).bind(actor).bind(bot).execute(&mut *tx).await.map_err(backend)?;
            sqlx::query("DELETE FROM blocks WHERE app=$1 AND ((account=$2 AND blocked=$3) OR (account=$3 AND blocked=$2))")
                .bind(app).bind(actor).bind(bot).execute(&mut *tx).await.map_err(backend)?;
            sqlx::query(
                "DELETE FROM users WHERE app=$1 AND account=$2 AND kind='bot' AND owner_account=$3",
            )
            .bind(app)
            .bind(bot)
            .bind(actor)
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
            tx.commit().await.map_err(backend)?;
            Ok(result)
        }
        async fn complete_delete(
            &self,
            app: &str,
            deletion_id: &str,
            actor: &str,
            scope: &StoreScope,
        ) -> Result<(), BotControlError> {
            if !scope.is_scoped() {
                return Err(BotControlError::Invalid);
            }
            let mut tx = self.pool.begin().await.map_err(backend)?;
            let row: ReceiptRow = sqlx::query_as("SELECT deletion_id, original_owner, cleanup_scope, status FROM bot_deletion WHERE app=$1 AND deletion_id=$2 FOR UPDATE")
                .bind(app).bind(deletion_id).fetch_optional(&mut *tx).await.map_err(backend)?.ok_or(BotControlError::NotFound)?;
            verify(&lease(row), actor, scope)?;
            sqlx::query("UPDATE bot_deletion SET status='done' WHERE app=$1 AND deletion_id=$2")
                .bind(app)
                .bind(deletion_id)
                .execute(&mut *tx)
                .await
                .map_err(backend)?;
            tx.commit().await.map_err(backend)?;
            Ok(())
        }
    }
}

#[cfg(feature = "postgres")]
pub use pg::PostgresBotIdentity;

#[cfg(test)]
mod tests;
