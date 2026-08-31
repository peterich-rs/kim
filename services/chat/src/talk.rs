use kim_protocol::pkt::{MessagePush, MessageReq, MessageResp, Status};
use kim_router::{Context, SessionError};
use tracing::{info, warn};

use crate::directory::{GroupDirectory, GroupError};
use crate::filter::ContentFilter;
use crate::social::SocialDirectory;
use crate::store::{InsertMessage, MessageStore};
use crate::users::UserDirectory;

#[derive(Debug, thiserror::Error)]
pub enum TalkError {
    #[error("no destination")]
    NoDestination,
    #[error("content blocked")]
    ContentBlocked,
    #[error("not a group member")]
    NotGroupMember,
    #[error("user not found")]
    UserNotFound,
    #[error("not friends")]
    NotFriends,
    #[error("blocked")]
    Blocked,
}

fn unix_nano() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    )
    .unwrap_or(i64::MAX)
}

pub async fn do_user_talk(
    ctx: Context,
    store: &dyn MessageStore,
    filter: &dyn ContentFilter,
    users: &dyn UserDirectory,
    social: &dyn SocialDirectory,
) {
    if ctx.header().dest.is_empty() {
        warn!(
            command = %ctx.header().command,
            account = %ctx.session().account,
            "no destination"
        );
        if let Err(err) = ctx
            .resp_with_error(Status::NoDestination, &TalkError::NoDestination)
            .await
        {
            warn!(%err, "resp failed");
        }
        return;
    }
    let req = match ctx.read_body::<MessageReq>() {
        Ok(r) => r,
        Err(err) => {
            warn!(%err, "invalid MessageReq");
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    if let Err(status) = filter.check(&req).await {
        let _ = ctx
            .resp_with_error(status, &TalkError::ContentBlocked)
            .await;
        return;
    }
    let receiver = ctx.header().dest.as_str();
    match users.exists(&ctx.session().app, receiver).await {
        Ok(true) => {}
        Ok(false) => {
            let _ = ctx
                .resp_with_error(Status::UserNotFound, &TalkError::UserNotFound)
                .await;
            return;
        }
        Err(err) => {
            warn!(%err, account = %receiver, "user exists failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    }
    if receiver != ctx.session().account {
        match social
            .is_blocked_either(&ctx.session().app, &ctx.session().account, receiver)
            .await
        {
            Ok(true) => {
                let _ = ctx
                    .resp_with_error(Status::Blocked, &TalkError::Blocked)
                    .await;
                return;
            }
            Ok(false) => {}
            Err(err) => {
                warn!(%err, "block check failed");
                let _ = ctx.resp_with_error(Status::SystemException, &err).await;
                return;
            }
        }
        match social
            .is_friend(&ctx.session().app, &ctx.session().account, receiver)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                let _ = ctx
                    .resp_with_error(Status::NotFriends, &TalkError::NotFriends)
                    .await;
                return;
            }
            Err(err) => {
                warn!(%err, "friend check failed");
                let _ = ctx.resp_with_error(Status::SystemException, &err).await;
                return;
            }
        }
    }
    let mut accounts = vec![receiver.to_string()];
    if receiver != ctx.session().account {
        accounts.push(ctx.session().account.clone());
    }
    let locs = match ctx.get_locations(&accounts).await {
        Ok(v) => v,
        Err(SessionError::NotFound) => Vec::new(),
        Err(err) => {
            warn!(%err, account = %receiver, "get_locations failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    };

    let send_time = unix_nano();
    let inserted = match store
        .insert_user(
            &ctx.session().app,
            &InsertMessage {
                sender: ctx.session().account.clone(),
                dest: receiver.to_string(),
                send_time,
                msg_type: req.r#type,
                body: req.body.clone(),
                extra: req.extra.clone(),
                client_id: req.client_id.clone(),
            },
        )
        .await
    {
        Ok(v) => v,
        Err(err) => {
            warn!(%err, "insert_user failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    };
    let send_time = inserted.send_time;

    let push = MessagePush {
        message_id: inserted.message_id,
        r#type: req.r#type,
        body: req.body,
        extra: req.extra,
        sender: ctx.session().account.clone(),
        send_time,
    };
    if !inserted.duplicate && !locs.is_empty() {
        if let Err(err) = ctx.dispatch(&push, &locs).await {
            warn!(%err, "dispatch user talk failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    }

    let resp = MessageResp {
        message_id: inserted.message_id,
        send_time,
    };
    info!(
        dest = %receiver,
        message_id = inserted.message_id,
        send_time,
        duplicate = inserted.duplicate,
        online = !locs.is_empty(),
        msg_type = push.r#type,
        body_len = push.body.len(),
        "user talk"
    );
    if let Err(err) = ctx.resp(Status::Success, Some(&resp)).await {
        warn!(%err, "resp failed");
    }
}

pub async fn do_group_talk(
    ctx: Context,
    store: &dyn MessageStore,
    groups: &dyn GroupDirectory,
    filter: &dyn ContentFilter,
) {
    if ctx.header().dest.is_empty() {
        warn!(
            command = %ctx.header().command,
            account = %ctx.session().account,
            "no destination"
        );
        let _ = ctx
            .resp_with_error(Status::NoDestination, &TalkError::NoDestination)
            .await;
        return;
    }
    let req = match ctx.read_body::<MessageReq>() {
        Ok(r) => r,
        Err(err) => {
            let _ = ctx.resp_with_error(Status::InvalidPacketBody, &err).await;
            return;
        }
    };
    if let Err(status) = filter.check(&req).await {
        let _ = ctx
            .resp_with_error(status, &TalkError::ContentBlocked)
            .await;
        return;
    }
    let group = ctx.header().dest.as_str();
    let send_time = unix_nano();

    let members = match groups.members(&ctx.session().app, group).await {
        Ok(m) => m,
        Err(GroupError::NotFound) => {
            let _ = ctx
                .resp_with_error(Status::NotGroupMember, &TalkError::NotGroupMember)
                .await;
            return;
        }
        Err(err) => {
            warn!(%err, "group members failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    };
    if !members.iter().any(|m| m == &ctx.session().account) {
        let _ = ctx
            .resp_with_error(Status::NotGroupMember, &TalkError::NotGroupMember)
            .await;
        return;
    }

    let inserted = match store
        .insert_group(
            &ctx.session().app,
            &InsertMessage {
                sender: ctx.session().account.clone(),
                dest: group.to_string(),
                send_time,
                msg_type: req.r#type,
                body: req.body.clone(),
                extra: req.extra.clone(),
                client_id: req.client_id.clone(),
            },
            &members,
        )
        .await
    {
        Ok(v) => v,
        Err(err) => {
            warn!(%err, "insert_group failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    };
    let send_time = inserted.send_time;

    let locs = match ctx.get_locations(&members).await {
        Ok(v) => v,
        Err(SessionError::NotFound) => Vec::new(),
        Err(err) => {
            warn!(%err, "get_locations failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    };

    let push = MessagePush {
        message_id: inserted.message_id,
        r#type: req.r#type,
        body: req.body,
        extra: req.extra,
        sender: ctx.session().account.clone(),
        send_time,
    };
    if !inserted.duplicate && !locs.is_empty() {
        if let Err(err) = ctx.dispatch(&push, &locs).await {
            warn!(%err, "dispatch group talk failed");
            let _ = ctx.resp_with_error(Status::SystemException, &err).await;
            return;
        }
    }

    info!(
        dest = %group,
        message_id = inserted.message_id,
        send_time,
        member_count = members.len(),
        loc_count = locs.len(),
        duplicate = inserted.duplicate,
        msg_type = push.r#type,
        body_len = push.body.len(),
        "group talk"
    );
    let _ = ctx
        .resp(
            Status::Success,
            Some(&MessageResp {
                message_id: inserted.message_id,
                send_time,
            }),
        )
        .await;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use bytes::Bytes;
    use kim_protocol::pkt::{Flag, MessagePush, MessageReq, MessageResp, Session, Status};
    use kim_protocol::{
        LogicPkt, CMD_CHAT_GROUP_TALK, CMD_CHAT_USER_TALK, MESSAGE_TYPE_IMAGE, MESSAGE_TYPE_TEXT,
        META_DEST_CHANNELS, META_DEST_SERVER,
    };
    use kim_router::test_support::{RecordedPush, RecordingDispatcher};
    use kim_router::{Location, Router, SessionError, SessionStorage};
    use kim_session::MemorySessionStore;

    use super::{do_group_talk, do_user_talk};
    use crate::directory::{CreateGroup, GroupDirectory, GroupError, MemoryGroupDirectory};
    use crate::filter::{ContentFilter, ImageFilter, NoopFilter, TextWordFilter};
    use crate::idgen::{IdGenerator, SequenceIdGen};
    use crate::social::{MemorySocialDirectory, SocialDirectory};
    use crate::store::{
        InsertMessage, InsertResult, MemoryMessageStore, MessageKind, MessageStore, StoreError,
    };
    use crate::users::{MemoryUserDirectory, UserDirectory};

    fn sender_session() -> Session {
        Session {
            channel_id: "ch-alice".into(),
            gate_id: "wg-1".into(),
            account: "alice".into(),
            app: "kim".into(),
            ..Session::default()
        }
    }

    fn receiver_session() -> Session {
        Session {
            channel_id: "ch-bob".into(),
            gate_id: "wg-1".into(),
            account: "bob".into(),
            app: "kim".into(),
            ..Session::default()
        }
    }

    fn sample_req() -> MessageReq {
        MessageReq {
            r#type: MESSAGE_TYPE_TEXT,
            body: "hello world".into(),
            extra: "e1".into(),
            client_id: String::new(),
        }
    }

    async fn seed_users(accounts: &[&str]) -> Arc<MemoryUserDirectory> {
        let dir = Arc::new(MemoryUserDirectory::new());
        for account in accounts {
            dir.upsert("kim", account).await.unwrap();
        }
        dir
    }

    async fn seed_friends(pairs: &[(&str, &str)]) -> Arc<MemorySocialDirectory> {
        let dir = Arc::new(MemorySocialDirectory::new());
        for (a, b) in pairs {
            dir.request("kim", a, b).await.unwrap();
            dir.accept("kim", b, a).await.unwrap();
        }
        dir
    }

    fn talk_pkt(dest: &str, body: Bytes) -> LogicPkt {
        let mut pkt = LogicPkt::new(CMD_CHAT_USER_TALK, 1, body);
        pkt.header.channel_id = "ch-alice".into();
        pkt.set_meta(META_DEST_SERVER, "wg-1");
        pkt.set_dest(dest);
        pkt
    }

    fn talk_req_pkt(dest: &str, req: &MessageReq) -> LogicPkt {
        let mut pkt = talk_pkt(dest, Bytes::new());
        pkt.write_body(req);
        pkt
    }

    fn memory_store() -> Arc<MemoryMessageStore> {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        Arc::new(MemoryMessageStore::new(idgen))
    }

    async fn serve_user_talk(
        store: Arc<dyn MessageStore>,
        cache: Arc<dyn SessionStorage>,
        dispatcher: Arc<RecordingDispatcher>,
        pkt: LogicPkt,
        session: Session,
    ) {
        serve_user_talk_users(
            store,
            cache,
            dispatcher,
            pkt,
            session,
            Arc::new(NoopFilter),
            seed_users(&["alice", "bob"]).await,
            seed_friends(&[("alice", "bob")]).await,
        )
        .await;
    }

    async fn serve_user_talk_filtered(
        store: Arc<dyn MessageStore>,
        cache: Arc<dyn SessionStorage>,
        dispatcher: Arc<RecordingDispatcher>,
        pkt: LogicPkt,
        session: Session,
        filter: Arc<dyn ContentFilter>,
    ) {
        serve_user_talk_users(
            store,
            cache,
            dispatcher,
            pkt,
            session,
            filter,
            seed_users(&["alice", "bob"]).await,
            seed_friends(&[("alice", "bob")]).await,
        )
        .await;
    }

    #[allow(clippy::too_many_arguments)]
    async fn serve_user_talk_users(
        store: Arc<dyn MessageStore>,
        cache: Arc<dyn SessionStorage>,
        dispatcher: Arc<RecordingDispatcher>,
        pkt: LogicPkt,
        session: Session,
        filter: Arc<dyn ContentFilter>,
        users: Arc<dyn UserDirectory>,
        social: Arc<dyn SocialDirectory>,
    ) {
        let mut router = Router::new();
        router.handle(CMD_CHAT_USER_TALK, move |ctx| {
            let store = store.clone();
            let filter = filter.clone();
            let users = users.clone();
            let social = social.clone();
            async move {
                do_user_talk(
                    ctx,
                    store.as_ref(),
                    filter.as_ref(),
                    users.as_ref(),
                    social.as_ref(),
                )
                .await
            }
        });
        router.serve(pkt, dispatcher, cache, session).await.unwrap();
    }

    struct FailStore;

    #[async_trait]
    impl MessageStore for FailStore {
        async fn insert_user(
            &self,
            _app: &str,
            _req: &InsertMessage,
        ) -> Result<InsertResult, StoreError> {
            Err(StoreError::Backend("insert failed".into()))
        }

        async fn insert_group(
            &self,
            _app: &str,
            _req: &InsertMessage,
            _members: &[String],
        ) -> Result<InsertResult, StoreError> {
            Err(StoreError::Backend("insert failed".into()))
        }

        async fn ack(
            &self,
            _app: &str,
            _account: &str,
            _message_id: i64,
        ) -> Result<(), StoreError> {
            Ok(())
        }

        async fn offline_index(
            &self,
            _app: &str,
            _account: &str,
            _message_id: i64,
        ) -> Result<Vec<crate::store::MessageIndexRow>, StoreError> {
            Ok(Vec::new())
        }

        async fn offline_content(
            &self,
            _app: &str,
            _account: &str,
            _message_ids: &[i64],
        ) -> Result<Vec<crate::store::MessageContentRow>, StoreError> {
            Ok(Vec::new())
        }

        async fn inbox(
            &self,
            _app: &str,
            _account: &str,
            _limit: i32,
        ) -> Result<Vec<crate::store::InboxEntry>, StoreError> {
            Ok(Vec::new())
        }

        async fn history(
            &self,
            _app: &str,
            _account: &str,
            _dest: &str,
            _kind: MessageKind,
            _before_id: i64,
            _limit: i32,
        ) -> Result<Vec<crate::store::HistoryEntry>, StoreError> {
            Ok(Vec::new())
        }

        async fn mark_read(
            &self,
            _app: &str,
            _account: &str,
            _dest: &str,
            _kind: MessageKind,
            _message_id: i64,
        ) -> Result<(), StoreError> {
            Ok(())
        }
    }

    struct OtherLocationStore;

    #[async_trait]
    impl SessionStorage for OtherLocationStore {
        async fn add(&self, _session: &Session) -> Result<(), SessionError> {
            Ok(())
        }

        async fn delete(&self, _account: &str, _channel_id: &str) -> Result<(), SessionError> {
            Ok(())
        }

        async fn get(&self, _channel_id: &str) -> Result<Session, SessionError> {
            Err(SessionError::NotFound)
        }

        async fn get_locations(&self, _accounts: &[String]) -> Result<Vec<Location>, SessionError> {
            Err(SessionError::Other("unavailable".into()))
        }

        async fn get_location(
            &self,
            _account: &str,
            _device: &str,
        ) -> Result<Location, SessionError> {
            Err(SessionError::Other("unavailable".into()))
        }
    }

    #[tokio::test]
    async fn empty_dest_is_no_destination_without_insert_or_push() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_user_talk(
            store.clone(),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            talk_req_pkt("", &sample_req()),
            sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::NoDestination as i32);
        assert!(store.recorded().is_empty());
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn unknown_dest_is_user_not_found_without_insert_or_push() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_user_talk_users(
            store.clone(),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            talk_req_pkt("carol", &sample_req()),
            sender_session(),
            Arc::new(NoopFilter),
            seed_users(&["alice"]).await,
            seed_friends(&[]).await,
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::UserNotFound as i32);
        assert!(store.recorded().is_empty());
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn same_client_id_does_not_insert_or_push_twice() {
        let store = memory_store();
        let cache = Arc::new(MemorySessionStore::new());
        cache.add(&receiver_session()).await.unwrap();
        let mut req = sample_req();
        req.client_id = "c1".into();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_user_talk(
            store.clone(),
            cache.clone(),
            dispatcher.clone(),
            talk_req_pkt("bob", &req),
            sender_session(),
        )
        .await;
        serve_user_talk(
            store.clone(),
            cache,
            dispatcher.clone(),
            talk_req_pkt("bob", &req),
            sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let pushes: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Push as i32)
            .collect();
        assert_eq!(pushes.len(), 1);
        assert_eq!(store.recorded().len(), 1);
        let resps: Vec<_> = got
            .iter()
            .filter(|p| {
                p.pkt.header.flag == Flag::Response as i32
                    && p.pkt.header.status == Status::Success as i32
            })
            .collect();
        assert_eq!(resps.len(), 2);
        let a: MessageResp = resps[0].pkt.read_body().unwrap();
        let b: MessageResp = resps[1].pkt.read_body().unwrap();
        assert_eq!(a.message_id, b.message_id);
        assert_eq!(a.send_time, b.send_time);
    }

    #[tokio::test]
    async fn dest_offline_succeeds_and_inserts_without_push() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_user_talk(
            store.clone(),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            talk_req_pkt("bob", &sample_req()),
            sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::Success as i32);
        let resp: MessageResp = resps[0].pkt.read_body().unwrap();
        assert!(resp.message_id > 10_000);
        assert!(resp.send_time > 1000);
        assert_eq!(store.recorded().len(), 1);
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn dest_online_dispatches_one_push_to_receiver_channel() {
        let store = memory_store();
        let cache = Arc::new(MemorySessionStore::new());
        cache.add(&receiver_session()).await.unwrap();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let req = sample_req();
        serve_user_talk(
            store.clone(),
            cache,
            dispatcher.clone(),
            talk_req_pkt("bob", &req),
            sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let pushes: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Push as i32)
            .collect();
        assert_eq!(pushes.len(), 1);
        assert_eq!(pushes[0].pkt.header.command, CMD_CHAT_USER_TALK);
        assert_eq!(pushes[0].pkt.header.flag, Flag::Push as i32);
        assert_eq!(pushes[0].channels, vec!["ch-bob".to_string()]);
        assert_eq!(pushes[0].pkt.get_meta(META_DEST_CHANNELS), Some("ch-bob"));
        assert!(!pushes[0].channels.iter().any(|c| c == "ch-alice"));

        let resps: Vec<_> = got
            .iter()
            .filter(|p| {
                p.pkt.header.flag == Flag::Response as i32
                    && p.pkt.header.status == Status::Success as i32
            })
            .collect();
        assert_eq!(resps.len(), 1);
        let resp: MessageResp = resps[0].pkt.read_body().unwrap();
        let push: MessagePush = pushes[0].pkt.read_body().unwrap();
        assert_eq!(push.message_id, resp.message_id);
        assert_eq!(push.send_time, resp.send_time);
        assert_eq!(push.body, req.body);
        assert_eq!(push.r#type, req.r#type);
        assert_eq!(push.extra, req.extra);
        assert_eq!(push.sender, "alice");
    }

    #[tokio::test]
    async fn insert_fail_is_system_exception_without_push() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_user_talk(
            Arc::new(FailStore),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            talk_req_pkt("bob", &sample_req()),
            sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::SystemException as i32);
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn illegal_protobuf_body_is_invalid_packet_body() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_user_talk(
            store.clone(),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            talk_pkt("bob", Bytes::from_static(&[0xff, 0x00, 0xab])),
            sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::InvalidPacketBody as i32);
        assert!(store.recorded().is_empty());
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn dispatch_fail_is_system_exception_without_success_resp() {
        let store = memory_store();
        let cache = Arc::new(MemorySessionStore::new());
        let mut bob = receiver_session();
        bob.gate_id = "wg-2".into();
        cache.add(&bob).await.unwrap();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        dispatcher.fail_on("wg-2");
        serve_user_talk(
            store,
            cache,
            dispatcher.clone(),
            talk_req_pkt("bob", &sample_req()),
            sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        assert!(!got.iter().any(|p| {
            p.pkt.header.flag == Flag::Response as i32
                && p.pkt.header.status == Status::Success as i32
        }));
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::SystemException as i32);
    }

    #[tokio::test]
    async fn get_location_other_is_system_exception_without_insert_or_push() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_user_talk(
            store.clone(),
            Arc::new(OtherLocationStore),
            dispatcher.clone(),
            talk_req_pkt("bob", &sample_req()),
            sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::SystemException as i32);
        assert!(store.recorded().is_empty());
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    fn group_sender_session() -> Session {
        Session {
            channel_id: "ch-self".into(),
            gate_id: "wg-1".into(),
            account: "alice".into(),
            app: "kim".into(),
            ..Session::default()
        }
    }

    fn member_session(account: &str, channel: &str, gate: &str) -> Session {
        Session {
            channel_id: channel.into(),
            gate_id: gate.into(),
            account: account.into(),
            app: "kim".into(),
            ..Session::default()
        }
    }

    fn group_talk_pkt(dest: &str, body: Bytes) -> LogicPkt {
        let mut pkt = LogicPkt::new(CMD_CHAT_GROUP_TALK, 1, body);
        pkt.header.channel_id = "ch-self".into();
        pkt.set_meta(META_DEST_SERVER, "wg-1");
        pkt.set_dest(dest);
        pkt
    }

    fn group_talk_req_pkt(dest: &str, req: &MessageReq) -> LogicPkt {
        let mut pkt = group_talk_pkt(dest, Bytes::new());
        pkt.write_body(req);
        pkt
    }

    fn memory_groups() -> Arc<MemoryGroupDirectory> {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        Arc::new(MemoryGroupDirectory::new(idgen))
    }

    async fn serve_group_talk(
        store: Arc<dyn MessageStore>,
        groups: Arc<dyn GroupDirectory>,
        cache: Arc<dyn SessionStorage>,
        dispatcher: Arc<RecordingDispatcher>,
        pkt: LogicPkt,
        session: Session,
    ) {
        serve_group_talk_filtered(
            store,
            groups,
            cache,
            dispatcher,
            pkt,
            session,
            Arc::new(NoopFilter),
        )
        .await;
    }

    async fn serve_group_talk_filtered(
        store: Arc<dyn MessageStore>,
        groups: Arc<dyn GroupDirectory>,
        cache: Arc<dyn SessionStorage>,
        dispatcher: Arc<RecordingDispatcher>,
        pkt: LogicPkt,
        session: Session,
        filter: Arc<dyn ContentFilter>,
    ) {
        let mut router = Router::new();
        router.handle(CMD_CHAT_GROUP_TALK, move |ctx| {
            let store = store.clone();
            let groups = groups.clone();
            let filter = filter.clone();
            async move {
                do_group_talk(ctx, store.as_ref(), groups.as_ref(), filter.as_ref()).await
            }
        });
        router.serve(pkt, dispatcher, cache, session).await.unwrap();
    }

    struct FailGroups;

    #[async_trait]
    impl GroupDirectory for FailGroups {
        async fn create(&self, _app: &str, _req: &CreateGroup) -> Result<String, GroupError> {
            Err(GroupError::Backend("create failed".into()))
        }

        async fn members(&self, _app: &str, _group_id: &str) -> Result<Vec<String>, GroupError> {
            Err(GroupError::Backend("members failed".into()))
        }

        async fn join(
            &self,
            _app: &str,
            _group_id: &str,
            _account: &str,
        ) -> Result<(), GroupError> {
            Err(GroupError::Backend("join failed".into()))
        }

        async fn quit(
            &self,
            _app: &str,
            _group_id: &str,
            _account: &str,
        ) -> Result<(), GroupError> {
            Err(GroupError::Backend("quit failed".into()))
        }

        async fn detail(
            &self,
            _app: &str,
            _group_id: &str,
        ) -> Result<crate::directory::GroupInfo, GroupError> {
            Err(GroupError::Backend("detail failed".into()))
        }
    }

    struct OtherLocationsStore;

    #[async_trait]
    impl SessionStorage for OtherLocationsStore {
        async fn add(&self, _session: &Session) -> Result<(), SessionError> {
            Ok(())
        }

        async fn delete(&self, _account: &str, _channel_id: &str) -> Result<(), SessionError> {
            Ok(())
        }

        async fn get(&self, _channel_id: &str) -> Result<Session, SessionError> {
            Err(SessionError::NotFound)
        }

        async fn get_locations(&self, _accounts: &[String]) -> Result<Vec<Location>, SessionError> {
            Err(SessionError::Other("unavailable".into()))
        }

        async fn get_location(
            &self,
            _account: &str,
            _device: &str,
        ) -> Result<Location, SessionError> {
            Err(SessionError::NotFound)
        }
    }

    fn success_resps(got: &[RecordedPush]) -> Vec<&RecordedPush> {
        got.iter()
            .filter(|p| {
                p.pkt.header.flag == Flag::Response as i32
                    && p.pkt.header.status == Status::Success as i32
            })
            .collect()
    }

    fn push_pkts(got: &[RecordedPush]) -> Vec<&RecordedPush> {
        got.iter()
            .filter(|p| p.pkt.header.flag == Flag::Push as i32)
            .collect()
    }

    #[tokio::test]
    async fn group_empty_dest_is_no_destination_without_insert() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_talk(
            store.clone(),
            memory_groups(),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            group_talk_req_pkt("", &sample_req()),
            group_sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::NoDestination as i32);
        assert!(store.recorded().is_empty());
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn group_two_gates_coalesce_skip_sender_and_omit_offline() {
        let store = memory_store();
        let groups = memory_groups();
        groups.seed(
            "kim",
            "g1",
            vec![
                "alice".into(),
                "bob".into(),
                "carol".into(),
                "dave".into(),
                "eve".into(),
            ],
        );
        let cache = Arc::new(MemorySessionStore::new());
        cache
            .add(&member_session("alice", "ch-self", "wg-1"))
            .await
            .unwrap();
        cache
            .add(&member_session("bob", "ch-a", "wg-1"))
            .await
            .unwrap();
        cache
            .add(&member_session("carol", "ch-b", "wg-1"))
            .await
            .unwrap();
        cache
            .add(&member_session("dave", "ch-c", "wg-2"))
            .await
            .unwrap();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_talk(
            store.clone(),
            groups,
            cache,
            dispatcher.clone(),
            group_talk_req_pkt("g1", &sample_req()),
            group_sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let pushes = push_pkts(&got);
        assert_eq!(pushes.len(), 2);
        assert_eq!(pushes[0].gateway, "wg-1");
        assert_eq!(
            pushes[0].channels,
            vec!["ch-a".to_string(), "ch-b".to_string()]
        );
        assert_eq!(
            pushes[0].pkt.get_meta(META_DEST_CHANNELS),
            Some("ch-a,ch-b")
        );
        assert_eq!(pushes[1].gateway, "wg-2");
        assert_eq!(pushes[1].channels, vec!["ch-c".to_string()]);
        assert_eq!(pushes[1].pkt.get_meta(META_DEST_CHANNELS), Some("ch-c"));
        for p in &pushes {
            assert!(!p.channels.iter().any(|c| c == "ch-self"));
            assert_eq!(p.pkt.header.command, CMD_CHAT_GROUP_TALK);
        }
        let resps = success_resps(&got);
        assert_eq!(resps.len(), 1);
        let resp: MessageResp = resps[0].pkt.read_body().unwrap();
        assert!(resp.message_id > 10_000);
        assert_eq!(store.recorded().len(), 1);
        assert_eq!(store.recorded()[0].kind, MessageKind::Group);
    }

    #[tokio::test]
    async fn group_offline_member_is_omitted_from_push() {
        let store = memory_store();
        let groups = memory_groups();
        groups.seed(
            "kim",
            "g1",
            vec!["alice".into(), "bob".into(), "eve".into()],
        );
        let cache = Arc::new(MemorySessionStore::new());
        cache
            .add(&member_session("alice", "ch-self", "wg-1"))
            .await
            .unwrap();
        cache
            .add(&member_session("bob", "ch-a", "wg-1"))
            .await
            .unwrap();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_talk(
            store,
            groups,
            cache,
            dispatcher.clone(),
            group_talk_req_pkt("g1", &sample_req()),
            group_sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let pushes = push_pkts(&got);
        assert_eq!(pushes.len(), 1);
        assert_eq!(pushes[0].channels, vec!["ch-a".to_string()]);
        assert!(!pushes[0].channels.iter().any(|c| c == "ch-self"));
    }

    #[tokio::test]
    async fn group_all_offline_succeeds_with_insert_without_push() {
        let store = memory_store();
        let groups = memory_groups();
        groups.seed("kim", "g1", vec!["alice".into(), "bob".into()]);
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_talk(
            store.clone(),
            groups,
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            group_talk_req_pkt("g1", &sample_req()),
            group_sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps = success_resps(&got);
        assert_eq!(resps.len(), 1);
        let resp: MessageResp = resps[0].pkt.read_body().unwrap();
        assert!(resp.message_id > 10_000);
        assert_eq!(store.recorded().len(), 1);
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn group_unknown_is_not_group_member_without_insert() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_talk(
            store.clone(),
            memory_groups(),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            group_talk_req_pkt("no-such", &sample_req()),
            group_sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::NotGroupMember as i32);
        assert!(store.recorded().is_empty());
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn group_insert_fail_is_system_exception_without_dispatch() {
        let groups = memory_groups();
        groups.seed("kim", "g1", vec!["alice".into()]);
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_talk(
            Arc::new(FailStore),
            groups,
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            group_talk_req_pkt("g1", &sample_req()),
            group_sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::SystemException as i32);
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn group_get_locations_other_is_system_exception_after_insert() {
        let store = memory_store();
        let groups = memory_groups();
        groups.seed("kim", "g1", vec!["alice".into(), "bob".into()]);
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_talk(
            store.clone(),
            groups,
            Arc::new(OtherLocationsStore),
            dispatcher.clone(),
            group_talk_req_pkt("g1", &sample_req()),
            group_sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::SystemException as i32);
        assert_eq!(store.recorded().len(), 1);
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn group_members_err_is_system_exception() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_talk(
            store.clone(),
            Arc::new(FailGroups),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            group_talk_req_pkt("g1", &sample_req()),
            group_sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::SystemException as i32);
        assert!(store.recorded().is_empty());
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn group_sender_not_in_members_is_rejected_without_insert() {
        let store = memory_store();
        let groups = memory_groups();
        groups.seed("kim", "g1", vec!["bob".into()]);
        let cache = Arc::new(MemorySessionStore::new());
        cache
            .add(&member_session("bob", "ch-a", "wg-1"))
            .await
            .unwrap();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        serve_group_talk(
            store.clone(),
            groups,
            cache,
            dispatcher.clone(),
            group_talk_req_pkt("g1", &sample_req()),
            group_sender_session(),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::NotGroupMember as i32);
        assert!(store.recorded().is_empty());
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn user_text_filter_blocks_without_insert() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut req = sample_req();
        req.body = "say badword now".into();
        serve_user_talk_filtered(
            store.clone(),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            talk_req_pkt("bob", &req),
            sender_session(),
            Arc::new(TextWordFilter::new(["badword"])),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::ContentBlocked as i32);
        assert!(store.recorded().is_empty());
        assert!(!got.iter().any(|p| p.pkt.header.flag == Flag::Push as i32));
    }

    #[tokio::test]
    async fn user_image_with_text_word_still_inserts() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let req = MessageReq {
            r#type: MESSAGE_TYPE_IMAGE,
            body: "http://cdn/badword.png".into(),
            extra: String::new(),
            client_id: String::new(),
        };
        serve_user_talk_filtered(
            store.clone(),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            talk_req_pkt("bob", &req),
            sender_session(),
            Arc::new(TextWordFilter::new(["badword"])),
        )
        .await;

        let got = dispatcher.recorded();
        assert_eq!(success_resps(&got).len(), 1);
        assert_eq!(store.recorded().len(), 1);
    }

    #[tokio::test]
    async fn user_image_filter_blocks_url() {
        let store = memory_store();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let req = MessageReq {
            r#type: MESSAGE_TYPE_IMAGE,
            body: "http://evil.example/a.png".into(),
            extra: String::new(),
            client_id: String::new(),
        };
        serve_user_talk_filtered(
            store.clone(),
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            talk_req_pkt("bob", &req),
            sender_session(),
            Arc::new(ImageFilter::new(["evil.example"])),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::ContentBlocked as i32);
        assert!(store.recorded().is_empty());
    }

    #[tokio::test]
    async fn group_text_filter_runs_before_member_check() {
        let store = memory_store();
        let groups = memory_groups();
        groups.seed("kim", "g1", vec!["alice".into()]);
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut req = sample_req();
        req.body = "badword".into();
        serve_group_talk_filtered(
            store.clone(),
            groups,
            Arc::new(MemorySessionStore::new()),
            dispatcher.clone(),
            group_talk_req_pkt("g1", &req),
            group_sender_session(),
            Arc::new(TextWordFilter::new(["badword"])),
        )
        .await;

        let got = dispatcher.recorded();
        let resps: Vec<_> = got
            .iter()
            .filter(|p| p.pkt.header.flag == Flag::Response as i32)
            .collect();
        assert_eq!(resps.len(), 1);
        assert_eq!(resps[0].pkt.header.status, Status::ContentBlocked as i32);
        assert!(store.recorded().is_empty());
    }
}
