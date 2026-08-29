use kim_protocol::pkt::{FriendRequestNotify, Status, UserListResp};
use kim_router::{Context, SessionError};
use tracing::warn;

use crate::profile::profiles_pb;
use crate::social::{FriendRequestOutcome, SocialDirectory, SocialError};
use crate::users::{UserDirectory, UserError};

#[derive(Debug, thiserror::Error)]
enum FriendCmdError {
    #[error("no destination")]
    NoDestination,
    #[error("self")]
    SelfOp,
}

fn dest_account(ctx: &Context) -> Result<&str, FriendCmdError> {
    let dest = ctx.header().dest.as_str();
    if dest.is_empty() {
        Err(FriendCmdError::NoDestination)
    } else if dest == ctx.session().account {
        Err(FriendCmdError::SelfOp)
    } else {
        Ok(dest)
    }
}

fn social_status(err: &SocialError) -> Status {
    match err {
        SocialError::SelfOp => Status::InvalidPacketBody,
        SocialError::NotFound => Status::UserNotFound,
        SocialError::Blocked => Status::Blocked,
        SocialError::Backend(_) => Status::SystemException,
    }
}

async fn require_user(
    users: &dyn UserDirectory,
    app: &str,
    account: &str,
) -> Result<bool, UserError> {
    users.exists(app, account).await
}

async fn notify_peer(ctx: &Context, account: &str, body: &FriendRequestNotify) {
    match ctx.get_location(account, "").await {
        Ok(loc) => {
            if let Err(err) = ctx.dispatch(body, std::slice::from_ref(&loc)).await {
                warn!(%err, account, "friend notify failed");
            }
        }
        Err(SessionError::NotFound) => {}
        Err(err) => warn!(%err, account, "friend notify loc failed"),
    }
}

pub async fn do_friend_request(
    ctx: Context,
    social: &dyn SocialDirectory,
    users: &dyn UserDirectory,
) {
    let peer = match dest_account(&ctx) {
        Ok(d) => d,
        Err(FriendCmdError::NoDestination) => {
            let _ = ctx
                .resp_with_error(Status::NoDestination, &FriendCmdError::NoDestination)
                .await;
            return;
        }
        Err(FriendCmdError::SelfOp) => {
            let _ = ctx
                .resp_with_error(Status::InvalidPacketBody, &FriendCmdError::SelfOp)
                .await;
            return;
        }
    };
    match require_user(users, &ctx.session().app, peer).await {
        Ok(true) => {}
        Ok(false) => {
            let _ = ctx
                .resp_bytes(Status::UserNotFound, bytes::Bytes::new())
                .await;
            return;
        }
        Err(err) => {
            warn!(%err, "friend request user lookup failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    }
    let from = ctx.session().account.clone();
    let nickname = match users.profile(&ctx.session().app, &from).await {
        Ok(Some(p)) => p.display_name().to_string(),
        Ok(None) => from.clone(),
        Err(err) => {
            warn!(%err, "friend request profile failed");
            from.clone()
        }
    };
    match social.request(&ctx.session().app, &from, peer).await {
        Ok(FriendRequestOutcome::Sent) => {
            notify_peer(
                &ctx,
                peer,
                &FriendRequestNotify {
                    from_account: from,
                    from_nickname: nickname,
                },
            )
            .await;
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Ok(FriendRequestOutcome::AutoAccepted | FriendRequestOutcome::AlreadyFriends) => {
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(social_status(&err), &err).await;
        }
    }
}

pub async fn do_friend_accept(
    ctx: Context,
    social: &dyn SocialDirectory,
    users: &dyn UserDirectory,
) {
    let peer = match dest_account(&ctx) {
        Ok(d) => d,
        Err(FriendCmdError::NoDestination) => {
            let _ = ctx
                .resp_with_error(Status::NoDestination, &FriendCmdError::NoDestination)
                .await;
            return;
        }
        Err(FriendCmdError::SelfOp) => {
            let _ = ctx
                .resp_with_error(Status::InvalidPacketBody, &FriendCmdError::SelfOp)
                .await;
            return;
        }
    };
    match social
        .accept(&ctx.session().app, &ctx.session().account, peer)
        .await
    {
        Ok(()) => {
            let nickname = match users
                .profile(&ctx.session().app, &ctx.session().account)
                .await
            {
                Ok(Some(p)) => p.display_name().to_string(),
                _ => ctx.session().account.clone(),
            };
            notify_peer(
                &ctx,
                peer,
                &FriendRequestNotify {
                    from_account: ctx.session().account.clone(),
                    from_nickname: nickname,
                },
            )
            .await;
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(social_status(&err), &err).await;
        }
    }
}

pub async fn do_friend_reject(ctx: Context, social: &dyn SocialDirectory) {
    let peer = match dest_account(&ctx) {
        Ok(d) => d,
        Err(FriendCmdError::NoDestination) => {
            let _ = ctx
                .resp_with_error(Status::NoDestination, &FriendCmdError::NoDestination)
                .await;
            return;
        }
        Err(FriendCmdError::SelfOp) => {
            let _ = ctx
                .resp_with_error(Status::InvalidPacketBody, &FriendCmdError::SelfOp)
                .await;
            return;
        }
    };
    match social
        .reject(&ctx.session().app, &ctx.session().account, peer)
        .await
    {
        Ok(()) => {
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(social_status(&err), &err).await;
        }
    }
}

pub async fn do_friend_remove(ctx: Context, social: &dyn SocialDirectory) {
    let peer = match dest_account(&ctx) {
        Ok(d) => d,
        Err(FriendCmdError::NoDestination) => {
            let _ = ctx
                .resp_with_error(Status::NoDestination, &FriendCmdError::NoDestination)
                .await;
            return;
        }
        Err(FriendCmdError::SelfOp) => {
            let _ = ctx
                .resp_with_error(Status::InvalidPacketBody, &FriendCmdError::SelfOp)
                .await;
            return;
        }
    };
    match social
        .remove(&ctx.session().app, &ctx.session().account, peer)
        .await
    {
        Ok(()) => {
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(social_status(&err), &err).await;
        }
    }
}

pub async fn do_friend_list(ctx: Context, social: &dyn SocialDirectory, users: &dyn UserDirectory) {
    match social
        .list_friends(&ctx.session().app, &ctx.session().account)
        .await
    {
        Ok(accounts) => match profiles_pb(users, &ctx.session().app, &accounts).await {
            Ok(list) => {
                let _ = ctx
                    .resp(Status::Success, Some(&UserListResp { users: list }))
                    .await;
            }
            Err(err) => {
                warn!(%err, "friend list profiles failed");
                let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            }
        },
        Err(err) => {
            let _ = ctx.resp_with_error(social_status(&err), &err).await;
        }
    }
}

pub async fn do_friend_incoming(
    ctx: Context,
    social: &dyn SocialDirectory,
    users: &dyn UserDirectory,
) {
    match social
        .incoming(&ctx.session().app, &ctx.session().account)
        .await
    {
        Ok(accounts) => match profiles_pb(users, &ctx.session().app, &accounts).await {
            Ok(list) => {
                let _ = ctx
                    .resp(Status::Success, Some(&UserListResp { users: list }))
                    .await;
            }
            Err(err) => {
                warn!(%err, "incoming profiles failed");
                let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            }
        },
        Err(err) => {
            let _ = ctx.resp_with_error(social_status(&err), &err).await;
        }
    }
}

pub async fn do_block_add(ctx: Context, social: &dyn SocialDirectory, users: &dyn UserDirectory) {
    let peer = match dest_account(&ctx) {
        Ok(d) => d,
        Err(FriendCmdError::NoDestination) => {
            let _ = ctx
                .resp_with_error(Status::NoDestination, &FriendCmdError::NoDestination)
                .await;
            return;
        }
        Err(FriendCmdError::SelfOp) => {
            let _ = ctx
                .resp_with_error(Status::InvalidPacketBody, &FriendCmdError::SelfOp)
                .await;
            return;
        }
    };
    match require_user(users, &ctx.session().app, peer).await {
        Ok(true) => {}
        Ok(false) => {
            let _ = ctx
                .resp_bytes(Status::UserNotFound, bytes::Bytes::new())
                .await;
            return;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    }
    match social
        .block(&ctx.session().app, &ctx.session().account, peer)
        .await
    {
        Ok(()) => {
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(social_status(&err), &err).await;
        }
    }
}

pub async fn do_block_remove(ctx: Context, social: &dyn SocialDirectory) {
    let peer = match dest_account(&ctx) {
        Ok(d) => d,
        Err(FriendCmdError::NoDestination) => {
            let _ = ctx
                .resp_with_error(Status::NoDestination, &FriendCmdError::NoDestination)
                .await;
            return;
        }
        Err(FriendCmdError::SelfOp) => {
            let _ = ctx
                .resp_with_error(Status::InvalidPacketBody, &FriendCmdError::SelfOp)
                .await;
            return;
        }
    };
    match social
        .unblock(&ctx.session().app, &ctx.session().account, peer)
        .await
    {
        Ok(()) => {
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Err(err) => {
            let _ = ctx.resp_with_error(social_status(&err), &err).await;
        }
    }
}

pub async fn do_block_list(ctx: Context, social: &dyn SocialDirectory, users: &dyn UserDirectory) {
    match social
        .list_blocked(&ctx.session().app, &ctx.session().account)
        .await
    {
        Ok(accounts) => match profiles_pb(users, &ctx.session().app, &accounts).await {
            Ok(list) => {
                let _ = ctx
                    .resp(Status::Success, Some(&UserListResp { users: list }))
                    .await;
            }
            Err(err) => {
                let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            }
        },
        Err(err) => {
            let _ = ctx.resp_with_error(social_status(&err), &err).await;
        }
    }
}
