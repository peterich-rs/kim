//! Consul HTTP catalog adapter. Optional: default tests stay on StaticNaming.
//! Booklet uses DNS SRV on port 53; this repo does not take over the host resolver.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use crate::naming::{Error, Naming};
use crate::registration::DefaultRegistration;

type Callback = Arc<dyn Fn(Vec<DefaultRegistration>) + Send + Sync>;

pub struct ConsulNaming {
    base: String,
    http: reqwest::Client,
}

impl ConsulNaming {
    pub fn new(base: &str) -> Result<Self, Error> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| Error::Other(e.to_string()))?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            http,
        })
    }
}

#[derive(Deserialize)]
struct HealthService {
    #[serde(rename = "Service")]
    service: HealthServiceBody,
}

#[derive(Deserialize)]
struct HealthServiceBody {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Service")]
    service: String,
    #[serde(rename = "Address")]
    address: String,
    #[serde(rename = "Port")]
    port: u16,
    #[serde(rename = "Tags")]
    tags: Option<Vec<String>>,
}

#[async_trait]
impl Naming for ConsulNaming {
    async fn find(
        &self,
        service_name: &str,
        tags: &[&str],
    ) -> Result<Vec<DefaultRegistration>, Error> {
        let url = format!("{}/v1/health/service/{}?passing=1", self.base, service_name);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Other(format!("consul http {}", resp.status())));
        }
        let rows: Vec<HealthService> =
            resp.json().await.map_err(|e| Error::Other(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|row| DefaultRegistration {
                service_id: row.service.id,
                service_name: row.service.service,
                protocol: "http".into(),
                public_address: row.service.address,
                public_port: row.service.port,
                tags: row.service.tags.unwrap_or_default(),
                meta: HashMap::new(),
            })
            .filter(|r| tags.iter().all(|t| r.tags.iter().any(|x| x == t)))
            .collect())
    }

    async fn subscribe(&self, service_name: &str, callback: Callback) -> Result<(), Error> {
        let snap = self.find(service_name, &[]).await?;
        callback(snap);
        Ok(())
    }

    async fn unsubscribe(&self, _service_name: &str) -> Result<(), Error> {
        Ok(())
    }

    async fn register(&self, service: DefaultRegistration) -> Result<(), Error> {
        let url = format!("{}/v1/agent/service/register", self.base);
        let body = serde_json::json!({
            "ID": service.service_id,
            "Name": service.service_name,
            "Address": service.public_address,
            "Port": service.public_port,
            "Tags": service.tags,
        });
        let resp = self
            .http
            .put(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Other(format!("consul register {}", resp.status())));
        }
        Ok(())
    }

    async fn deregister(&self, service_id: &str) -> Result<(), Error> {
        let url = format!("{}/v1/agent/service/deregister/{service_id}", self.base);
        let resp = self
            .http
            .put(&url)
            .send()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Other(format!("consul deregister {}", resp.status())));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_consul_is_error_not_panic() {
        let naming = ConsulNaming::new("http://127.0.0.1:1").unwrap();
        assert!(naming.find("royal", &[]).await.is_err());
    }
}
