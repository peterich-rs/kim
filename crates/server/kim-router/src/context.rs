use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use kim_protocol::pkt::{Flag, Header, Session, Status};
use kim_protocol::{AccountId, ChannelId, GatewayId, LogicPkt, ProtocolError};
use prost::Message;
use tracing::warn;

use crate::dispatcher::{Dispatcher, RouterError};
use crate::location::Location;
use crate::storage::{SessionError, SessionStorage};

pub struct Context {
    request: LogicPkt,
    session: Session,
    dispatcher: Arc<dyn Dispatcher>,
    storage: Arc<dyn SessionStorage>,
}

impl Context {
    pub fn new(
        request: LogicPkt,
        session: Session,
        dispatcher: Arc<dyn Dispatcher>,
        storage: Arc<dyn SessionStorage>,
    ) -> Self {
        Self {
            request,
            session,
            dispatcher,
            storage,
        }
    }

    pub fn header(&self) -> &Header {
        &self.request.header
    }

    pub fn request(&self) -> &LogicPkt {
        &self.request
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn read_body<T: Message + Default>(&self) -> Result<T, ProtocolError> {
        self.request.read_body()
    }

    pub async fn resp<B: Message>(
        &self,
        status: Status,
        body: Option<&B>,
    ) -> Result<(), RouterError> {
        let mut packet = self.response_packet(status);
        if let Some(body) = body {
            packet.write_body(body);
        }
        self.push_to_sender(packet).await
    }

    pub async fn resp_bytes(&self, status: Status, body: Bytes) -> Result<(), RouterError> {
        let mut packet = self.response_packet(status);
        packet.header.body_length = body.len() as u32;
        packet.body = body;
        self.push_to_sender(packet).await
    }

    /// Flag=Response, `status` as given, empty body. Do not send `err` Display
    /// to the client (not a stable ABI).
    pub async fn resp_with_error(
        &self,
        status: Status,
        _err: &(dyn std::error::Error + Send + Sync),
    ) -> Result<(), RouterError> {
        self.resp_bytes(status, Bytes::new()).await
    }

    /// Push to `recvs`, skipping the sender's own `channel_id`, coalescing by
    /// `gate_id`. Command is copied from the inbound request (friend notify).
    pub async fn dispatch<B: Message>(
        &self,
        body: &B,
        recvs: &[Location],
    ) -> Result<(), RouterError> {
        self.dispatch_cmd(&self.request.header.command, body, recvs)
            .await
    }

    /// Like [`dispatch`], but sets an explicit push `command` (e.g. profile
    /// snapshot `chat.user.updated` while handling `chat.user.update`).
    pub async fn dispatch_cmd<B: Message>(
        &self,
        command: &str,
        body: &B,
        recvs: &[Location],
    ) -> Result<(), RouterError> {
        if recvs.is_empty() {
            return Ok(());
        }
        let mut packet = LogicPkt::new_from(&self.request.header);
        packet.header.command = command.to_string();
        packet.header.flag = Flag::Push as i32;
        packet.write_body(body);

        let mut group: HashMap<GatewayId, Vec<ChannelId>> = HashMap::new();
        let mut order: Vec<GatewayId> = Vec::new();
        for recv in recvs {
            if recv.channel_id.as_str() == self.session.channel_id {
                continue;
            }
            match group.entry(recv.gate_id.clone()) {
                Entry::Vacant(v) => {
                    order.push(v.key().clone());
                    v.insert(vec![recv.channel_id.clone()]);
                }
                Entry::Occupied(mut o) => {
                    o.get_mut().push(recv.channel_id.clone());
                }
            }
        }

        let mut first_err = None;
        for gw in order {
            let Some(ids) = group.get(&gw) else {
                continue;
            };
            if let Err(err) = self.dispatcher.push(&gw, ids, packet.clone()).await {
                warn!(%err, gateway = %gw, "dispatch push failed");
                if first_err.is_none() {
                    first_err = Some(err);
                }
            }
        }
        match first_err {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    pub async fn add(&self, session: &Session) -> Result<(), SessionError> {
        self.storage.add(session).await
    }

    pub async fn delete(
        &self,
        account: &AccountId,
        channel_id: &ChannelId,
    ) -> Result<(), SessionError> {
        self.storage.delete(account, channel_id).await
    }

    pub async fn get_location(
        &self,
        account: &AccountId,
        device: &str,
    ) -> Result<Location, SessionError> {
        self.storage.get_location(account, device).await
    }

    pub async fn get_locations(
        &self,
        accounts: &[AccountId],
    ) -> Result<Vec<Location>, SessionError> {
        self.storage.get_locations(accounts).await
    }

    pub async fn list_locations(&self, account: &AccountId) -> Result<Vec<Location>, SessionError> {
        self.storage.list_locations(account).await
    }

    fn response_packet(&self, status: Status) -> LogicPkt {
        let mut packet = LogicPkt::new_from(&self.request.header);
        packet.header.status = status as i32;
        packet.header.flag = Flag::Response as i32;
        packet
    }

    async fn push_to_sender(&self, packet: LogicPkt) -> Result<(), RouterError> {
        let gate = GatewayId::from_trusted(&self.session.gate_id);
        let channel = ChannelId::from_trusted(&self.session.channel_id);
        self.dispatcher
            .push(&gate, std::slice::from_ref(&channel), packet)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{NoopStorage, RecordingDispatcher};
    use crate::{Router, RouterError};
    use kim_protocol::pkt::KickoutNotify;
    use kim_protocol::{
        Command, CMD_DEMO_ECHO, CMD_USER_UPDATED, META_DEST_CHANNELS, META_DEST_SERVER,
    };

    fn session(channel: &str, gate: &str) -> Session {
        Session {
            channel_id: channel.into(),
            gate_id: gate.into(),
            account: "alice".into(),
            ..Session::default()
        }
    }

    fn loc(channel: &str, gate: &str) -> Location {
        Location {
            channel_id: ChannelId::from_trusted(channel),
            gate_id: GatewayId::from_trusted(gate),
            device: String::new(),
            jti: String::new(),
        }
    }

    fn request(command: &str, body: Bytes) -> LogicPkt {
        let mut pkt = LogicPkt::new(command, 7, body);
        pkt.header.channel_id = "ch-self".into();
        pkt.set_meta(META_DEST_SERVER, "gate-a");
        pkt
    }

    #[tokio::test]
    async fn resp_bytes_echoes_header_and_body() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut router = Router::new();
        router.handle(Command::DemoEcho, |ctx| async move {
            let body = ctx.request().body.clone();
            let _ = ctx.resp_bytes(Status::Success, body).await;
            Ok(())
        });
        let pkt = request(CMD_DEMO_ECHO, Bytes::from_static(b"hello pkt"));
        router
            .serve(
                pkt,
                dispatcher.clone(),
                Arc::new(NoopStorage),
                session("ch-self", "gate-a"),
            )
            .await
            .unwrap();
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].gateway, "gate-a");
        assert_eq!(got[0].channels, vec!["ch-self".to_string()]);
        let p = &got[0].pkt;
        assert_eq!(p.header.command, CMD_DEMO_ECHO);
        assert_eq!(p.header.sequence, 7);
        assert_eq!(p.header.channel_id, "ch-self");
        assert_eq!(p.header.flag, Flag::Response as i32);
        assert_eq!(p.header.status, Status::Success as i32);
        assert_eq!(&p.body[..], b"hello pkt");
        assert_eq!(p.get_meta(META_DEST_SERVER), Some("gate-a"));
        assert_eq!(p.get_meta(META_DEST_CHANNELS), Some("ch-self"));
    }

    #[tokio::test]
    async fn dispatch_coalesces_same_gate() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut router = Router::new();
        router.handle(Command::LoginSignIn, |ctx| async move {
            let recvs = [loc("ch-a", "wg-1"), loc("ch-b", "wg-1")];
            let _ = ctx
                .dispatch(
                    &KickoutNotify {
                        channel_id: "ch-a".into(),
                    },
                    &recvs,
                )
                .await;
            Ok(())
        });
        router
            .serve(
                request("login.signin", Bytes::new()),
                dispatcher.clone(),
                Arc::new(NoopStorage),
                session("ch-self", "gate-a"),
            )
            .await
            .unwrap();
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].gateway, "wg-1");
        assert_eq!(
            got[0].channels,
            vec!["ch-a".to_string(), "ch-b".to_string()]
        );
        assert_eq!(got[0].pkt.get_meta(META_DEST_SERVER), Some("wg-1"));
        assert_eq!(got[0].pkt.get_meta(META_DEST_CHANNELS), Some("ch-a,ch-b"));
        assert_eq!(got[0].pkt.header.flag, Flag::Push as i32);
    }

    #[tokio::test]
    async fn dispatch_rewrites_dest_server_to_target_gate() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut router = Router::new();
        router.handle(Command::LoginSignIn, |ctx| async move {
            let recvs = [loc("ch-x", "gate-b")];
            let _ = ctx
                .dispatch(
                    &KickoutNotify {
                        channel_id: "ch-x".into(),
                    },
                    &recvs,
                )
                .await;
            Ok(())
        });
        router
            .serve(
                request("login.signin", Bytes::new()),
                dispatcher.clone(),
                Arc::new(NoopStorage),
                session("ch-self", "gate-a"),
            )
            .await
            .unwrap();
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].gateway, "gate-b");
        assert_eq!(got[0].pkt.get_meta(META_DEST_SERVER), Some("gate-b"));
        assert_ne!(got[0].pkt.get_meta(META_DEST_SERVER), Some("gate-a"));
    }

    #[tokio::test]
    async fn dispatch_skips_own_channel() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let mut router = Router::new();
        router.handle(Command::LoginSignIn, |ctx| async move {
            let recvs = [loc("ch-self", "wg-1"), loc("ch-other", "wg-1")];
            let _ = ctx
                .dispatch(
                    &KickoutNotify {
                        channel_id: "ch-self".into(),
                    },
                    &recvs,
                )
                .await;
            Ok(())
        });
        router
            .serve(
                request("login.signin", Bytes::new()),
                dispatcher.clone(),
                Arc::new(NoopStorage),
                session("ch-self", "wg-1"),
            )
            .await
            .unwrap();
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].channels, vec!["ch-other".to_string()]);
        assert_eq!(got[0].pkt.get_meta(META_DEST_CHANNELS), Some("ch-other"));
    }

    #[tokio::test]
    async fn dispatch_two_gates_are_separate_pushes() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let ctx = Context::new(
            request("login.signin", Bytes::new()),
            session("ch-self", "gate-a"),
            dispatcher.clone(),
            Arc::new(NoopStorage),
        );
        ctx.dispatch(
            &KickoutNotify {
                channel_id: "ch-a".into(),
            },
            &[
                loc("ch-a", "wg-1"),
                loc("ch-b", "wg-2"),
                loc("ch-c", "wg-1"),
            ],
        )
        .await
        .unwrap();
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].gateway, "wg-1");
        assert_eq!(
            got[0].channels,
            vec!["ch-a".to_string(), "ch-c".to_string()]
        );
        assert_eq!(got[0].pkt.get_meta(META_DEST_CHANNELS), Some("ch-a,ch-c"));
        assert_eq!(got[1].gateway, "wg-2");
        assert_eq!(got[1].channels, vec!["ch-b".to_string()]);
        assert_eq!(got[1].pkt.get_meta(META_DEST_CHANNELS), Some("ch-b"));
        assert_ne!(got[0].channels, got[1].channels);
    }

    #[tokio::test]
    async fn dispatch_cmd_overrides_request_command() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let ctx = Context::new(
            request(CMD_DEMO_ECHO, Bytes::new()),
            session("ch-self", "wg-1"),
            dispatcher.clone(),
            Arc::new(NoopStorage),
        );
        ctx.dispatch_cmd(
            CMD_USER_UPDATED,
            &KickoutNotify {
                channel_id: "ch-a".into(),
            },
            &[loc("ch-a", "wg-1")],
        )
        .await
        .unwrap();
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].pkt.header.command, CMD_USER_UPDATED);
        assert_eq!(got[0].pkt.header.flag, Flag::Push as i32);
    }

    #[tokio::test]
    async fn dispatch_continues_after_first_push_error() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        dispatcher.fail_on("wg-1");
        let ctx = Context::new(
            request("login.signin", Bytes::new()),
            session("ch-self", "gate-a"),
            dispatcher.clone(),
            Arc::new(NoopStorage),
        );
        let err = ctx
            .dispatch(
                &KickoutNotify {
                    channel_id: "ch-a".into(),
                },
                &[loc("ch-a", "wg-1"), loc("ch-b", "wg-2")],
            )
            .await
            .unwrap_err();
        assert!(matches!(err, RouterError::Dispatcher(ref g) if g == "wg-1"));
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].gateway, "wg-1");
        assert_eq!(got[1].gateway, "wg-2");
    }

    struct StubLocations {
        result: Result<Vec<Location>, SessionError>,
    }

    #[async_trait::async_trait]
    impl SessionStorage for StubLocations {
        async fn add(&self, _session: &Session) -> Result<(), SessionError> {
            Ok(())
        }

        async fn delete(
            &self,
            _account: &AccountId,
            _channel_id: &ChannelId,
        ) -> Result<(), SessionError> {
            Ok(())
        }

        async fn get(&self, _channel_id: &ChannelId) -> Result<Session, SessionError> {
            Err(SessionError::NotFound)
        }

        async fn get_locations(
            &self,
            _accounts: &[AccountId],
        ) -> Result<Vec<Location>, SessionError> {
            match &self.result {
                Ok(locs) => Ok(locs.clone()),
                Err(SessionError::NotFound) => Err(SessionError::NotFound),
                Err(SessionError::Truncated) => Err(SessionError::Truncated),
                Err(SessionError::InvalidUtf8) => Err(SessionError::InvalidUtf8),
                Err(SessionError::Other(msg)) => Err(SessionError::Other(msg.clone())),
            }
        }

        async fn get_location(
            &self,
            _account: &AccountId,
            _device: &str,
        ) -> Result<Location, SessionError> {
            Err(SessionError::NotFound)
        }
    }

    #[tokio::test]
    async fn get_locations_forwards_two_locs() {
        let loc_a = loc("ch-a", "gw-1");
        let loc_b = loc("ch-b", "gw-2");
        let ctx = Context::new(
            request("chat.group.talk", Bytes::new()),
            session("ch-self", "gate-a"),
            Arc::new(RecordingDispatcher::default()),
            Arc::new(StubLocations {
                result: Ok(vec![loc_a.clone(), loc_b.clone()]),
            }),
        );
        let got = ctx
            .get_locations(&[
                AccountId::from_trusted("alice"),
                AccountId::from_trusted("bob"),
            ])
            .await
            .unwrap();
        assert_eq!(got, vec![loc_a, loc_b]);
    }

    #[tokio::test]
    async fn get_locations_forwards_not_found() {
        let ctx = Context::new(
            request("chat.group.talk", Bytes::new()),
            session("ch-self", "gate-a"),
            Arc::new(RecordingDispatcher::default()),
            Arc::new(StubLocations {
                result: Err(SessionError::NotFound),
            }),
        );
        let err = ctx
            .get_locations(&[AccountId::from_trusted("carol")])
            .await
            .unwrap_err();
        assert!(matches!(err, SessionError::NotFound));
    }
}
