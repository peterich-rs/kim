use std::sync::Arc;

use async_trait::async_trait;

use crate::registration::DefaultRegistration;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("consul http {status}")]
    Http { status: u16 },
    #[error("transport: {0}")]
    Transport(String),
    #[error("rebuild with --features consul")]
    FeatureDisabled,
    #[error("tls: {0}")]
    Tls(String),
    #[error("{0}")]
    Invalid(String),
}

#[async_trait]
pub trait Naming: Send + Sync {
    async fn find(
        &self,
        service_name: &str,
        tags: &[&str],
    ) -> Result<Vec<DefaultRegistration>, Error>;
    async fn subscribe(
        &self,
        service_name: &str,
        callback: Arc<dyn Fn(Vec<DefaultRegistration>) + Send + Sync>,
    ) -> Result<(), Error>;
    async fn unsubscribe(&self, service_name: &str) -> Result<(), Error>;
    async fn register(&self, service: DefaultRegistration) -> Result<(), Error>;
    async fn deregister(&self, service_id: &str) -> Result<(), Error>;
}
