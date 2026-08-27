use std::sync::Mutex;

use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_protocol::{LogicPkt, META_DEST_CHANNELS, META_DEST_SERVER};

use crate::{Dispatcher, Location, RouterError, SessionError, SessionStorage};

#[derive(Default)]
pub struct RecordingDispatcher {
    pushes: Mutex<Vec<RecordedPush>>,
}

#[derive(Clone)]
pub struct RecordedPush {
    pub gateway: String,
    pub channels: Vec<String>,
    pub pkt: LogicPkt,
}

impl RecordingDispatcher {
    pub fn recorded(&self) -> Vec<RecordedPush> {
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
        gateway: &str,
        channels: &[String],
        mut pkt: LogicPkt,
    ) -> Result<(), RouterError> {
        pkt.set_meta(META_DEST_SERVER, gateway);
        pkt.set_meta(META_DEST_CHANNELS, &channels.join(","));
        self.pushes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(RecordedPush {
                gateway: gateway.to_string(),
                channels: channels.to_vec(),
                pkt,
            });
        Ok(())
    }
}

pub struct NoopStorage;

#[async_trait]
impl SessionStorage for NoopStorage {
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
        Err(SessionError::NotFound)
    }

    async fn get_location(&self, _account: &str, _device: &str) -> Result<Location, SessionError> {
        Err(SessionError::NotFound)
    }
}
