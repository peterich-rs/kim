//! HMAC bot create / reply / pending.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use kim_protocol::pkt::{
    AccountExists, AccountPair, BotConfig as PbBotConfig, BotCreateResp,
    BotPendingItem as PbPending, BotPendingQuery, BotPendingResp, BotReplyStoreReq,
    InternalBotConfig, InternalBotCreate, InternalBotUpdate,
};
use kim_protocol::PROFILE_KIND_BOT;
use kim_server_data::store::StoreError;
use kim_server_data::users::{BotConfig, BotPatch, CreateBot, UserError};

use crate::product::to_pb;
use crate::{backend, decode, encode, insert_from_req, RoyalState};
use axum::Json;
use kim_server_data::bot_control::{
    BotControlError, CompleteDelete, DeleteIdentity, DeletionLease,
};
use kim_server_data::store::StoreScope;

fn control_http(err: BotControlError) -> (StatusCode, String) {
    let status = match err {
        BotControlError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        BotControlError::NotFound => StatusCode::NOT_FOUND,
        BotControlError::NotOwner => StatusCode::FORBIDDEN,
        BotControlError::Invalid => StatusCode::BAD_REQUEST,
        BotControlError::Backend(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, err.to_string())
}

fn check_app(st: &RoyalState, app: &str) -> Result<(), (StatusCode, String)> {
    if app != st.app {
        return Err((StatusCode::BAD_REQUEST, "app mismatch".into()));
    }
    Ok(())
}

pub async fn store_scope(
    State(st): State<RoyalState>,
) -> Result<Json<StoreScope>, (StatusCode, String)> {
    st.store
        .resolve_scope()
        .await
        .map(Json)
        .map_err(|err| (StatusCode::SERVICE_UNAVAILABLE, err.to_string()))
}

pub async fn delete_identity(
    State(st): State<RoyalState>,
    Json(req): Json<DeleteIdentity>,
) -> Result<Json<DeletionLease>, (StatusCode, String)> {
    check_app(&st, &req.app)?;
    st.bots
        .delete_identity_or_resume(&st.app, &req.actor, &req.bot, &req.scope)
        .await
        .map(Json)
        .map_err(control_http)
}

pub async fn complete_delete(
    State(st): State<RoyalState>,
    Json(req): Json<CompleteDelete>,
) -> Result<Json<bool>, (StatusCode, String)> {
    check_app(&st, &req.app)?;
    st.bots
        .complete_delete(&st.app, &req.deletion_id, &req.actor, &req.scope)
        .await
        .map_err(control_http)?;
    Ok(Json(true))
}

pub async fn scoped_purge(
    State(st): State<RoyalState>,
    Json(req): Json<DeleteIdentity>,
) -> Result<Json<bool>, (StatusCode, String)> {
    check_app(&st, &req.app)?;
    if req.actor.is_empty() || req.bot.is_empty() || req.actor == req.bot || !req.scope.is_scoped()
    {
        return Err((StatusCode::BAD_REQUEST, "invalid purge target".into()));
    }
    st.store
        .purge_peer_dm_scoped(&req.scope, &st.app, &req.actor, &req.bot)
        .await
        .map_err(store_http)?;
    Ok(Json(true))
}

fn to_pb_config(c: BotConfig) -> PbBotConfig {
    PbBotConfig {
        model: c.model,
        thinking_effort: c.thinking_effort,
        context_tokens: c.context_tokens,
        visibility: c.visibility,
    }
}

fn user_http(err: UserError) -> (StatusCode, String) {
    match err {
        UserError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
        UserError::InvalidProfile => (StatusCode::BAD_REQUEST, "invalid profile".into()),
        UserError::Limit => (StatusCode::BAD_REQUEST, "limit".into()),
        UserError::NotBotOwner => (StatusCode::FORBIDDEN, "not bot owner".into()),
        UserError::Conflict => (StatusCode::CONFLICT, "conflict".into()),
        UserError::Backend(e) => backend(e),
    }
}

fn store_http(err: StoreError) -> (StatusCode, String) {
    match err {
        StoreError::Invalid(msg) => (StatusCode::BAD_REQUEST, msg),
        StoreError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
        StoreError::Http { status, msg } => (
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            msg,
        ),
        other => backend(other),
    }
}

pub async fn bot_create(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InternalBotCreate>(&body)?;
    let record = st
        .bots
        .create_bot(
            &st.app,
            &CreateBot {
                owner: req.owner,
                client_profile_id: req.client_profile_id,
                nickname: req.nickname,
                avatar: req.avatar,
                bio: req.bio,
                model: req.model,
                thinking_effort: req.thinking_effort,
                context_tokens: req.context_tokens,
                visibility: req.visibility,
            },
        )
        .await
        .map_err(control_http)?;
    Ok(encode(&BotCreateResp {
        profile: Some(to_pb(record.profile)),
        config: Some(to_pb_config(record.config)),
    }))
}

pub async fn bot_delete(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    kim_server_data::bot_control::coordinate_delete(
        st.bots.as_ref(),
        st.store.as_ref(),
        &st.app,
        &req.account,
        &req.peer,
    )
    .await
    .map_err(|status| {
        let http = match status {
            kim_protocol::pkt::Status::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            kim_protocol::pkt::Status::UserNotFound => StatusCode::NOT_FOUND,
            kim_protocol::pkt::Status::NotBotOwner => StatusCode::FORBIDDEN,
            kim_protocol::pkt::Status::InvalidPacketBody => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (http, format!("bot delete: {}", status as i32))
    })?;
    Ok(encode(&AccountExists {
        exists: true,
        kind: 0,
        owner_account: String::new(),
    }))
}

pub async fn purge_dm(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    st.store
        .purge_peer_dm(&st.app, &req.account, &req.peer)
        .await
        .map_err(store_http)?;
    Ok(encode(&AccountExists {
        exists: true,
        kind: 0,
        owner_account: String::new(),
    }))
}

pub async fn bot_update(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InternalBotUpdate>(&body)?;
    let record = st
        .bots
        .update_bot(
            &st.app,
            &req.owner,
            &req.account,
            &BotPatch {
                nickname: req.nickname,
                avatar: req.avatar,
                bio: req.bio,
                model: req.model,
                thinking_effort: req.thinking_effort,
                context_tokens: req.context_tokens,
                visibility: req.visibility,
            },
        )
        .await
        .map_err(control_http)?;
    Ok(encode(&BotCreateResp {
        profile: Some(to_pb(record.profile)),
        config: Some(to_pb_config(record.config)),
    }))
}

pub async fn bot_config(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InternalBotConfig>(&body)?;
    let config = st
        .users
        .bot_config(&st.app, &req.owner, &req.account)
        .await
        .map_err(user_http)?;
    Ok(encode(&InternalBotConfig {
        owner: req.owner,
        account: req.account,
        config: Some(to_pb_config(config)),
    }))
}

pub async fn bot_reply(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<BotReplyStoreReq>(&body)?;
    let insert = req
        .insert
        .ok_or((StatusCode::BAD_REQUEST, "insert".into()))?;
    let inserted = st
        .store
        .insert_bot_reply(
            &st.app,
            &req.owner,
            &req.bot_account,
            req.in_reply_to,
            &insert_from_req(insert),
        )
        .await
        .map_err(store_http)?;
    Ok(encode(&crate::encode_insert(&inserted)))
}

pub async fn bot_pending(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<BotPendingQuery>(&body)?;
    match st.users.lookup(&st.app, &req.bot_account).await {
        Ok(Some(p)) if p.kind == PROFILE_KIND_BOT && p.owner_account == req.owner => {}
        Ok(Some(p)) if p.kind == PROFILE_KIND_BOT => {
            return Err((StatusCode::FORBIDDEN, "not bot owner".into()));
        }
        Ok(_) => return Err((StatusCode::NOT_FOUND, "not found".into())),
        Err(err) => return Err(user_http(err)),
    }
    let items = st
        .store
        .bot_pending(&st.app, &req.owner, &req.bot_account, req.limit)
        .await
        .map_err(store_http)?;
    Ok(encode(&BotPendingResp {
        items: items
            .into_iter()
            .map(|i| PbPending {
                message_id: i.message_id,
                body: i.body,
                send_time: i.send_time,
            })
            .collect(),
    }))
}
