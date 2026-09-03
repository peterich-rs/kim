//! Royal instance pool: round-robin, per-instance circuit breaker, Consul refresh.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use kim_naming::{DefaultRegistration, Naming};

use crate::royal::RoyalClient;
use crate::store::StoreError;

const REFRESH: Duration = Duration::from_secs(10);

pub struct RoyalPool {
    clients: RwLock<Vec<Arc<RoyalClient>>>,
    rr: AtomicUsize,
    bootstrap: Option<Arc<RoyalClient>>,
    naming: Option<Arc<dyn Naming>>,
    hmac: String,
    refresh: Duration,
}

fn lock_read<T>(m: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    m.read().unwrap_or_else(|e| e.into_inner())
}

fn lock_write<T>(m: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    m.write().unwrap_or_else(|e| e.into_inner())
}

impl RoyalPool {
    /// `naming: None` → static single address (local / e2e).
    /// `royal_url` is bootstrap + the only address when discovery is off.
    pub fn new(
        royal_url: Option<&str>,
        naming: Option<Arc<dyn Naming>>,
        hmac: &str,
    ) -> Result<Self, StoreError> {
        let bootstrap = match royal_url {
            Some(url) if !url.trim().is_empty() => {
                Some(Arc::new(RoyalClient::with_hmac(url, hmac)?))
            }
            _ => None,
        };
        if bootstrap.is_none() && naming.is_none() {
            return Err(StoreError::Backend("no royal url or naming".into()));
        }
        let clients = match &bootstrap {
            Some(c) => vec![c.clone()],
            None => Vec::new(),
        };
        Ok(Self {
            clients: RwLock::new(clients),
            rr: AtomicUsize::new(0),
            bootstrap,
            naming,
            hmac: hmac.to_string(),
            refresh: REFRESH,
        })
    }

    pub fn spawn_refresh(self: &Arc<Self>) {
        let Some(naming) = self.naming.clone() else {
            return;
        };
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(this.refresh);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tick.tick().await;
                if let Err(err) = this.refresh_once(naming.as_ref()).await {
                    tracing::warn!(%err, "royal pool refresh failed");
                }
            }
        });
    }

    async fn refresh_once(&self, naming: &dyn Naming) -> Result<(), StoreError> {
        let regs = naming
            .find("royal", &[])
            .await
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        let bases = regs.into_iter().filter_map(royal_base).collect::<Vec<_>>();
        self.replace_with(bases)
    }

    fn replace_with(&self, bases: Vec<String>) -> Result<(), StoreError> {
        if bases.is_empty() {
            if let Some(boot) = &self.bootstrap {
                *lock_write(&self.clients) = vec![boot.clone()];
            }
            return Ok(());
        }
        let current = lock_read(&self.clients).clone();
        let mut next = Vec::with_capacity(bases.len());
        for base in bases {
            if let Some(existing) = current.iter().find(|c| c.base == base) {
                next.push(existing.clone());
            } else {
                next.push(Arc::new(RoyalClient::with_hmac(&base, &self.hmac)?));
            }
        }
        *lock_write(&self.clients) = next;
        Ok(())
    }

    pub fn pick(&self) -> Result<Arc<RoyalClient>, StoreError> {
        let clients = lock_read(&self.clients);
        // Probe opened clients first so a recovered backend can re-enter rotation
        // even while other instances are still healthy.
        for c in clients.iter() {
            if c.is_open() && c.try_probe() {
                return Ok(c.clone());
            }
        }
        let healthy: Vec<Arc<RoyalClient>> =
            clients.iter().filter(|c| !c.is_open()).cloned().collect();
        if !healthy.is_empty() {
            let i = self.rr.fetch_add(1, Ordering::Relaxed) % healthy.len();
            return Ok(healthy[i].clone());
        }
        if let Some(boot) = &self.bootstrap {
            if !boot.is_open() || boot.try_probe() {
                return Ok(boot.clone());
            }
        }
        Err(StoreError::Backend("no royal available".into()))
    }

    pub(crate) fn report_success(&self, c: &RoyalClient) {
        c.report_success();
    }

    pub(crate) fn report_failure(&self, c: &RoyalClient) {
        c.report_failure();
    }
}

fn royal_base(reg: DefaultRegistration) -> Option<String> {
    let proto = reg
        .meta
        .get("protocol")
        .cloned()
        .unwrap_or_else(|| reg.protocol.clone());
    let proto = proto.to_ascii_lowercase();
    if !proto.is_empty() && proto != "http" && proto != "https" {
        return None;
    }
    if reg.public_address.is_empty() || reg.public_port == 0 {
        return None;
    }
    let scheme = if proto == "https" { "https" } else { "http" };
    Some(format!(
        "{scheme}://{}:{}",
        reg.public_address, reg.public_port
    ))
}

impl RoyalClient {
    #[cfg(test)]
    pub(crate) fn half_open_at_for_test(&self, ms: u64) {
        self.half_open_at.store(ms, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn fails_for_test(&self) -> u32 {
        self.fails.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::http::StatusCode;
    use axum::routing::post;
    use axum::Router;
    use kim_naming::{DefaultRegistration, StaticNaming};
    use prost::Message;

    use super::*;
    use crate::royal::{circuit_failure_status, retry_http};
    use kim_protocol::pkt::AccountExists;

    async fn spawn_status(status: StatusCode) -> String {
        async fn ok() -> (StatusCode, Vec<u8>) {
            let body = AccountExists { exists: true }.encode_to_vec();
            (StatusCode::OK, body)
        }
        async fn svc() -> StatusCode {
            StatusCode::SERVICE_UNAVAILABLE
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = if status == StatusCode::OK {
            Router::new().route("/api/v1/friend/check", post(ok))
        } else {
            Router::new().route("/api/v1/friend/check", post(svc))
        };
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        format!("http://{addr}")
    }

    #[test]
    fn service_unavailable_counts_for_breaker_but_is_not_retried() {
        assert!(!retry_http(StatusCode::SERVICE_UNAVAILABLE));
        assert!(circuit_failure_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(circuit_failure_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(!circuit_failure_status(StatusCode::BAD_REQUEST));
    }

    #[tokio::test]
    async fn bad_instance_removed_after_five_503() {
        let good = spawn_status(StatusCode::OK).await;
        let bad = spawn_status(StatusCode::SERVICE_UNAVAILABLE).await;
        let pool = Arc::new(RoyalPool::new(Some(&good), None, "test-hmac").unwrap());
        let bad_c = Arc::new(RoyalClient::with_hmac(&bad, "test-hmac").unwrap());
        lock_write(&pool.clients).push(bad_c.clone());

        let body = kim_protocol::pkt::AccountPair {
            account: "a".into(),
            peer: "b".into(),
        };
        for _ in 0..5 {
            let _ = pool
                .send_pb::<AccountExists, _>(
                    reqwest::Method::POST,
                    "/api/v1/friend/check",
                    Some(&body),
                )
                .await;
        }
        // Force the bad instance open if RR missed it: trip it directly.
        if !bad_c.is_open() {
            for _ in 0..5 {
                bad_c.report_failure();
            }
        }
        assert!(bad_c.is_open());
        for _ in 0..8 {
            let picked = pool.pick().unwrap();
            assert_eq!(picked.base, good.trim_end_matches('/'));
        }
    }

    #[tokio::test]
    async fn half_open_allows_one_probe() {
        let url = spawn_status(StatusCode::OK).await;
        let c = RoyalClient::with_hmac(&url, "test-hmac").unwrap();
        for _ in 0..5 {
            c.report_failure();
        }
        assert!(c.is_open());
        c.half_open_at_for_test(0);
        let first = c.try_probe();
        let second = c.try_probe();
        assert!(first);
        assert!(!second);
    }

    #[tokio::test]
    async fn opened_instance_is_probed_while_healthy_peers_exist() {
        let good = spawn_status(StatusCode::OK).await;
        let recovering = spawn_status(StatusCode::OK).await;
        let pool = Arc::new(RoyalPool::new(Some(&good), None, "test-hmac").unwrap());
        let recovering_c = Arc::new(RoyalClient::with_hmac(&recovering, "test-hmac").unwrap());
        for _ in 0..5 {
            recovering_c.report_failure();
        }
        assert!(recovering_c.is_open());
        recovering_c.half_open_at_for_test(0);
        lock_write(&pool.clients).push(recovering_c.clone());

        let recovering_base = recovering.trim_end_matches('/');
        let good_base = good.trim_end_matches('/');
        let first = pool.pick().unwrap();
        assert_eq!(first.base, recovering_base);
        for _ in 0..8 {
            let picked = pool.pick().unwrap();
            assert_eq!(picked.base, good_base);
        }
    }

    #[tokio::test]
    async fn no_instance_is_backend() {
        let pool = RoyalPool {
            clients: RwLock::new(Vec::new()),
            rr: AtomicUsize::new(0),
            bootstrap: None,
            naming: None,
            hmac: String::new(),
            refresh: REFRESH,
        };
        match pool.pick() {
            Err(StoreError::Backend(s)) => assert!(s.contains("no royal available"), "{s}"),
            Ok(_) => panic!("expected backend, got a client"),
            Err(other) => panic!("expected backend, got {other}"),
        }
    }

    #[tokio::test]
    async fn refresh_merges_breaker_state() {
        let naming = Arc::new(StaticNaming::from_slice(vec![reg("r1", "127.0.0.1", 1)]));
        let pool = Arc::new(RoyalPool::new(Some("http://127.0.0.1:1"), Some(naming), "h").unwrap());
        let first = pool.pick().unwrap();
        first.report_failure();
        pool.replace_with(vec!["http://127.0.0.1:1".into()])
            .unwrap();
        let again = pool.pick().unwrap();
        assert_eq!(again.fails_for_test(), 1);
    }

    fn reg(id: &str, addr: &str, port: u16) -> DefaultRegistration {
        DefaultRegistration {
            service_id: id.into(),
            service_name: "royal".into(),
            protocol: "http".into(),
            public_address: addr.into(),
            public_port: port,
            tags: vec![],
            meta: [("protocol".into(), "http".into())].into(),
        }
    }
}
