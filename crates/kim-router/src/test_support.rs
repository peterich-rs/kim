use std::sync::Mutex;

use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_protocol::{
    AccountId, ChannelId, GatewayId, LogicPkt, META_DEST_CHANNELS, META_DEST_SERVER,
};

use crate::{Dispatcher, Location, RouterError, SessionError, SessionStorage};

#[derive(Default)]
pub struct RecordingDispatcher {
    pushes: Mutex<Vec<RecordedPush>>,
    fail_gateways: Mutex<Vec<String>>,
    hang_gateways: Mutex<Vec<String>>,
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

    pub fn fail_on(&self, gateway: &str) {
        self.fail_gateways
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(gateway.to_string());
    }

    pub fn hang_on(&self, gateway: &str) {
        self.hang_gateways
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(gateway.to_string());
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
        let joined = channels
            .iter()
            .map(ChannelId::as_str)
            .collect::<Vec<_>>()
            .join(",");
        pkt.set_meta(META_DEST_SERVER, gateway.as_str());
        pkt.set_meta(META_DEST_CHANNELS, &joined);
        let fail = self
            .fail_gateways
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|g| g == gateway.as_str());
        self.pushes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(RecordedPush {
                gateway: gateway.as_str().to_owned(),
                channels: channels.iter().map(|c| c.as_str().to_owned()).collect(),
                pkt,
            });
        let hang = self
            .hang_gateways
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|g| g == gateway.as_str());
        if hang {
            std::future::pending::<()>().await;
        }
        if fail {
            return Err(RouterError::Dispatcher(gateway.as_str().to_owned()));
        }
        Ok(())
    }
}

pub struct NoopStorage;

#[async_trait]
impl SessionStorage for NoopStorage {
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

    async fn get_locations(&self, _accounts: &[AccountId]) -> Result<Vec<Location>, SessionError> {
        Err(SessionError::NotFound)
    }

    async fn get_location(
        &self,
        _account: &AccountId,
        _device: &str,
    ) -> Result<Location, SessionError> {
        Err(SessionError::NotFound)
    }
}
