use kim_protocol::pkt::{
    Status, UserProfile as PbProfile, UserProfileUpdate, UserSearchReq, UserSearchResp,
};
use kim_router::Context;
use tracing::warn;

use crate::social::SocialDirectory;
use crate::users::{
    validate_patch, ProfilePatch, UserDirectory, UserError, UserProfile, SEARCH_LIMIT,
};

pub(crate) fn to_pb(p: &UserProfile) -> PbProfile {
    PbProfile {
        account: p.account.clone(),
        nickname: p.nickname.clone(),
        avatar: p.avatar.clone(),
        bio: p.bio.clone(),
    }
}

pub(crate) async fn profiles_pb(
    users: &dyn UserDirectory,
    app: &str,
    accounts: &[String],
) -> Result<Vec<PbProfile>, UserError> {
    let rows = users.profiles(app, accounts).await?;
    Ok(rows.iter().map(to_pb).collect())
}

pub async fn do_user_profile(ctx: Context, users: &dyn UserDirectory) {
    let dest = ctx.header().dest.as_str();
    let account = if dest.is_empty() {
        ctx.session().account.as_str()
    } else {
        dest
    };
    match users.profile(&ctx.session().app, account).await {
        Ok(Some(p)) => {
            let _ = ctx.resp(Status::Success, Some(&to_pb(&p))).await;
        }
        Ok(None) => {
            let _ = ctx
                .resp_bytes(Status::UserNotFound, bytes::Bytes::new())
                .await;
        }
        Err(err) => {
            warn!(%err, "profile failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        }
    }
}

pub async fn do_user_update(ctx: Context, users: &dyn UserDirectory) {
    let req = match ctx.read_body::<UserProfileUpdate>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let dest = ctx.header().dest.as_str();
    if !dest.is_empty() && dest != ctx.session().account {
        let _ = ctx
            .resp_bytes(Status::Unauthorized, bytes::Bytes::new())
            .await;
        return;
    }
    let patch = ProfilePatch {
        nickname: req.nickname,
        avatar: req.avatar,
        bio: req.bio,
    };
    if let Err(UserError::InvalidProfile) = validate_patch(&patch) {
        let _ = ctx
            .resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await;
        return;
    }
    match users
        .update_profile(&ctx.session().app, &ctx.session().account, &patch)
        .await
    {
        Ok(p) => {
            let _ = ctx.resp(Status::Success, Some(&to_pb(&p))).await;
        }
        Err(UserError::NotFound) => {
            let _ = ctx
                .resp_bytes(Status::UserNotFound, bytes::Bytes::new())
                .await;
        }
        Err(UserError::InvalidProfile) => {
            let _ = ctx
                .resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
                .await;
        }
        Err(err) => {
            warn!(%err, "update profile failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        }
    }
}

pub async fn do_user_search(ctx: Context, users: &dyn UserDirectory, social: &dyn SocialDirectory) {
    let req = match ctx.read_body::<UserSearchReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let me = ctx.session().account.as_str();
    let mut exclude = vec![me.to_string()];
    match social.list_blocked(&ctx.session().app, me).await {
        Ok(blocked) => exclude.extend(blocked),
        Err(err) => {
            warn!(%err, "list blocked for search failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    }
    match users
        .search(&ctx.session().app, &req.query, &exclude, SEARCH_LIMIT)
        .await
    {
        Ok(rows) => {
            let mut users_pb: Vec<PbProfile> = Vec::with_capacity(rows.len());
            for p in rows {
                match social
                    .is_blocked_either(&ctx.session().app, me, &p.account)
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => users_pb.push(to_pb(&p)),
                    Err(err) => {
                        warn!(%err, "block check failed");
                        let _ = ctx.resp_with_error(Status::SystemException, &err).await;
                        return;
                    }
                }
            }
            let resp = UserSearchResp { users: users_pb };
            let _ = ctx.resp(Status::Success, Some(&resp)).await;
        }
        Err(err) => {
            warn!(%err, "search failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        }
    }
}
