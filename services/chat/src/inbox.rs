use kim_protocol::pkt::{
    ConversationReadReq, HistoryItem, HistoryReq, HistoryResp, InboxItem, InboxReq, InboxResp,
    Status,
};
use kim_protocol::{INBOX_KIND_GROUP, INBOX_KIND_USER};
use kim_router::Context;
use tracing::warn;

use crate::directory::GroupDirectory;
use crate::store::{MessageKind, MessageStore};
use crate::users::UserDirectory;

fn kind_of(kind: i32) -> Result<MessageKind, ()> {
    match kind {
        INBOX_KIND_USER => Ok(MessageKind::User),
        INBOX_KIND_GROUP => Ok(MessageKind::Group),
        _ => Err(()),
    }
}

pub fn parse_kind(kind: i32) -> Option<MessageKind> {
    kind_of(kind).ok()
}

pub async fn do_inbox_list(
    ctx: Context,
    store: &dyn MessageStore,
    users: &dyn UserDirectory,
    groups: &dyn GroupDirectory,
) {
    let limit = match ctx.read_body::<InboxReq>() {
        Ok(r) => r.limit,
        Err(_) => 0,
    };
    let rows = match store
        .inbox(&ctx.session().app, &ctx.session().account, limit)
        .await
    {
        Ok(v) => v,
        Err(err) => {
            warn!(%err, "inbox failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    };
    let user_dests: Vec<String> = rows
        .iter()
        .filter(|r| r.kind == MessageKind::User)
        .map(|r| r.dest.clone())
        .collect();
    let profiles = match users.profiles(&ctx.session().app, &user_dests).await {
        Ok(v) => v,
        Err(err) => {
            warn!(%err, "inbox profiles failed");
            Vec::new()
        }
    };
    let mut by_account = std::collections::HashMap::new();
    for p in profiles {
        by_account.insert(p.account.clone(), p);
    }
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let (title, avatar) = match row.kind {
            MessageKind::User => match by_account.get(&row.dest) {
                Some(p) => (p.display_name().to_string(), p.avatar.clone()),
                None => (row.dest.clone(), String::new()),
            },
            MessageKind::Group => match groups.detail(&ctx.session().app, &row.dest).await {
                Ok(g) => (g.name, g.avatar),
                Err(err) => {
                    warn!(%err, dest = %row.dest, "inbox group failed");
                    (row.dest.clone(), String::new())
                }
            },
        };
        items.push(InboxItem {
            dest: row.dest,
            kind: match row.kind {
                MessageKind::User => INBOX_KIND_USER,
                MessageKind::Group => INBOX_KIND_GROUP,
            },
            title,
            avatar,
            last_body: row.last_body,
            last_sender: row.last_sender,
            last_message_id: row.last_message_id,
            last_send_time: row.last_send_time,
            unread: row.unread,
        });
    }
    let _ = ctx.resp(Status::Success, Some(&InboxResp { items })).await;
}

pub async fn do_history(ctx: Context, store: &dyn MessageStore) {
    if ctx.header().dest.is_empty() {
        let _ = ctx
            .resp_bytes(Status::NoDestination, bytes::Bytes::new())
            .await;
        return;
    }
    let req = match ctx.read_body::<HistoryReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let Some(kind) = parse_kind(req.kind) else {
        let _ = ctx
            .resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await;
        return;
    };
    match store
        .history(
            &ctx.session().app,
            &ctx.session().account,
            &ctx.header().dest,
            kind,
            req.before_id,
            req.limit,
        )
        .await
    {
        Ok(rows) => {
            let messages = rows
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
                .collect();
            let _ = ctx
                .resp(Status::Success, Some(&HistoryResp { messages }))
                .await;
        }
        Err(err) => {
            warn!(%err, "history failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        }
    }
}

pub async fn do_inbox_read(ctx: Context, store: &dyn MessageStore) {
    if ctx.header().dest.is_empty() {
        let _ = ctx
            .resp_bytes(Status::NoDestination, bytes::Bytes::new())
            .await;
        return;
    }
    let req = match ctx.read_body::<ConversationReadReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let Some(kind) = parse_kind(req.kind) else {
        let _ = ctx
            .resp_bytes(Status::InvalidPacketBody, bytes::Bytes::new())
            .await;
        return;
    };
    match store
        .mark_read(
            &ctx.session().app,
            &ctx.session().account,
            &ctx.header().dest,
            kind,
            req.message_id,
        )
        .await
    {
        Ok(()) => {
            let _ = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await;
        }
        Err(err) => {
            warn!(%err, "mark read failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        }
    }
}
