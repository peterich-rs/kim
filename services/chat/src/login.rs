use bytes::Bytes;
use kim_protocol::pkt::{KickoutNotify, LoginResp, Status};
use kim_protocol::{AccountId, ChannelId};
use kim_router::{Context, RouterError, SessionError};
use kim_session::exclusive_device;
use tracing::{error, info, warn};

use crate::presence::PresenceHub;
use crate::store::MessageStore;
use crate::users::UserDirectory;

pub async fn do_sys_login(ctx: Context, users: &dyn UserDirectory) -> Result<(), RouterError> {
    do_sys_login_with_zone(ctx, "", users, None, false, None).await
}

pub async fn do_sys_login_with_zone(
    ctx: Context,
    zone: &str,
    users: &dyn UserDirectory,
    store: Option<&dyn MessageStore>,
    pending_receipt: bool,
    presence: Option<&PresenceHub>,
) -> Result<(), RouterError> {
    let body = match ctx.read_body::<kim_protocol::pkt::Session>() {
        Ok(s) if !s.account.is_empty() => s,
        Ok(_) | Err(_) => {
            ctx.resp_bytes(Status::InvalidPacketBody, Bytes::new())
                .await?;
            return Ok(());
        }
    };
    let mut body = body;
    if !zone.is_empty() {
        body.zone = zone.to_string();
    }
    info!(account = %body.account, channel = %body.channel_id, zone = %body.zone, "do login");
    match users.lookup(&body.app, &body.account).await {
        Ok(Some(p)) if p.kind == kim_protocol::PROFILE_KIND_BOT => {
            ctx.resp_bytes(Status::Unauthorized, Bytes::new()).await?;
            return Ok(());
        }
        Ok(_) => {}
        Err(err) => {
            error!(%err, "user lookup failed");
            ctx.resp_bytes(Status::SystemException, Bytes::new())
                .await?;
            return Ok(());
        }
    }
    if let Err(err) = users.upsert(&body.app, &body.account).await {
        error!(%err, "user upsert failed");
        ctx.resp_bytes(Status::SystemException, Bytes::new())
            .await?;
        return Ok(());
    }
    let account = AccountId::from_trusted(&body.account);
    let channel = ChannelId::from_trusted(&body.channel_id);
    let existing = match ctx.list_locations(&account).await {
        Ok(v) => v,
        Err(SessionError::NotFound) => Vec::new(),
        Err(err) => {
            warn!(%err, "list_locations failed");
            ctx.resp_bytes(Status::SystemException, Bytes::new())
                .await?;
            return Ok(());
        }
    };
    if exclusive_device(&body.device) {
        let victims: Vec<_> = existing
            .into_iter()
            .filter(|loc| loc.channel_id != channel && exclusive_device(&loc.device))
            .collect();
        for old in &victims {
            info!(
                old_channel = %old.channel_id,
                new_channel = %body.channel_id,
                device = %body.device,
                "kickout mobile"
            );
            let notify = KickoutNotify {
                channel_id: old.channel_id.as_str().to_owned(),
            };
            if let Err(err) = ctx.dispatch(&notify, std::slice::from_ref(old)).await {
                warn!(%err, "dispatch kickout failed");
            }
        }
    }
    if let Err(err) = ctx.add(&body).await {
        error!(%err, "session add failed");
        ctx.resp_bytes(Status::SystemException, Bytes::new())
            .await?;
        return Ok(());
    }
    if pending_receipt {
        let jti = body.jti.trim();
        if jti.is_empty() {
            error!("pending receipt login missing jti");
            let _ = ctx.delete(&account, &channel).await;
            ctx.resp_bytes(Status::SystemException, Bytes::new())
                .await?;
            return Ok(());
        }
        let Some(store) = store else {
            error!("pending receipt login missing store");
            let _ = ctx.delete(&account, &channel).await;
            ctx.resp_bytes(Status::SystemException, Bytes::new())
                .await?;
            return Ok(());
        };
        if let Err(err) = store.backfill_delivery(&body.app, &body.account, jti).await {
            error!(%err, "backfill failed");
            let _ = ctx.delete(&account, &channel).await;
            ctx.resp_bytes(Status::SystemException, Bytes::new())
                .await?;
            return Ok(());
        }
    }
    if let Some(hub) = presence {
        hub.on_location_added(&body.app, &body.account).await;
    }
    let resp = LoginResp {
        channel_id: body.channel_id.clone(),
    };
    ctx.resp(Status::Success, Some(&resp)).await?;
    Ok(())
}

pub async fn do_sys_logout(
    ctx: Context,
    presence: Option<&PresenceHub>,
) -> Result<(), RouterError> {
    let account = AccountId::from_trusted(&ctx.session().account);
    let channel_id = ChannelId::from_trusted(&ctx.session().channel_id);
    let app = ctx.session().app.clone();
    info!(account = %account, channel = %channel_id, "do logout");
    match ctx.delete(&account, &channel_id).await {
        Ok(()) => {
            if let Some(hub) = presence {
                hub.on_location_removed(&app, account.as_str(), channel_id.as_str())
                    .await;
            }
            ctx.resp_bytes(Status::Success, Bytes::new()).await?;
        }
        Err(err) => {
            warn!(%err, "delete failed");
            ctx.resp_bytes(Status::SystemException, Bytes::new())
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use bytes::Bytes;
    use kim_protocol::pkt::{Flag, KickoutNotify, Session, Status};
    use kim_protocol::{
        AccountId, ChannelId, Command, GatewayId, LogicPkt, CMD_LOGIN_SIGN_IN, META_DEST_CHANNELS,
        META_DEST_SERVER,
    };
    use kim_router::{Dispatcher, Router, RouterError, SessionStorage};
    use kim_session::MemorySessionStore;

    use super::do_sys_login;
    use crate::users::MemoryUserDirectory;

    #[derive(Default)]
    struct RecordingDispatcher {
        pushes: Mutex<Vec<LogicPkt>>,
    }

    impl RecordingDispatcher {
        fn recorded(&self) -> Vec<LogicPkt> {
            self.pushes
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        }
    }

    #[async_trait]
    impl Dispatcher for RecordingDispatcher {
        async fn push(
            &self,
            gateway: &GatewayId,
            channels: &[ChannelId],
            mut pkt: LogicPkt,
        ) -> Result<(), RouterError> {
            pkt.set_meta(META_DEST_SERVER, gateway.as_str());
            pkt.set_meta(
                META_DEST_CHANNELS,
                &channels
                    .iter()
                    .map(ChannelId::as_str)
                    .collect::<Vec<_>>()
                    .join(","),
            );
            self.pushes
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(pkt);
            Ok(())
        }
    }

    fn acc(s: &str) -> AccountId {
        AccountId::from_trusted(s)
    }

    fn ch(s: &str) -> ChannelId {
        ChannelId::from_trusted(s)
    }

    fn wrapper(channel: &str) -> Session {
        Session {
            channel_id: channel.into(),
            gate_id: "wg-1".into(),
            tags: vec!["AutoGenerated".into()],
            ..Session::default()
        }
    }

    fn body_session_device(channel: &str, device: &str) -> Session {
        Session {
            channel_id: channel.into(),
            gate_id: "wg-1".into(),
            account: "alice".into(),
            app: "kim".into(),
            device: device.into(),
            ..Session::default()
        }
    }

    fn signin_pkt(channel: &str, body: &Session) -> LogicPkt {
        let mut pkt = LogicPkt::new(CMD_LOGIN_SIGN_IN, 1, Bytes::new());
        pkt.header.channel_id = channel.into();
        pkt.set_meta(META_DEST_SERVER, "wg-1");
        pkt.write_body(body);
        pkt
    }

    #[tokio::test]
    async fn second_web_login_keeps_both_sessions() {
        let cache: Arc<dyn SessionStorage> = Arc::new(MemorySessionStore::new());
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let users = Arc::new(MemoryUserDirectory::new());
        let mut router = Router::new();
        router.handle(Command::LoginSignIn, move |ctx| {
            let users = users.clone();
            async move { do_sys_login(ctx, users.as_ref()).await }
        });

        router
            .serve(
                signin_pkt("wg-1_alice_1", &body_session_device("wg-1_alice_1", "web")),
                dispatcher.clone(),
                cache.clone(),
                wrapper("wg-1_alice_1"),
            )
            .await
            .unwrap();
        router
            .serve(
                signin_pkt("wg-1_alice_2", &body_session_device("wg-1_alice_2", "web")),
                dispatcher.clone(),
                cache.clone(),
                wrapper("wg-1_alice_2"),
            )
            .await
            .unwrap();

        let kicks: Vec<_> = dispatcher
            .recorded()
            .into_iter()
            .filter(|p| p.header.flag == Flag::Push as i32)
            .collect();
        assert!(kicks.is_empty());
        assert!(cache.get(&ch("wg-1_alice_1")).await.is_ok());
        assert!(cache.get(&ch("wg-1_alice_2")).await.is_ok());
        let locs = cache.list_locations(&acc("alice")).await.unwrap();
        assert_eq!(locs.len(), 2);
    }

    #[tokio::test]
    async fn second_mobile_login_kicks_the_other_mobile() {
        let cache: Arc<dyn SessionStorage> = Arc::new(MemorySessionStore::new());
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let users = Arc::new(MemoryUserDirectory::new());
        let mut router = Router::new();
        router.handle(Command::LoginSignIn, move |ctx| {
            let users = users.clone();
            async move { do_sys_login(ctx, users.as_ref()).await }
        });

        router
            .serve(
                signin_pkt(
                    "wg-1_alice_1",
                    &body_session_device("wg-1_alice_1", "mobile"),
                ),
                dispatcher.clone(),
                cache.clone(),
                wrapper("wg-1_alice_1"),
            )
            .await
            .unwrap();
        router
            .serve(
                signin_pkt("wg-1_alice_2", &body_session_device("wg-1_alice_2", "ios")),
                dispatcher.clone(),
                cache.clone(),
                wrapper("wg-1_alice_2"),
            )
            .await
            .unwrap();

        let kicks: Vec<_> = dispatcher
            .recorded()
            .into_iter()
            .filter(|p| p.header.flag == Flag::Push as i32)
            .collect();
        assert_eq!(kicks.len(), 1);
        assert_eq!(kicks[0].header.command, CMD_LOGIN_SIGN_IN);
        let notify: KickoutNotify = kicks[0].read_body().unwrap();
        assert_eq!(notify.channel_id, "wg-1_alice_1");

        let stored = cache.get(&ch("wg-1_alice_2")).await.unwrap();
        assert_eq!(stored.account, "alice");
        assert_eq!(stored.device, "ios");
        assert!(stored.device_id.is_empty());
        assert!(!stored.tags.iter().any(|t| t == "AutoGenerated"));
    }

    #[tokio::test]
    async fn second_mobile_does_not_kick_web() {
        let cache: Arc<dyn SessionStorage> = Arc::new(MemorySessionStore::new());
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let users = Arc::new(MemoryUserDirectory::new());
        let mut router = Router::new();
        router.handle(Command::LoginSignIn, move |ctx| {
            let users = users.clone();
            async move { do_sys_login(ctx, users.as_ref()).await }
        });
        for (ch, device) in [
            ("wg-1_alice_web", "web"),
            ("wg-1_alice_m1", "mobile"),
            ("wg-1_alice_m2", "ios"),
        ] {
            router
                .serve(
                    signin_pkt(ch, &body_session_device(ch, device)),
                    dispatcher.clone(),
                    cache.clone(),
                    wrapper(ch),
                )
                .await
                .unwrap();
        }
        let kicks: Vec<_> = dispatcher
            .recorded()
            .into_iter()
            .filter(|p| p.header.flag == Flag::Push as i32)
            .collect();
        assert_eq!(kicks.len(), 1);
        let notify: KickoutNotify = kicks[0].read_body().unwrap();
        assert_eq!(notify.channel_id, "wg-1_alice_m1");
        assert!(cache.get(&ch("wg-1_alice_web")).await.is_ok());
        assert!(cache.get(&ch("wg-1_alice_m2")).await.is_ok());
    }

    #[tokio::test]
    async fn desktop_and_mobile_keep_both_sessions() {
        let cache: Arc<dyn SessionStorage> = Arc::new(MemorySessionStore::new());
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let users = Arc::new(MemoryUserDirectory::new());
        let mut router = Router::new();
        router.handle(Command::LoginSignIn, move |ctx| {
            let users = users.clone();
            async move { do_sys_login(ctx, users.as_ref()).await }
        });
        for (ch, device) in [("wg-1_alice_m", "mobile"), ("wg-1_alice_d", "desktop")] {
            router
                .serve(
                    signin_pkt(ch, &body_session_device(ch, device)),
                    dispatcher.clone(),
                    cache.clone(),
                    wrapper(ch),
                )
                .await
                .unwrap();
        }
        let kicks: Vec<_> = dispatcher
            .recorded()
            .into_iter()
            .filter(|p| p.header.flag == Flag::Push as i32)
            .collect();
        assert!(kicks.is_empty());
        assert!(cache.get(&ch("wg-1_alice_m")).await.is_ok());
        assert!(cache.get(&ch("wg-1_alice_d")).await.is_ok());
        let locs = cache.list_locations(&acc("alice")).await.unwrap();
        assert_eq!(locs.len(), 2);
    }

    struct FailBackfill;

    #[async_trait]
    impl crate::store::MessageStore for FailBackfill {
        async fn insert_user(
            &self,
            _: &str,
            _: &crate::store::InsertMessage,
        ) -> Result<crate::store::InsertResult, crate::store::StoreError> {
            Err(crate::store::StoreError::Backend("unused".into()))
        }
        async fn insert_group(
            &self,
            _: &str,
            _: &crate::store::InsertMessage,
            _: &[String],
        ) -> Result<crate::store::InsertResult, crate::store::StoreError> {
            Err(crate::store::StoreError::Backend("unused".into()))
        }
        async fn ack(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &[i64],
        ) -> Result<(), crate::store::StoreError> {
            Ok(())
        }
        async fn offline_index(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: i64,
            _: bool,
        ) -> Result<(Vec<crate::store::MessageIndexRow>, bool), crate::store::StoreError> {
            Ok((Vec::new(), false))
        }
        async fn backfill_delivery(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<(), crate::store::StoreError> {
            Err(crate::store::StoreError::Backend("backfill down".into()))
        }
        async fn offline_content(
            &self,
            _: &str,
            _: &str,
            _: &[i64],
        ) -> Result<Vec<crate::store::MessageContentRow>, crate::store::StoreError> {
            Ok(Vec::new())
        }
        async fn inbox(
            &self,
            _: &str,
            _: &str,
            _: i32,
        ) -> Result<Vec<crate::store::InboxEntry>, crate::store::StoreError> {
            Ok(Vec::new())
        }
        async fn history(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: crate::store::MessageKind,
            _: i64,
            _: i32,
        ) -> Result<Vec<crate::store::HistoryEntry>, crate::store::StoreError> {
            Ok(Vec::new())
        }
        async fn mark_read(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: crate::store::MessageKind,
            _: i64,
        ) -> Result<(), crate::store::StoreError> {
            Ok(())
        }
        async fn insert_bot_reply(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: i64,
            _: &crate::store::InsertMessage,
        ) -> Result<crate::store::InsertResult, crate::store::StoreError> {
            Err(crate::store::StoreError::Backend("unused".into()))
        }
        async fn bot_pending(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: i32,
        ) -> Result<Vec<crate::store::BotPendingItem>, crate::store::StoreError> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn backfill_failure_deletes_loc_without_login_resp() {
        let cache: Arc<dyn SessionStorage> = Arc::new(MemorySessionStore::new());
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let users = Arc::new(MemoryUserDirectory::new());
        let store = Arc::new(FailBackfill);
        let mut router = Router::new();
        router.handle(Command::LoginSignIn, {
            let users = users.clone();
            let store = store.clone();
            move |ctx| {
                let users = users.clone();
                let store = store.clone();
                async move {
                    super::do_sys_login_with_zone(
                        ctx,
                        "",
                        users.as_ref(),
                        Some(store.as_ref()),
                        true,
                        None,
                    )
                    .await
                }
            }
        });
        let body = Session {
            channel_id: "wg-1_alice_1".into(),
            gate_id: "wg-1".into(),
            account: "alice".into(),
            app: "kim".into(),
            device: "web".into(),
            jti: "j1".into(),
            ..Session::default()
        };
        router
            .serve(
                signin_pkt("wg-1_alice_1", &body),
                dispatcher.clone(),
                cache.clone(),
                wrapper("wg-1_alice_1"),
            )
            .await
            .unwrap();
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].header.status, Status::SystemException as i32);
        assert!(matches!(
            cache.get(&ch("wg-1_alice_1")).await,
            Err(kim_router::SessionError::NotFound)
        ));
        let locs = cache
            .list_locations(&acc("alice"))
            .await
            .unwrap_or_default();
        assert!(locs.is_empty());
    }
}
