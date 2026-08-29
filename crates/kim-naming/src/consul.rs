//! Consul HTTP catalog adapter. Optional: default tests stay on StaticNaming.
//! Booklet uses DNS SRV on port 53; this repo does not take over the host resolver.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::naming::{Error, Naming};
use crate::registration::DefaultRegistration;

type Callback = Arc<dyn Fn(Vec<DefaultRegistration>) + Send + Sync>;

pub struct ConsulNaming {
    base: String,
    http: reqwest::Client,
    wait_http: reqwest::Client,
    watches: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl ConsulNaming {
    pub fn new(base: &str) -> Result<Self, Error> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| Error::Other(e.to_string()))?;
        let wait_http = reqwest::Client::builder()
            .timeout(Duration::from_secs(70))
            .build()
            .map_err(|e| Error::Other(e.to_string()))?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            http,
            wait_http,
            watches: Mutex::new(HashMap::new()),
        })
    }

    fn parse_rows(rows: Vec<HealthService>, tags: &[&str]) -> Vec<DefaultRegistration> {
        rows.into_iter()
            .filter_map(|row| {
                let meta = row.service.meta.unwrap_or_default();
                let protocol = meta.get("protocol")?.clone();
                if protocol.is_empty() {
                    return None;
                }
                Some(DefaultRegistration {
                    service_id: row.service.id,
                    service_name: row.service.service,
                    protocol,
                    public_address: row.service.address,
                    public_port: row.service.port,
                    tags: row.service.tags.unwrap_or_default(),
                    meta,
                })
            })
            .filter(|r| tags.iter().all(|t| r.tags.iter().any(|x| x == t)))
            .collect()
    }

    async fn fetch(
        &self,
        client: &reqwest::Client,
        service_name: &str,
        index: u64,
        wait: Option<&str>,
    ) -> Result<(Vec<DefaultRegistration>, Option<u64>), Error> {
        let mut url = format!(
            "{}/v1/health/service/{}?passing=1&index={index}",
            self.base, service_name
        );
        if let Some(w) = wait {
            url.push_str("&wait=");
            url.push_str(w);
        }
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Other(format!("consul http {}", resp.status())));
        }
        let new_index = resp
            .headers()
            .get("X-Consul-Index")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        let rows: Vec<HealthService> =
            resp.json().await.map_err(|e| Error::Other(e.to_string()))?;
        Ok((Self::parse_rows(rows, &[]), new_index))
    }
}

impl Drop for ConsulNaming {
    fn drop(&mut self) {
        if let Ok(map) = self.watches.try_lock() {
            for stop in map.values() {
                stop.store(true, Ordering::SeqCst);
            }
        }
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
    #[serde(rename = "Meta")]
    meta: Option<HashMap<String, String>>,
}

#[async_trait]
impl Naming for ConsulNaming {
    async fn find(
        &self,
        service_name: &str,
        tags: &[&str],
    ) -> Result<Vec<DefaultRegistration>, Error> {
        let (list, _) = self.fetch(&self.http, service_name, 0, None).await?;
        Ok(list
            .into_iter()
            .filter(|r| tags.iter().all(|t| r.tags.iter().any(|x| x == t)))
            .collect())
    }

    async fn subscribe(&self, service_name: &str, callback: Callback) -> Result<(), Error> {
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut w = self.watches.lock().await;
            if let Some(old) = w.insert(service_name.to_string(), stop.clone()) {
                old.store(true, Ordering::SeqCst);
            }
        }
        let this_base = self.base.clone();
        let wait_http = self.wait_http.clone();
        let name = service_name.to_string();
        tokio::spawn(async move {
            watch_loop(this_base, wait_http, name, callback, stop).await;
        });
        Ok(())
    }

    async fn unsubscribe(&self, service_name: &str) -> Result<(), Error> {
        if let Some(stop) = self.watches.lock().await.remove(service_name) {
            stop.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    async fn register(&self, service: DefaultRegistration) -> Result<(), Error> {
        let health_url = service
            .meta
            .get("health_url")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Error::Other("health_url required to register".into()))?;
        if service.protocol.is_empty() {
            return Err(Error::Other("protocol required to register".into()));
        }
        let mut meta = service.meta.clone();
        meta.insert("protocol".into(), service.protocol.clone());
        let url = format!("{}/v1/agent/service/register", self.base);
        let body = serde_json::json!({
            "ID": service.service_id,
            "Name": service.service_name,
            "Address": service.public_address,
            "Port": service.public_port,
            "Tags": service.tags,
            "Meta": meta,
            "Check": {
                "HTTP": health_url,
                "Interval": "10s",
                "Timeout": "2s",
            }
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

async fn watch_loop(
    base: String,
    http: reqwest::Client,
    service_name: String,
    callback: Callback,
    stop: Arc<AtomicBool>,
) {
    let mut index: u64 = 0;
    while !stop.load(Ordering::SeqCst) {
        let url =
            format!("{base}/v1/health/service/{service_name}?passing=1&index={index}&wait=60s");
        let resp = match http.get(&url).send().await {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!(%err, service = %service_name, "consul watch");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        if stop.load(Ordering::SeqCst) {
            break;
        }
        if !resp.status().is_success() {
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }
        let Some(new_index) = resp
            .headers()
            .get("X-Consul-Index")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
        else {
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        };
        let rows: Vec<HealthService> = match resp.json().await {
            Ok(r) => r,
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        if new_index == index {
            continue;
        }
        index = new_index;
        callback(ConsulNaming::parse_rows(rows, &[]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path, Query, State};
    use axum::http::{HeaderMap, HeaderValue, StatusCode};
    use axum::routing::{get, put};
    use axum::{Json, Router};
    use serde::Deserialize;
    use serde_json::{json, Value};
    use std::sync::Mutex as StdMutex;
    use tokio::sync::Notify;

    #[tokio::test]
    async fn missing_consul_is_error_not_panic() {
        let naming = ConsulNaming::new("http://127.0.0.1:1").unwrap();
        assert!(naming.find("royal", &[]).await.is_err());
    }

    #[derive(Clone)]
    struct Mock {
        inner: Arc<StdMutex<MockInner>>,
        changed: Arc<Notify>,
    }

    struct MockInner {
        index: u64,
        rows: Value,
        last_register: Option<Value>,
    }

    #[derive(Deserialize)]
    struct Q {
        index: Option<u64>,
        wait: Option<String>,
    }

    async fn health(
        State(m): State<Mock>,
        Path(_name): Path<String>,
        Query(q): Query<Q>,
    ) -> (HeaderMap, Json<Value>) {
        let wait = q.wait.is_some();
        let req_index = q.index.unwrap_or(0);
        if wait {
            let cur = m.inner.lock().unwrap_or_else(|e| e.into_inner()).index;
            if req_index == cur {
                let n = m.changed.clone();
                tokio::select! {
                    _ = n.notified() => {}
                    _ = tokio::time::sleep(Duration::from_millis(80)) => {}
                }
            }
        }
        let g = m.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Consul-Index",
            HeaderValue::from_str(&g.index.to_string()).unwrap(),
        );
        (headers, Json(g.rows.clone()))
    }

    async fn register(State(m): State<Mock>, Json(body): Json<Value>) -> StatusCode {
        m.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_register = Some(body);
        StatusCode::OK
    }

    async fn mock_listen() -> (String, Mock) {
        let mock = Mock {
            inner: Arc::new(StdMutex::new(MockInner {
                index: 1,
                rows: json!([]),
                last_register: None,
            })),
            changed: Arc::new(Notify::new()),
        };
        let app = Router::new()
            .route("/v1/health/service/{name}", get(health))
            .route("/v1/agent/service/register", put(register))
            .with_state(mock.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        (format!("http://{addr}"), mock)
    }

    fn tcp_row() -> Value {
        json!([{
            "Service": {
                "ID": "chat-1",
                "Service": "chat",
                "Address": "127.0.0.1",
                "Port": 8002,
                "Tags": ["zone:zone_local"],
                "Meta": { "protocol": "tcp" }
            }
        }])
    }

    fn no_meta_row() -> Value {
        json!([{
            "Service": {
                "ID": "chat-1",
                "Service": "chat",
                "Address": "127.0.0.1",
                "Port": 8002,
                "Tags": [],
            }
        }])
    }

    #[tokio::test]
    async fn find_reads_protocol_and_drops_missing_meta() {
        let (base, mock) = mock_listen().await;
        mock.inner.lock().unwrap().rows = tcp_row();
        let naming = ConsulNaming::new(&base).unwrap();
        let found = naming.find("chat", &[]).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].protocol, "tcp");

        mock.inner.lock().unwrap().rows = no_meta_row();
        let empty = naming.find("chat", &[]).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn register_requires_health_url_and_sends_check() {
        let (base, mock) = mock_listen().await;
        let naming = ConsulNaming::new(&base).unwrap();
        let mut reg = DefaultRegistration {
            service_id: "chat-1".into(),
            service_name: "chat".into(),
            protocol: "tcp".into(),
            public_address: "chat".into(),
            public_port: 8002,
            tags: vec!["zone:zone_local".into()],
            meta: HashMap::new(),
        };
        assert!(naming.register(reg.clone()).await.is_err());
        reg.meta
            .insert("health_url".into(), "http://chat:9002/health".into());
        naming.register(reg).await.unwrap();
        let body = mock
            .inner
            .lock()
            .unwrap()
            .last_register
            .clone()
            .expect("register body");
        assert_eq!(body["Meta"]["protocol"], "tcp");
        assert_eq!(body["Check"]["HTTP"], "http://chat:9002/health");
    }

    #[tokio::test]
    async fn watch_skips_same_index_then_callbacks_on_change() {
        let (base, mock) = mock_listen().await;
        mock.inner.lock().unwrap().rows = json!([]);
        mock.inner.lock().unwrap().index = 3;
        let naming = ConsulNaming::new(&base).unwrap();
        let hits = Arc::new(StdMutex::new(Vec::<usize>::new()));
        let hits2 = hits.clone();
        naming
            .subscribe(
                "chat",
                Arc::new(move |list| {
                    hits2.lock().unwrap().push(list.len());
                }),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        {
            let mut g = mock.inner.lock().unwrap();
            g.rows = tcp_row();
            g.index = 4;
        }
        mock.changed.notify_waiters();
        tokio::time::sleep(Duration::from_millis(80)).await;
        let got = hits.lock().unwrap().clone();
        assert!(got.contains(&1), "got {got:?}");
        naming.unsubscribe("chat").await.unwrap();
    }
}
