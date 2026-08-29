use kim_protocol::pkt::{MessageAckReq, Status};
use kim_router::Context;
use tracing::{info, warn};

use crate::store::MessageStore;

pub async fn do_talk_ack(ctx: Context, store: &dyn MessageStore) {
    let req = match ctx.read_body::<MessageAckReq>() {
        Ok(r) => r,
        Err(err) => {
            warn!(%err, "invalid MessageAckReq");
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    if let Err(err) = store
        .ack(&ctx.session().app, &ctx.session().account, req.message_id)
        .await
    {
        warn!(%err, "ack failed");
        let _ = ctx.resp_with_error(Status::SystemException, &err).await;
        return;
    }
    info!(
        account = %ctx.session().account,
        message_id = req.message_id,
        "talk ack"
    );
    if let Err(err) = ctx.resp_bytes(Status::Success, bytes::Bytes::new()).await {
        warn!(%err, "resp failed");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use kim_protocol::pkt::{Flag, MessageAckReq, Session, Status};
    use kim_protocol::{LogicPkt, CMD_CHAT_TALK_ACK, META_DEST_SERVER};
    use kim_router::test_support::RecordingDispatcher;
    use kim_router::Router;
    use kim_session::MemorySessionStore;

    use super::do_talk_ack;
    use crate::idgen::{IdGenerator, SequenceIdGen};
    use crate::store::{InsertMessage, MemoryMessageStore, MessageStore};

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
    async fn ack_success_empty_body() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = Arc::new(MemoryMessageStore::new(idgen));
        store
            .insert_user(
                "kim",
                &InsertMessage {
                    sender: "alice".into(),
                    dest: "bob".into(),
                    send_time: crate::store::now_unix_nano(),
                    msg_type: 1,
                    body: "hi".into(),
                    extra: String::new(),
                },
            )
            .await
            .unwrap();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut router = Router::new();
        router.handle(CMD_CHAT_TALK_ACK, {
            let store = store.clone();
            move |ctx| {
                let store = store.clone();
                async move { do_talk_ack(ctx, store.as_ref()).await }
            }
        });
        let mut pkt = LogicPkt::new(CMD_CHAT_TALK_ACK, 1, Bytes::new());
        pkt.header.channel_id = "ch-bob".into();
        pkt.set_meta(META_DEST_SERVER, "wg-1");
        pkt.write_body(&MessageAckReq { message_id: 1 });
        router
            .serve(
                pkt,
                dispatcher.clone(),
                Arc::new(MemorySessionStore::new()),
                session(),
            )
            .await
            .unwrap();
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].pkt.header.status, Status::Success as i32);
        assert_eq!(got[0].pkt.header.flag, Flag::Response as i32);
        assert!(got[0].pkt.body.is_empty());
    }

    #[tokio::test]
    async fn bad_body_is_invalid_packet_body() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = Arc::new(MemoryMessageStore::new(idgen));
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut router = Router::new();
        router.handle(CMD_CHAT_TALK_ACK, {
            let store = store.clone();
            move |ctx| {
                let store = store.clone();
                async move { do_talk_ack(ctx, store.as_ref()).await }
            }
        });
        let mut pkt = LogicPkt::new(CMD_CHAT_TALK_ACK, 1, Bytes::from_static(&[0xff]));
        pkt.header.channel_id = "ch-bob".into();
        pkt.set_meta(META_DEST_SERVER, "wg-1");
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
