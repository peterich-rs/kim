use kim_metrics::KimMetrics;
use kim_protocol::pkt::{
    BotCreateReq, BotCreateResp, BotPendingItem as PbPending, BotPendingResp, BotReplyReq,
    InboxReq, Status, UserProfileUpdate,
};
use kim_protocol::{CMD_CHAT_USER_TALK, PROFILE_KIND_BOT};
use kim_router::{Context, RouterError};
use tracing::warn;

use crate::filter::ContentFilter;
use crate::profile::to_pb;
use crate::store::{BotPendingItem, InsertMessage, MessageStore, StoreError};
use crate::talk::{fallback_targets, persist_then_push, unix_nano, TalkError};
use crate::users::{valid_client_profile_id, CreateBot, UserDirectory, UserError, UserPresence};

pub(crate) fn store_status(err: &StoreError) -> Status {
    match err {
        StoreError::Invalid(_) => Status::InvalidPacketBody,
        StoreError::NotFound => Status::UserNotFound,
        StoreError::Http { status: 400, .. } => Status::InvalidPacketBody,
        StoreError::Http { status: 403, .. } => Status::NotBotOwner,
        StoreError::Http { status: 404, .. } => Status::UserNotFound,
        _ => Status::SystemException,
    }
}

pub(crate) fn user_status(err: &UserError) -> Status {
    match err {
        UserError::NotFound => Status::UserNotFound,
        UserError::InvalidProfile | UserError::Limit => Status::InvalidPacketBody,
        UserError::NotBotOwner => Status::NotBotOwner,
        UserError::Conflict => Status::InvalidPacketBody,
        UserError::Backend(_) => Status::SystemException,
    }
}

pub(crate) async fn lookup_bot(
    users: &dyn UserDirectory,
    app: &str,
    account: &str,
) -> Result<UserPresence, Status> {
    match users.lookup(app, account).await {
        Ok(Some(p)) if p.exists && p.kind == PROFILE_KIND_BOT => Ok(p),
        Ok(Some(_)) | Ok(None) => Err(Status::UserNotFound),
        Err(_) => Err(Status::SystemException),
    }
}

pub async fn do_bot_create(ctx: Context, users: &dyn UserDirectory) -> Result<(), RouterError> {
    let req = match ctx.read_body::<BotCreateReq>() {
        Ok(r) => r,
        Err(err) => {
            ctx.resp_with_error(Status::InvalidPacketBody, &err).await?;
            return Ok(());
        }
    };
    if !valid_client_profile_id(&req.client_profile_id) {
        ctx.resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await?;
        return Ok(());
    }
    match users
        .create_bot(
            &ctx.session().app,
            &CreateBot {
                owner: ctx.session().account.clone(),
                client_profile_id: req.client_profile_id,
                nickname: req.nickname,
                avatar: req.avatar,
                bio: req.bio,
            },
        )
        .await
    {
        Ok(profile) => {
            ctx.resp(
                Status::Success,
                Some(&BotCreateResp {
                    profile: Some(to_pb(&profile)),
                }),
            )
            .await?;
        }
        Err(err) => {
            warn!(%err, "bot create failed");
            ctx.resp_with_error(user_status(&err), &err).await?;
        }
    }
    Ok(())
}

pub async fn do_bot_delete(ctx: Context, users: &dyn UserDirectory) -> Result<(), RouterError> {
    if ctx.header().dest.is_empty() {
        ctx.resp_with_error(Status::NoDestination, &TalkError::NoDestination)
            .await?;
        return Ok(());
    }
    let dest = ctx.header().dest.clone();
    let presence = match lookup_bot(users, &ctx.session().app, &dest).await {
        Ok(p) => p,
        Err(status) => {
            ctx.resp_bytes(status, bytes::Bytes::new()).await?;
            return Ok(());
        }
    };
    if presence.owner_account != ctx.session().account {
        ctx.resp_bytes(Status::NotBotOwner, bytes::Bytes::new())
            .await?;
        return Ok(());
    }
    match users
        .delete_bot(&ctx.session().app, &ctx.session().account, &dest)
        .await
    {
        Ok(()) => {
            ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await?;
        }
        Err(err) => {
            ctx.resp_with_error(user_status(&err), &err).await?;
        }
    }
    Ok(())
}

pub async fn do_bot_update(ctx: Context, users: &dyn UserDirectory) -> Result<(), RouterError> {
    if ctx.header().dest.is_empty() {
        ctx.resp_with_error(Status::NoDestination, &TalkError::NoDestination)
            .await?;
        return Ok(());
    }
    let dest = ctx.header().dest.clone();
    let presence = match lookup_bot(users, &ctx.session().app, &dest).await {
        Ok(p) => p,
        Err(status) => {
            ctx.resp_bytes(status, bytes::Bytes::new()).await?;
            return Ok(());
        }
    };
    if presence.owner_account != ctx.session().account {
        ctx.resp_bytes(Status::NotBotOwner, bytes::Bytes::new())
            .await?;
        return Ok(());
    }
    let req = match ctx.read_body::<UserProfileUpdate>() {
        Ok(r) => r,
        Err(err) => {
            ctx.resp_with_error(Status::InvalidPacketBody, &err).await?;
            return Ok(());
        }
    };
    let patch = crate::users::ProfilePatch {
        nickname: req.nickname,
        avatar: req.avatar,
        bio: req.bio,
    };
    match users
        .update_profile(&ctx.session().app, &dest, &patch)
        .await
    {
        Ok(p) => {
            ctx.resp(Status::Success, Some(&to_pb(&p))).await?;
        }
        Err(err) => {
            ctx.resp_with_error(user_status(&err), &err).await?;
        }
    }
    Ok(())
}

pub async fn do_bot_reply(
    ctx: Context,
    store: &dyn MessageStore,
    filter: &dyn ContentFilter,
    users: &dyn UserDirectory,
    metrics: Option<&KimMetrics>,
    push_budget: std::time::Duration,
) -> Result<(), RouterError> {
    if ctx.header().dest.is_empty() {
        ctx.resp_with_error(Status::NoDestination, &TalkError::NoDestination)
            .await?;
        return Ok(());
    }
    let dest = ctx.header().dest.clone();
    let req = match ctx.read_body::<BotReplyReq>() {
        Ok(r) => r,
        Err(err) => {
            ctx.resp_with_error(Status::InvalidPacketBody, &err).await?;
            return Ok(());
        }
    };
    if req.in_reply_to <= 0 {
        ctx.resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await?;
        return Ok(());
    }
    let message = match req.message {
        Some(m) => m,
        None => {
            ctx.resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
                .await?;
            return Ok(());
        }
    };
    if let Err(status) = filter.check(&message).await {
        ctx.resp_with_error(status, &TalkError::ContentBlocked)
            .await?;
        return Ok(());
    }
    let presence = match lookup_bot(users, &ctx.session().app, &dest).await {
        Ok(p) => p,
        Err(status) => {
            ctx.resp_bytes(status, bytes::Bytes::new()).await?;
            return Ok(());
        }
    };
    if presence.owner_account != ctx.session().account {
        ctx.resp_bytes(Status::NotBotOwner, bytes::Bytes::new())
            .await?;
        return Ok(());
    }
    let owner = ctx.session().account.clone();
    let online_targets = fallback_targets(&ctx, std::slice::from_ref(&owner)).await;
    let inserted = match store
        .insert_bot_reply(
            &ctx.session().app,
            &owner,
            &dest,
            req.in_reply_to,
            &InsertMessage {
                sender: dest.clone(),
                dest: owner.clone(),
                send_time: unix_nano(),
                msg_type: message.r#type,
                body: message.body.clone(),
                extra: message.extra.clone(),
                client_id: message.client_id.clone(),
                online_targets,
            },
        )
        .await
    {
        Ok(v) => v,
        Err(err) => {
            warn!(%err, "insert_bot_reply failed");
            ctx.resp_with_error(store_status(&err), &err).await?;
            return Ok(());
        }
    };
    persist_then_push(
        &ctx,
        &inserted,
        "bot",
        metrics,
        push_budget,
        CMD_CHAT_USER_TALK,
    )
    .await?;
    Ok(())
}

pub async fn do_bot_pending(
    ctx: Context,
    store: &dyn MessageStore,
    users: &dyn UserDirectory,
) -> Result<(), RouterError> {
    if ctx.header().dest.is_empty() {
        ctx.resp_with_error(Status::NoDestination, &TalkError::NoDestination)
            .await?;
        return Ok(());
    }
    let dest = ctx.header().dest.clone();
    let presence = match lookup_bot(users, &ctx.session().app, &dest).await {
        Ok(p) => p,
        Err(status) => {
            ctx.resp_bytes(status, bytes::Bytes::new()).await?;
            return Ok(());
        }
    };
    if presence.owner_account != ctx.session().account {
        ctx.resp_bytes(Status::NotBotOwner, bytes::Bytes::new())
            .await?;
        return Ok(());
    }
    let limit = match ctx.read_body::<InboxReq>() {
        Ok(r) => r.limit,
        Err(_) => 20,
    };
    match store
        .bot_pending(&ctx.session().app, &ctx.session().account, &dest, limit)
        .await
    {
        Ok(items) => {
            ctx.resp(
                Status::Success,
                Some(&BotPendingResp {
                    items: items
                        .into_iter()
                        .map(|i: BotPendingItem| PbPending {
                            message_id: i.message_id,
                            body: i.body,
                            send_time: i.send_time,
                        })
                        .collect(),
                }),
            )
            .await?;
        }
        Err(err) => {
            ctx.resp_with_error(store_status(&err), &err).await?;
        }
    }
    Ok(())
}
