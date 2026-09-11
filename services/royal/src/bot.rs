//! HMAC bot create / reply / pending.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use chat::store::StoreError;
use chat::users::{CreateBot, UserError};
use kim_protocol::pkt::{
    AccountExists, AccountPair, BotCreateResp, BotPendingItem as PbPending, BotPendingQuery,
    BotPendingResp, BotReplyStoreReq, InternalBotCreate,
};
use kim_protocol::PROFILE_KIND_BOT;

use crate::product::to_pb;
use crate::{backend, decode, encode, insert_from_req, RoyalState};

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
    let profile = st
        .users
        .create_bot(
            &st.app,
            &CreateBot {
                owner: req.owner,
                client_profile_id: req.client_profile_id,
                nickname: req.nickname,
                avatar: req.avatar,
                bio: req.bio,
            },
        )
        .await
        .map_err(user_http)?;
    Ok(encode(&BotCreateResp {
        profile: Some(to_pb(profile)),
    }))
}

pub async fn bot_delete(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    st.users
        .delete_bot(&st.app, &req.account, &req.peer)
        .await
        .map_err(user_http)?;
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
    let req = decode::<kim_protocol::pkt::InternalBotUpdate>(&body)?;
    match st.users.lookup(&st.app, &req.account).await {
        Ok(Some(p)) if p.kind == PROFILE_KIND_BOT && p.owner_account == req.owner => {}
        Ok(Some(p)) if p.kind == PROFILE_KIND_BOT => {
            return Err((StatusCode::FORBIDDEN, "not bot owner".into()));
        }
        Ok(_) => return Err((StatusCode::NOT_FOUND, "not found".into())),
        Err(err) => return Err(user_http(err)),
    }
    let patch = chat::users::ProfilePatch {
        nickname: req.nickname,
        avatar: req.avatar,
        bio: req.bio,
    };
    let p = st
        .users
        .update_profile(&st.app, &req.account, &patch)
        .await
        .map_err(user_http)?;
    Ok(encode(&to_pb(p)))
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
