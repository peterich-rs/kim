use kim_protocol::pkt::{
    Message as PktMessage, MessageContentReq, MessageContentResp, MessageIndex, MessageIndexReq,
    MessageIndexResp, Status,
};
use kim_router::Context;
use tracing::{info, warn};

use crate::store::{MessageStore, MESSAGE_MAX_COUNT_PER_PAGE};

#[derive(Debug, thiserror::Error)]
enum OfflineError {
    #[error("too many message ids")]
    TooManyIds,
}

pub async fn do_offline_index(ctx: Context, store: &dyn MessageStore, pending_receipt: bool) {
    let req = match ctx.read_body::<MessageIndexReq>() {
        Ok(r) => r,
        Err(err) => {
            warn!(%err, "invalid MessageIndexReq");
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    let session = ctx.session();
    let (rows, has_more) = if pending_receipt {
        if session.jti.trim().is_empty() {
            (Vec::new(), false)
        } else {
            match store
                .offline_index(
                    &session.app,
                    &session.account,
                    session.jti.trim(),
                    req.message_id,
                    req.resume,
                )
                .await
            {
                Ok(v) => v,
                Err(err) => {
                    warn!(%err, "offline index failed");
                    let _ = ctx.resp_with_error(Status::SystemException, &err).await;
                    return;
                }
            }
        }
    } else {
        match store
            .offline_index(&session.app, &session.account, "", req.message_id, false)
            .await
        {
            Ok(v) => v,
            Err(err) => {
                warn!(%err, "offline index failed");
                let _ = ctx.resp_with_error(Status::SystemException, &err).await;
                return;
            }
        }
    };
    let resp = MessageIndexResp {
        indexes: rows
            .into_iter()
            .map(|r| MessageIndex {
                message_id: r.message_id,
                direction: r.direction,
                send_time: r.send_time,
                account_b: r.account_b,
                group: r.group,
            })
            .collect(),
        has_more,
    };
    info!(
        account = %ctx.session().account,
        count = resp.indexes.len(),
        "offline index"
    );
    if let Err(err) = ctx.resp(Status::Success, Some(&resp)).await {
        warn!(%err, "resp failed");
    }
}

pub async fn do_offline_content(ctx: Context, store: &dyn MessageStore) {
    let req = match ctx.read_body::<MessageContentReq>() {
        Ok(r) => r,
        Err(err) => {
            warn!(%err, "invalid MessageContentReq");
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    if req.message_ids.len() > MESSAGE_MAX_COUNT_PER_PAGE {
        warn!(count = req.message_ids.len(), "too many message ids");
        let _ = ctx
            .resp_with_error(Status::InvalidPacketBody, &OfflineError::TooManyIds)
            .await;
        return;
    }
    let rows = match store
        .offline_content(&ctx.session().app, &ctx.session().account, &req.message_ids)
        .await
    {
        Ok(v) => v,
        Err(err) => {
            warn!(%err, "offline content failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    };
    let resp = MessageContentResp {
        messages: rows
            .into_iter()
            .map(|r| PktMessage {
                message_id: r.message_id,
                r#type: r.msg_type,
                body: r.body,
                extra: r.extra,
            })
            .collect(),
    };
    info!(
        account = %ctx.session().account,
        count = resp.messages.len(),
        "offline content"
    );
    if let Err(err) = ctx.resp(Status::Success, Some(&resp)).await {
        warn!(%err, "resp failed");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use kim_protocol::pkt::{Flag, MessageContentReq, MessageIndexReq, Session, Status};
    use kim_protocol::{LogicPkt, CMD_OFFLINE_CONTENT, CMD_OFFLINE_INDEX, META_DEST_SERVER};
    use kim_router::test_support::RecordingDispatcher;
    use kim_router::Router;
    use kim_session::MemorySessionStore;

    use super::do_offline_content;
    use crate::idgen::{IdGenerator, SequenceIdGen};
    use crate::store::{MemoryMessageStore, MESSAGE_MAX_COUNT_PER_PAGE};

    fn session() -> Session {
        Session {
            channel_id: "ch-bob".into(),
            gate_id: "wg-1".into(),
            account: "bob".into(),
            app: "kim".into(),
            ..Session::default()
        }
    }

    #[tokio::test]
    async fn content_over_page_is_invalid_packet_body_without_store() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = Arc::new(MemoryMessageStore::new(idgen));
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut router = Router::new();
        router.handle(CMD_OFFLINE_CONTENT, {
            let store = store.clone();
            move |ctx| {
                let store = store.clone();
                async move { do_offline_content(ctx, store.as_ref()).await }
            }
        });
        let ids = vec![1i64; MESSAGE_MAX_COUNT_PER_PAGE + 1];
        let mut pkt = LogicPkt::new(CMD_OFFLINE_CONTENT, 1, Bytes::new());
        pkt.header.channel_id = "ch-bob".into();
        pkt.set_meta(META_DEST_SERVER, "wg-1");
        pkt.write_body(&MessageContentReq {
            message_ids: ids,
            ..Default::default()
        });
        router
            .serve(
                pkt,
                dispatcher.clone(),
                Arc::new(MemorySessionStore::new()),
                session(),
            )
            .await
            .unwrap();
        assert_eq!(
            dispatcher.recorded()[0].pkt.header.status,
            Status::InvalidPacketBody as i32
        );
        assert_eq!(
            dispatcher.recorded()[0].pkt.header.flag,
            Flag::Response as i32
        );
        assert!(store.recorded().is_empty());
    }

    #[tokio::test]
    async fn index_bad_body_is_invalid_packet_body() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = Arc::new(MemoryMessageStore::new(idgen));
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut router = Router::new();
        router.handle(CMD_OFFLINE_INDEX, {
            let store = store.clone();
            move |ctx| {
                let store = store.clone();
                async move { super::do_offline_index(ctx, store.as_ref(), false).await }
            }
        });
        let mut pkt = LogicPkt::new(CMD_OFFLINE_INDEX, 1, Bytes::from_static(&[0xff]));
        pkt.header.channel_id = "ch-bob".into();
        pkt.set_meta(META_DEST_SERVER, "wg-1");
        let _ = MessageIndexReq {
            message_id: 0,
            ..Default::default()
        };
        router
            .serve(
                pkt,
                dispatcher.clone(),
                Arc::new(MemorySessionStore::new()),
                session(),
            )
            .await
            .unwrap();
        assert_eq!(
            dispatcher.recorded()[0].pkt.header.status,
            Status::InvalidPacketBody as i32
        );
    }
}
