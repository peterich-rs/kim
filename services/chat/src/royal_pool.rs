//! Royal instance pool: round-robin, per-instance circuit breaker, Consul refresh.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use bytes::Bytes;
use kim_metrics::KimMetrics;
use kim_naming::{DefaultRegistration, Naming};
use prost::Message;

use crate::royal::{
    attempt_timeout, backoff, circuit_failure_status, http_status_err, retry_http, RoyalClient,
    RETRIES,
};
use crate::store::StoreError;

const REFRESH: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RpcCause {
    NoClient,
    Http,
    Transport,
    Decode,
}

impl RpcCause {
    fn as_str(self) -> &'static str {
        match self {
            Self::NoClient => "no_client",
            Self::Http => "http",
            Self::Transport => "transport",
            Self::Decode => "decode",
        }
    }
}

pub struct RoyalPool {
    clients: RwLock<Vec<Arc<RoyalClient>>>,
    rr: AtomicUsize,
    bootstrap: Option<Arc<RoyalClient>>,
    naming: Option<Arc<dyn Naming>>,
    hmac: String,
    refresh: Duration,
    metrics: Option<Arc<KimMetrics>>,
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
        Self::with_metrics(royal_url, naming, hmac, None)
    }

    pub fn with_metrics(
        royal_url: Option<&str>,
        naming: Option<Arc<dyn Naming>>,
        hmac: &str,
        metrics: Option<Arc<KimMetrics>>,
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
            metrics,
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

    pub(crate) async fn send_pb<T: Message + Default, B: Message>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, StoreError> {
        let started = Instant::now();
        let group = classify(path);
        let out = self.send_pb_inner(method, path, body).await;
        self.observe_rpc(group, started, &out);
        out.map_err(|(_, e)| e)
    }

    /// Decode-empty-tolerant POST for ack/join/quit.
    pub(crate) async fn post_maybe_empty(
        &self,
        path: &str,
        body: &impl Message,
    ) -> Result<(), StoreError> {
        let started = Instant::now();
        let group = classify(path);
        let out = self.post_maybe_empty_inner(path, body).await;
        self.observe_rpc(group, started, &out);
        out.map_err(|(_, e)| e)
    }

    fn observe_rpc<T>(
        &self,
        group: &'static str,
        started: Instant,
        out: &Result<T, (RpcCause, StoreError)>,
    ) {
        let Some(m) = &self.metrics else {
            return;
        };
        match out {
            Ok(_) => m.observe_royal_rpc(group, started.elapsed()),
            Err((cause, _)) => m.on_royal_rpc_error(group, cause.as_str()),
        }
    }

    async fn send_pb_inner<T: Message + Default, B: Message>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, (RpcCause, StoreError)> {
        let bytes = body.map(|b| Bytes::from(b.encode_to_vec()));
        let payload = bytes.as_deref().unwrap_or(&[]);
        let mut last = (
            RpcCause::Transport,
            StoreError::Backend("royal request failed".into()),
        );
        for attempt in 0..RETRIES {
            if attempt_timeout().is_zero() {
                break;
            }
            let client = match self.pick() {
                Ok(c) => c,
                Err(e) => return Err((RpcCause::NoClient, e)),
            };
            let req = match client.signed(method.clone(), path, payload) {
                Ok(r) => r.timeout(attempt_timeout()),
                Err(e) => return Err((RpcCause::Transport, e)),
            };
            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let buf = match resp.bytes().await {
                        Ok(b) => b,
                        Err(e) => {
                            self.report_failure(&client);
                            last = (RpcCause::Transport, StoreError::Backend(e.to_string()));
                            if attempt + 1 < RETRIES {
                                backoff(attempt).await;
                            }
                            continue;
                        }
                    };
                    if status.is_success() {
                        self.report_success(&client);
                        return T::decode(buf.as_ref())
                            .map_err(|e| (RpcCause::Decode, StoreError::Backend(e.to_string())));
                    }
                    last = (RpcCause::Http, http_status_err(status, &buf));
                    if circuit_failure_status(status) {
                        self.report_failure(&client);
                    } else {
                        self.report_success(&client);
                    }
                    if !retry_http(status) {
                        return Err(last);
                    }
                }
                Err(err) => {
                    self.report_failure(&client);
                    last = (RpcCause::Transport, StoreError::Backend(err.to_string()));
                }
            }
            if attempt + 1 < RETRIES {
                backoff(attempt).await;
            }
        }
        Err(last)
    }

    async fn post_maybe_empty_inner(
        &self,
        path: &str,
        body: &impl Message,
    ) -> Result<(), (RpcCause, StoreError)> {
        let bytes = Bytes::from(body.encode_to_vec());
        let mut last = (
            RpcCause::Transport,
            StoreError::Backend("royal request failed".into()),
        );
        for attempt in 0..RETRIES {
            if attempt_timeout().is_zero() {
                break;
            }
            let client = match self.pick() {
                Ok(c) => c,
                Err(e) => return Err((RpcCause::NoClient, e)),
            };
            let req = match client.signed(reqwest::Method::POST, path, &bytes) {
                Ok(r) => r.timeout(attempt_timeout()),
                Err(e) => return Err((RpcCause::Transport, e)),
            };
            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let buf = match resp.bytes().await {
                        Ok(b) => b,
                        Err(e) => {
                            self.report_failure(&client);
                            last = (RpcCause::Transport, StoreError::Backend(e.to_string()));
                            if attempt + 1 < RETRIES {
                                backoff(attempt).await;
                            }
                            continue;
                        }
                    };
                    if status.is_success() {
                        self.report_success(&client);
                        return Ok(());
                    }
                    last = (RpcCause::Http, http_status_err(status, &buf));
                    if circuit_failure_status(status) {
                        self.report_failure(&client);
                    } else {
                        self.report_success(&client);
                    }
                    if !retry_http(status) {
                        return Err(last);
                    }
                }
                Err(err) => {
                    self.report_failure(&client);
                    last = (RpcCause::Transport, StoreError::Backend(err.to_string()));
                }
            }
            if attempt + 1 < RETRIES {
                backoff(attempt).await;
            }
        }
        Err(last)
    }
}

fn classify(path: &str) -> &'static str {
    const PREFIXES: &[(&str, &str)] = &[
        ("/api/v1/message", "message"),
        ("/api/v1/group", "group"),
        ("/api/v1/friend", "friend"),
        ("/api/v1/user", "user"),
        ("/api/v1/block", "block"),
        ("/api/v1/offline", "offline"),
        ("/api/v1/delivery", "delivery"),
        ("/api/v1/inbox", "inbox"),
        ("/api/v1/history", "history"),
        ("/internal", "internal"),
    ];
    for (prefix, group) in PREFIXES {
        if path.starts_with(prefix) {
            return group;
        }
    }
    "other"
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
    use kim_metrics::KimMetrics;
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
            metrics: None,
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

    fn scrape(m: &KimMetrics) -> String {
        m.scrape_text().expect("encode")
    }

    #[test]
    fn classify_covers_router_hmac_paths() {
        const HMAC_PATHS: &[&str] = &[
            "/internal/user/lookup",
            "/internal/user/upsert",
            "/internal/revoke/check",
            "/internal/token-epoch",
            "/internal/device/check",
            "/api/v1/message/user",
            "/api/v1/message/group",
            "/api/v1/message/ack",
            "/api/v1/offline/index",
            "/api/v1/delivery/backfill",
            "/api/v1/offline/content",
            "/api/v1/group",
            "/api/v1/group/member",
            "/api/v1/group/quit",
            "/api/v1/group/members",
            "/api/v1/group/detail",
            "/api/v1/user/profile",
            "/api/v1/user/update",
            "/api/v1/user/profiles",
            "/api/v1/user/search",
            "/api/v1/friend/request",
            "/api/v1/friend/accept",
            "/api/v1/friend/reject",
            "/api/v1/friend/remove",
            "/api/v1/friend/list",
            "/api/v1/friend/incoming",
            "/api/v1/friend/check",
            "/api/v1/block/add",
            "/api/v1/block/remove",
            "/api/v1/block/list",
            "/api/v1/block/check",
            "/api/v1/inbox",
            "/api/v1/history",
            "/api/v1/inbox/read",
        ];
        for path in HMAC_PATHS {
            assert_ne!(classify(path), "other", "{path}");
        }
        assert_eq!(classify("/health"), "other");
        assert_eq!(classify("/api/v1/auth/login"), "other");
        assert_eq!(classify("/api/v1/group/summaries"), "group");
    }

    #[tokio::test]
    async fn rpc_success_observes_once() {
        let good = spawn_status(StatusCode::OK).await;
        let metrics = KimMetrics::new("t", "chat").unwrap();
        let pool =
            RoyalPool::with_metrics(Some(&good), None, "test-hmac", Some(metrics.clone())).unwrap();
        let body = kim_protocol::pkt::AccountPair {
            account: "a".into(),
            peer: "b".into(),
        };
        pool.send_pb::<AccountExists, _>(
            reqwest::Method::POST,
            "/api/v1/friend/check",
            Some(&body),
        )
        .await
        .unwrap();
        let body = scrape(&metrics);
        assert!(
            body.contains("kim_royal_rpc_seconds_count{path_group=\"friend\"} 1"),
            "{body}"
        );
        assert!(
            !body.contains("kim_royal_rpc_errors_total{path_group=\"friend\""),
            "{body}"
        );
    }

    #[tokio::test]
    async fn rpc_503_records_http() {
        let bad = spawn_status(StatusCode::SERVICE_UNAVAILABLE).await;
        let metrics = KimMetrics::new("t", "chat").unwrap();
        let pool =
            RoyalPool::with_metrics(Some(&bad), None, "test-hmac", Some(metrics.clone())).unwrap();
        let body = kim_protocol::pkt::AccountPair {
            account: "a".into(),
            peer: "b".into(),
        };
        let err = pool
            .send_pb::<AccountExists, _>(reqwest::Method::POST, "/api/v1/friend/check", Some(&body))
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::Http { status: 503, .. }));
        let body = scrape(&metrics);
        assert!(
            body.contains("kim_royal_rpc_errors_total{cause=\"http\",path_group=\"friend\"} 1")
                || body
                    .contains("kim_royal_rpc_errors_total{path_group=\"friend\",cause=\"http\"} 1"),
            "{body}"
        );
    }

    #[tokio::test]
    async fn rpc_no_instance_records_no_client() {
        let metrics = KimMetrics::new("t", "chat").unwrap();
        let pool = RoyalPool {
            clients: RwLock::new(Vec::new()),
            rr: AtomicUsize::new(0),
            bootstrap: None,
            naming: None,
            hmac: String::new(),
            refresh: REFRESH,
            metrics: Some(metrics.clone()),
        };
        let body = kim_protocol::pkt::AccountPair {
            account: "a".into(),
            peer: "b".into(),
        };
        let err = pool
            .send_pb::<AccountExists, _>(reqwest::Method::POST, "/api/v1/friend/check", Some(&body))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no royal available"));
        let body = scrape(&metrics);
        assert!(
            body.contains("cause=\"no_client\"") && body.contains("path_group=\"friend\""),
            "{body}"
        );
        assert!(!body.contains("cause=\"transport\""), "{body}");
    }

    #[tokio::test]
    async fn rpc_bad_protobuf_records_decode() {
        async fn junk() -> (StatusCode, Vec<u8>) {
            (StatusCode::OK, b"not-protobuf".to_vec())
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route("/api/v1/friend/check", post(junk));
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let url = format!("http://{addr}");
        let metrics = KimMetrics::new("t", "chat").unwrap();
        let pool =
            RoyalPool::with_metrics(Some(&url), None, "test-hmac", Some(metrics.clone())).unwrap();
        let body = kim_protocol::pkt::AccountPair {
            account: "a".into(),
            peer: "b".into(),
        };
        pool.send_pb::<AccountExists, _>(
            reqwest::Method::POST,
            "/api/v1/friend/check",
            Some(&body),
        )
        .await
        .unwrap_err();
        let body = scrape(&metrics);
        assert!(
            body.contains("cause=\"decode\"") && body.contains("path_group=\"friend\""),
            "{body}"
        );
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
