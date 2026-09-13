//! Room enter / leave handlers.

use kim_protocol::pkt::{RoomEnterReq, RoomEnterResp, RoomLeaveReq, Status};
use kim_protocol::{AccountId, INBOX_KIND_USER};
use kim_router::{Context, RouterError, SessionError};
use tracing::warn;

use crate::interest::RoomInterestStore;
use crate::presence::PresenceHub;
use crate::social::SocialDirectory;

pub async fn do_room_enter(
    ctx: Context,
    social: &dyn SocialDirectory,
    interest: &dyn RoomInterestStore,
) -> Result<(), RouterError> {
    let req = match ctx.read_body::<RoomEnterReq>() {
        Ok(r) => r,
        Err(err) => {
            ctx.resp_with_error(Status::InvalidPacketBody, &err).await?;
            return Ok(());
        }
    };
    let me = ctx.session().account.as_str();
    let app = ctx.session().app.as_str();
    let channel = ctx.session().channel_id.as_str();
    let dest = req.dest.trim();
    let kind = req.kind;

    if dest.is_empty() || dest == me {
        ctx.resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await?;
        return Ok(());
    }
    if kind != INBOX_KIND_USER {
        // v1: private chat only. Group presence strategy is out of scope.
        ctx.resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await?;
        return Ok(());
    }

    match social.is_blocked_either(app, me, dest).await {
        Ok(true) => {
            ctx.resp_bytes(Status::Blocked, bytes::Bytes::new()).await?;
            return Ok(());
        }
        Ok(false) => {}
        Err(err) => {
            warn!(%err, "room enter block check");
            ctx.resp_with_error(Status::SystemException, &err).await?;
            return Ok(());
        }
    }
    match social.is_friend(app, me, dest).await {
        Ok(true) => {}
        Ok(false) => {
            ctx.resp_bytes(Status::NotFriends, bytes::Bytes::new())
                .await?;
            return Ok(());
        }
        Err(err) => {
            warn!(%err, "room enter friend check");
            ctx.resp_with_error(Status::SystemException, &err).await?;
            return Ok(());
        }
    }

    if let Err(err) = interest.enter(app, me, channel, dest, kind).await {
        warn!(%err, "room enter interest write");
        ctx.resp_with_error(Status::SystemException, &err).await?;
        return Ok(());
    }

    let dest_id = match AccountId::parse(dest) {
        Ok(id) => id,
        Err(_) => {
            ctx.resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
                .await?;
            return Ok(());
        }
    };
    let locs = match ctx.list_locations(&dest_id).await {
        Ok(v) => v,
        Err(SessionError::NotFound) => Vec::new(),
        Err(err) => {
            warn!(%err, "room enter snapshot locations");
            ctx.resp_with_error(Status::SystemException, &err).await?;
            return Ok(());
        }
    };
    let presence = vec![PresenceHub::presence_of(dest, &locs)];
    let resp = RoomEnterResp { presence };
    ctx.resp(Status::Success, Some(&resp)).await?;
    Ok(())
}

pub async fn do_room_leave(
    ctx: Context,
    interest: &dyn RoomInterestStore,
) -> Result<(), RouterError> {
    let req = match ctx.read_body::<RoomLeaveReq>() {
        Ok(r) => r,
        Err(err) => {
            ctx.resp_with_error(Status::InvalidPacketBody, &err).await?;
            return Ok(());
        }
    };
    let me = ctx.session().account.as_str();
    let app = ctx.session().app.as_str();
    let channel = ctx.session().channel_id.as_str();
    let dest = req.dest.trim();
    if dest.is_empty() {
        ctx.resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await?;
        return Ok(());
    }
    if let Err(err) = interest.leave(app, me, channel, dest, req.kind).await {
        warn!(%err, "room leave interest write");
        ctx.resp_with_error(Status::SystemException, &err).await?;
        return Ok(());
    }
    ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await?;
    Ok(())
}
