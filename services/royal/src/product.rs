//! Profile, social, inbox HTTP. Called by Chat `Http*` adapters.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use chat::parse_kind;
use chat::social::{FriendRequestOutcome, SocialError};
use chat::store::InboxEntry;
use chat::users::{ProfilePatch, UserError, UserProfile};
use kim_protocol::pkt::{
    AccountExists, AccountList, AccountPair, AccountQuery, ConversationRead, HistoryItem,
    HistoryQuery, HistoryResp, InboxItem, InboxQuery, InboxResp, ProfileUpdateReq, UserListResp,
    UserProfile as PbProfile, UserSearchQuery, UserSearchResp,
};
use kim_protocol::{INBOX_KIND_GROUP, INBOX_KIND_USER};

use crate::{backend, decode, encode, RoyalState};

fn social_http(err: SocialError) -> (StatusCode, String) {
    match err {
        SocialError::SelfOp => (StatusCode::BAD_REQUEST, "self".into()),
        SocialError::Blocked => (StatusCode::FORBIDDEN, "blocked".into()),
        SocialError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
        SocialError::Backend(e) => backend(e),
    }
}

fn user_http(err: UserError) -> (StatusCode, String) {
    match err {
        UserError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
        UserError::InvalidProfile => (StatusCode::BAD_REQUEST, "invalid profile".into()),
        UserError::Conflict => (StatusCode::CONFLICT, "conflict".into()),
        UserError::Backend(e) => backend(e),
    }
}

fn to_pb(p: UserProfile) -> PbProfile {
    PbProfile {
        account: p.account,
        nickname: p.nickname,
        avatar: p.avatar,
        bio: p.bio,
    }
}

pub async fn user_profile(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountQuery>(&body)?;
    match st.users.profile(&st.app, &req.account).await {
        Ok(Some(p)) => Ok(encode(&to_pb(p))),
        Ok(None) => Err((StatusCode::NOT_FOUND, "not found".into())),
        Err(err) => Err(user_http(err)),
    }
}

pub async fn user_update(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<ProfileUpdateReq>(&body)?;
    let patch = ProfilePatch {
        nickname: req.nickname,
        avatar: req.avatar,
        bio: req.bio,
    };
    match st.users.update_profile(&st.app, &req.account, &patch).await {
        Ok(p) => Ok(encode(&to_pb(p))),
        Err(err) => Err(user_http(err)),
    }
}

pub async fn user_profiles(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountList>(&body)?;
    let rows = st
        .users
        .profiles(&st.app, &req.accounts)
        .await
        .map_err(user_http)?;
    Ok(encode(&UserListResp {
        users: rows.into_iter().map(to_pb).collect(),
    }))
}

pub async fn user_search(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<UserSearchQuery>(&body)?;
    let limit = usize::try_from(req.limit.max(0)).unwrap_or(20);
    let rows = st
        .users
        .search(&st.app, &req.query, &req.exclude, limit)
        .await
        .map_err(user_http)?;
    Ok(encode(&UserSearchResp {
        users: rows.into_iter().map(to_pb).collect(),
    }))
}

pub async fn friend_request(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    let outcome = st
        .social
        .request(&st.app, &req.account, &req.peer)
        .await
        .map_err(social_http)?;
    Ok(encode(&AccountExists {
        exists: matches!(
            outcome,
            FriendRequestOutcome::AutoAccepted | FriendRequestOutcome::AlreadyFriends
        ),
    }))
}

pub async fn friend_accept(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    st.social
        .accept(&st.app, &req.account, &req.peer)
        .await
        .map_err(social_http)?;
    Ok(Bytes::new())
}

pub async fn friend_reject(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    st.social
        .reject(&st.app, &req.account, &req.peer)
        .await
        .map_err(social_http)?;
    Ok(Bytes::new())
}

pub async fn friend_remove(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    st.social
        .remove(&st.app, &req.account, &req.peer)
        .await
        .map_err(social_http)?;
    Ok(Bytes::new())
}

pub async fn friend_list(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountQuery>(&body)?;
    let accounts = st
        .social
        .list_friends(&st.app, &req.account)
        .await
        .map_err(social_http)?;
    Ok(encode(&AccountList { accounts }))
}

pub async fn friend_incoming(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountQuery>(&body)?;
    let accounts = st
        .social
        .incoming(&st.app, &req.account)
        .await
        .map_err(social_http)?;
    Ok(encode(&AccountList { accounts }))
}

pub async fn friend_check(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    let exists = st
        .social
        .is_friend(&st.app, &req.account, &req.peer)
        .await
        .map_err(social_http)?;
    Ok(encode(&AccountExists { exists }))
}

pub async fn block_add(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    st.social
        .block(&st.app, &req.account, &req.peer)
        .await
        .map_err(social_http)?;
    Ok(Bytes::new())
}

pub async fn block_remove(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    st.social
        .unblock(&st.app, &req.account, &req.peer)
        .await
        .map_err(social_http)?;
    Ok(Bytes::new())
}

pub async fn block_list(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountQuery>(&body)?;
    let accounts = st
        .social
        .list_blocked(&st.app, &req.account)
        .await
        .map_err(social_http)?;
    Ok(encode(&AccountList { accounts }))
}

pub async fn block_check(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<AccountPair>(&body)?;
    let exists = st
        .social
        .is_blocked_either(&st.app, &req.account, &req.peer)
        .await
        .map_err(social_http)?;
    Ok(encode(&AccountExists { exists }))
}

fn inbox_kind(kind: chat::store::MessageKind) -> i32 {
    match kind {
        chat::store::MessageKind::User => INBOX_KIND_USER,
        chat::store::MessageKind::Group => INBOX_KIND_GROUP,
    }
}

fn to_inbox_item(row: InboxEntry) -> InboxItem {
    InboxItem {
        dest: row.dest,
        kind: inbox_kind(row.kind),
        title: String::new(),
        avatar: String::new(),
        last_body: row.last_body,
        last_sender: row.last_sender,
        last_message_id: row.last_message_id,
        last_send_time: row.last_send_time,
        unread: row.unread,
    }
}

pub async fn inbox_list(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<InboxQuery>(&body)?;
    let rows = st
        .store
        .inbox(&st.app, &req.account, req.limit)
        .await
        .map_err(backend)?;
    Ok(encode(&InboxResp {
        items: rows.into_iter().map(to_inbox_item).collect(),
    }))
}

pub async fn history(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<HistoryQuery>(&body)?;
    let kind = parse_kind(req.kind).ok_or((StatusCode::BAD_REQUEST, "kind".into()))?;
    let rows = st
        .store
        .history(
            &st.app,
            &req.account,
            &req.dest,
            kind,
            req.before_id,
            req.limit,
        )
        .await
        .map_err(backend)?;
    Ok(encode(&HistoryResp {
        messages: rows
            .into_iter()
            .map(|r| HistoryItem {
                message_id: r.message_id,
                r#type: r.msg_type,
                body: r.body,
                extra: r.extra,
                sender: r.sender,
                send_time: r.send_time,
                direction: r.direction,
            })
            .collect(),
    }))
}

pub async fn inbox_read(
    State(st): State<RoyalState>,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    let req = decode::<ConversationRead>(&body)?;
    let kind = parse_kind(req.kind).ok_or((StatusCode::BAD_REQUEST, "kind".into()))?;
    st.store
        .mark_read(&st.app, &req.account, &req.dest, kind, req.message_id)
        .await
        .map_err(backend)?;
    Ok(Bytes::new())
}
