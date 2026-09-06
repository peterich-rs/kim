//! Conversation-scoped typing: ephemeral fanout via room interest (no DB).

use std::collections::HashSet;

use kim_protocol::pkt::{Status, TypingPush, TypingReq};
use kim_protocol::{CMD_TYPING, INBOX_KIND_USER};
use kim_router::{Context, SessionError};
use tracing::warn;

use crate::interest::RoomInterestStore;
use crate::social::SocialDirectory;

/// Validate friend + private; fanout to peer devices that entered the typer's room.
pub async fn do_typing(
    ctx: Context,
    social: &dyn SocialDirectory,
    interest: &dyn RoomInterestStore,
) {
    let req = match ctx.read_body::<TypingReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let me = ctx.session().account.as_str();
    let app = ctx.session().app.as_str();
    let dest = req.dest.trim();
    let kind = req.kind;

    if dest.is_empty() || dest == me {
        let _ = ctx
            .resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await;
        return;
    }
    if kind != INBOX_KIND_USER {
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
            warn!(%err, "typing block check");
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
            warn!(%err, "typing friend check");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    }

    let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;

    let body = TypingPush {
        typer: me.to_string(),
        dest: dest.to_string(),
        kind,
        active: req.active,
    };

    // People watching a DM with the typer (entered dest=me). Keep only the peer.
    let viewers = match interest.viewers(app, me, INBOX_KIND_USER).await {
        Ok(v) => v,
        Err(err) => {
            warn!(%err, "typing list viewers");
            return;
        }
    };
    let peer_channels: Vec<String> = viewers
        .into_iter()
        .filter(|v| v.account == dest)
        .map(|v| v.channel_id)
        .collect();
    if peer_channels.is_empty() {
        return;
    }
    let locs = match ctx.list_locations(dest).await {
        Ok(v) => v,
        Err(SessionError::NotFound) => return,
        Err(err) => {
            warn!(%err, dest, "typing peer locations");
            return;
        }
    };
    let want: HashSet<&str> = peer_channels.iter().map(|c| c.as_str()).collect();
    let recvs: Vec<_> = locs
        .into_iter()
        .filter(|l| want.contains(l.channel_id.as_str()))
        .collect();
    if recvs.is_empty() {
        return;
    }
    if let Err(err) = ctx.dispatch_cmd(CMD_TYPING, &body, &recvs).await {
        warn!(%err, dest, "typing fanout failed");
    }
}
