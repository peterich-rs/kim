//! Room enter / leave handlers.

use kim_protocol::pkt::{RoomEnterReq, RoomEnterResp, RoomLeaveReq, Status};
use kim_protocol::INBOX_KIND_USER;
use kim_router::{Context, SessionError};
use tracing::warn;

use crate::interest::RoomInterestStore;
use crate::presence::PresenceHub;
use crate::social::SocialDirectory;

pub async fn do_room_enter(
    ctx: Context,
    social: &dyn SocialDirectory,
    interest: &dyn RoomInterestStore,
) {
    let req = match ctx.read_body::<RoomEnterReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let me = ctx.session().account.as_str();
    let app = ctx.session().app.as_str();
    let channel = ctx.session().channel_id.as_str();
    let dest = req.dest.trim();
    let kind = req.kind;

    if dest.is_empty() || dest == me {
        let _ = ctx
            .resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await;
        return;
    }
    if kind != INBOX_KIND_USER {
        // v1: private chat only. Group presence strategy is out of scope.
        let _ = ctx
            .resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await;
        return;
    }

    match social.is_blocked_either(app, me, dest).await {
        Ok(true) => {
            let _ = ctx.resp_bytes(Status::Blocked, bytes::Bytes::new()).await;
            return;
        }
        Ok(false) => {}
        Err(err) => {
            warn!(%err, "room enter block check");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    }
    match social.is_friend(app, me, dest).await {
        Ok(true) => {}
        Ok(false) => {
            let _ = ctx
                .resp_bytes(Status::NotFriends, bytes::Bytes::new())
                .await;
            return;
        }
        Err(err) => {
            warn!(%err, "room enter friend check");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    }

    if let Err(err) = interest.enter(app, me, channel, dest, kind).await {
        warn!(%err, "room enter interest write");
        let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        return;
    }

    let locs = match ctx.list_locations(dest).await {
        Ok(v) => v,
        Err(SessionError::NotFound) => Vec::new(),
        Err(err) => {
            warn!(%err, "room enter snapshot locations");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    };
    let presence = vec![PresenceHub::presence_of(dest, &locs)];
    let resp = RoomEnterResp { presence };
    let _ = ctx.resp(Status::Success, Some(&resp)).await;
}

pub async fn do_room_leave(ctx: Context, interest: &dyn RoomInterestStore) {
    let req = match ctx.read_body::<RoomLeaveReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let me = ctx.session().account.as_str();
    let app = ctx.session().app.as_str();
    let channel = ctx.session().channel_id.as_str();
    let dest = req.dest.trim();
    if dest.is_empty() {
        let _ = ctx
            .resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await;
        return;
    }
    if let Err(err) = interest.leave(app, me, channel, dest, req.kind).await {
        warn!(%err, "room leave interest write");
        let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        return;
    }
    let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
}
