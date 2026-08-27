use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;

use crate::{Acceptor, Error, MessageListener, StateListener};

#[async_trait]
pub trait Server: Send {
    fn set_acceptor(&mut self, acceptor: Arc<dyn Acceptor>);
    fn set_message_listener(&mut self, listener: Arc<dyn MessageListener>);
    fn set_state_listener(&mut self, listener: Arc<dyn StateListener>);
    fn set_read_wait(&mut self, wait: Duration);

    async fn start(&self) -> Result<(), Error>;
    async fn push(&self, channel_id: &str, payload: Bytes) -> Result<(), Error>;
    async fn shutdown(&self) -> Result<(), Error>;
}

#[async_trait]
pub trait Client: Send {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn set_dialer(&mut self, dialer: Arc<dyn crate::Dialer>);

    async fn connect(&mut self, addr: &str) -> Result<(), Error>;
    async fn send(&self, payload: Bytes) -> Result<(), Error>;
    async fn read(&mut self) -> Result<crate::Frame, Error>;
    async fn close(&mut self) -> Result<(), Error>;
}
