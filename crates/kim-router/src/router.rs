use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use kim_protocol::pkt::{Session, Status};
use kim_protocol::{Command, LogicPkt};

use crate::context::Context;
use crate::dispatcher::{Dispatcher, RouterError};
use crate::storage::SessionStorage;

#[allow(clippy::type_complexity)]
pub type HandlerFn = Arc<
    dyn Fn(Context) -> Pin<Box<dyn Future<Output = Result<(), RouterError>> + Send>> + Send + Sync,
>;

#[derive(Default)]
pub struct Router {
    handlers: HashMap<Command, HandlerFn>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle<F, Fut>(&mut self, command: Command, f: F)
    where
        F: Fn(Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), RouterError>> + Send + 'static,
    {
        self.handlers
            .insert(command, Arc::new(move |ctx| Box::pin(f(ctx))));
    }

    pub async fn serve(
        &self,
        packet: LogicPkt,
        dispatcher: Arc<dyn Dispatcher>,
        cache: Arc<dyn SessionStorage>,
        session: Session,
    ) -> Result<(), RouterError> {
        let handler =
            Command::parse(&packet.header.command).and_then(|cmd| self.handlers.get(&cmd).cloned());
        match handler {
            Some(handler) => handler(Context::new(packet, session, dispatcher, cache)).await,
            None => {
                let ctx = Context::new(packet, session, dispatcher, cache);
                ctx.resp_bytes(Status::CommandNotFound, Bytes::new()).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{NoopStorage, RecordingDispatcher};
    use kim_protocol::pkt::Flag;
    use kim_protocol::META_DEST_SERVER;

    #[tokio::test]
    async fn unknown_command_is_command_not_found() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let router = Router::new();
        let mut pkt = LogicPkt::new("no.such", 3, Bytes::new());
        pkt.header.channel_id = "ch-1".into();
        pkt.set_meta(META_DEST_SERVER, "gate-a");
        let session = Session {
            channel_id: "ch-1".into(),
            gate_id: "gate-a".into(),
            ..Session::default()
        };
        router
            .serve(pkt, dispatcher.clone(), Arc::new(NoopStorage), session)
            .await
            .unwrap();
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].pkt.header.command, "no.such");
        assert_eq!(got[0].pkt.header.sequence, 3);
        assert_eq!(got[0].pkt.header.flag, Flag::Response as i32);
        assert_eq!(got[0].pkt.header.status, Status::CommandNotFound as i32);
        assert!(got[0].pkt.body.is_empty());
    }

    #[tokio::test]
    async fn unregistered_known_command_is_command_not_found() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let router = Router::new();
        let mut pkt = LogicPkt::new(Command::Presence.as_str(), 3, Bytes::new());
        pkt.header.channel_id = "ch-1".into();
        pkt.set_meta(META_DEST_SERVER, "gate-a");
        let session = Session {
            channel_id: "ch-1".into(),
            gate_id: "gate-a".into(),
            ..Session::default()
        };
        router
            .serve(pkt, dispatcher.clone(), Arc::new(NoopStorage), session)
            .await
            .unwrap();
        let got = dispatcher.recorded();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].pkt.header.command, Command::Presence.as_str());
        assert_eq!(got[0].pkt.header.status, Status::CommandNotFound as i32);
    }
}
