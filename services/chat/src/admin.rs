//! Loopback HTTP for Royal: kick a live session via the existing Kickout path.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use http_body_util::BodyExt;
use kim_protocol::pkt::{Flag, KickAccount, KickoutNotify};
use kim_protocol::{hmac_headers_from, verify_internal_hmac, LogicPkt, CMD_LOGIN_SIGN_IN};
use kim_router::{Dispatcher, SessionError, SessionStorage};
use prost::Message;
use tracing::{info, warn};

use crate::hmac_nonce::HmacNonceGuard;

#[derive(Clone)]
pub struct ChatAdmin {
    cache: Arc<dyn SessionStorage>,
    dispatcher: Arc<dyn Dispatcher>,
    hmac_secret: String,
    nonce: Arc<dyn HmacNonceGuard>,
}

impl ChatAdmin {
    pub fn new(
        cache: Arc<dyn SessionStorage>,
        dispatcher: Arc<dyn Dispatcher>,
        hmac_secret: impl Into<String>,
        nonce: Arc<dyn HmacNonceGuard>,
    ) -> Self {
        Self {
            cache,
            dispatcher,
            hmac_secret: hmac_secret.into(),
            nonce,
        }
    }

    pub async fn kick(&self, account: &str) -> Result<bool, SessionError> {
        let locs = match self.cache.list_locations(account).await {
            Ok(v) => v,
            Err(SessionError::NotFound) => return Ok(false),
            Err(err) => return Err(err),
        };
        if locs.is_empty() {
            return Ok(false);
        }
        for loc in &locs {
            let mut pkt = LogicPkt::new(CMD_LOGIN_SIGN_IN, 0, Bytes::new());
            pkt.header.flag = Flag::Push as i32;
            pkt.write_body(&KickoutNotify {
                channel_id: loc.channel_id.clone(),
            });
            if let Err(err) = self
                .dispatcher
                .push(&loc.gate_id, std::slice::from_ref(&loc.channel_id), pkt)
                .await
            {
                warn!(%err, account, "kick dispatch failed");
                return Err(SessionError::Other(err.to_string()));
            }
            self.cache.delete(account, &loc.channel_id).await?;
            info!(account, channel = %loc.channel_id, "kicked");
        }
        Ok(true)
    }
}

fn hmac_unauthorized() -> (StatusCode, String) {
    (StatusCode::UNAUTHORIZED, "unauthorized".into())
}

fn header_str<'a>(headers: &'a axum::http::HeaderMap, name: &'static str) -> &'a str {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

async fn require_kick_hmac(
    State(admin): State<ChatAdmin>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let (parts, body) = req.into_parts();
    let collected = body.collect().await.map_err(|_| hmac_unauthorized())?;
    let bytes = collected.to_bytes();
    if bytes.len() > 4 * 1024 * 1024 {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "payload too large".into()));
    }
    let headers = hmac_headers_from(|name| header_str(&parts.headers, name));
    if !verify_internal_hmac(
        admin.hmac_secret.as_bytes(),
        parts.method.as_str(),
        parts.uri.path(),
        &bytes,
        &headers,
    ) {
        return Err(hmac_unauthorized());
    }
    match admin.nonce.claim(&headers.nonce).await {
        Ok(true) => {}
        Ok(false) => return Err(hmac_unauthorized()),
        Err(err) => {
            tracing::error!(%err, "hmac nonce");
            return Err((StatusCode::SERVICE_UNAVAILABLE, "unavailable".into()));
        }
    }
    let req = Request::from_parts(parts, Body::from(bytes));
    Ok(next.run(req).await)
}

async fn kick_handler(
    State(admin): State<ChatAdmin>,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let req =
        KickAccount::decode(body.as_ref()).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if req.account.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty account".into()));
    }
    if !req.app.is_empty() {
        info!(account = %req.account, app = %req.app, "kick request");
    }
    match admin.kick(&req.account).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(err) => {
            warn!(%err, "kick failed");
            Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
        }
    }
}

pub fn router(admin: ChatAdmin) -> Router {
    Router::new()
        .route("/internal/kick", post(kick_handler))
        .route_layer(middleware::from_fn_with_state(
            admin.clone(),
            require_kick_hmac,
        ))
        .with_state(admin)
}

pub async fn serve(listen: SocketAddr, app: Router) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(listen).await?;
    axum::serve(listener, app)
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hmac_nonce::MemoryHmacNonceGuard;
    use async_trait::async_trait;
    use kim_protocol::pkt::Session;
    use kim_protocol::{sign_internal_hmac, sign_internal_hmac_at, MAX_SKEW_SECS};
    use kim_router::test_support::RecordingDispatcher;
    use kim_session::MemorySessionStore;
    use std::time::Duration;

    const SECRET: &[u8] = b"test-hmac-secret-xx";

    struct FailGuard;

    #[async_trait]
    impl HmacNonceGuard for FailGuard {
        async fn claim(&self, _: &str) -> Result<bool, String> {
            Err("redis down".into())
        }
    }

    async fn listen_admin(admin: ChatAdmin) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(admin);
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        format!("http://{addr}")
    }

    async fn seeded() -> (Arc<MemorySessionStore>, Arc<RecordingDispatcher>) {
        let cache = Arc::new(MemorySessionStore::new());
        cache
            .add(&Session {
                channel_id: "wg-1_alice_1".into(),
                gate_id: "wg-1".into(),
                account: "alice".into(),
                app: "kim".into(),
                ..Session::default()
            })
            .await
            .unwrap();
        cache
            .add(&Session {
                channel_id: "wg-1_bob_1".into(),
                gate_id: "wg-1".into(),
                account: "bob".into(),
                app: "kim".into(),
                ..Session::default()
            })
            .await
            .unwrap();
        (cache, Arc::new(RecordingDispatcher::default()))
    }

    fn kick_body(account: &str) -> Vec<u8> {
        KickAccount {
            account: account.into(),
            app: "kim".into(),
        }
        .encode_to_vec()
    }

    fn signed(path: &str, body: &[u8]) -> reqwest::RequestBuilder {
        signed_with(SECRET, path, body)
    }

    fn signed_with(secret: &[u8], path: &str, body: &[u8]) -> reqwest::RequestBuilder {
        let headers = sign_internal_hmac(secret, "POST", path, body).unwrap();
        let mut req = reqwest::Client::new()
            .post("http://placeholder")
            .header("Content-Type", "application/x-protobuf");
        for (k, v) in headers.pairs() {
            req = req.header(k, v);
        }
        req.body(body.to_vec())
    }

    async fn post_kick(base: &str, builder: reqwest::RequestBuilder) -> reqwest::Response {
        let built = builder.build().unwrap();
        let url = format!("{base}/internal/kick");
        let mut req = reqwest::Client::new().post(url);
        for (k, v) in built.headers() {
            req = req.header(k, v);
        }
        if let Some(b) = built.body() {
            req = req.body(b.as_bytes().unwrap().to_vec());
        }
        req.send().await.unwrap()
    }

    #[tokio::test]
    async fn unsigned_kick_is_unauthorized() {
        let (cache, dispatcher) = seeded().await;
        let admin = ChatAdmin::new(
            cache,
            dispatcher,
            String::from_utf8(SECRET.to_vec()).unwrap(),
            Arc::new(MemoryHmacNonceGuard::new()),
        );
        let base = listen_admin(admin).await;
        let resp = reqwest::Client::new()
            .post(format!("{base}/internal/kick"))
            .header("Content-Type", "application/x-protobuf")
            .body(kick_body("alice"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(resp.text().await.unwrap(), "unauthorized");
    }

    #[tokio::test]
    async fn wrong_secret_or_body_is_unauthorized() {
        let (cache, dispatcher) = seeded().await;
        let admin = ChatAdmin::new(
            cache,
            dispatcher,
            String::from_utf8(SECRET.to_vec()).unwrap(),
            Arc::new(MemoryHmacNonceGuard::new()),
        );
        let base = listen_admin(admin).await;
        let body = kick_body("alice");
        let wrong = signed_with(b"other-secret-value!!", "/internal/kick", &body);
        let resp = post_kick(&base, wrong).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let headers = sign_internal_hmac(SECRET, "POST", "/internal/kick", &body).unwrap();
        let tampered = kick_body("bob");
        let mut req = reqwest::Client::new()
            .post(format!("{base}/internal/kick"))
            .header("Content-Type", "application/x-protobuf");
        for (k, v) in headers.pairs() {
            req = req.header(k, v);
        }
        let resp = req.body(tampered).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn expired_timestamp_is_unauthorized() {
        let (cache, dispatcher) = seeded().await;
        let admin = ChatAdmin::new(
            cache,
            dispatcher,
            String::from_utf8(SECRET.to_vec()).unwrap(),
            Arc::new(MemoryHmacNonceGuard::new()),
        );
        let base = listen_admin(admin).await;
        let body = kick_body("alice");
        let headers = sign_internal_hmac_at(SECRET, "POST", "/internal/kick", &body, 1).unwrap();
        let mut req = reqwest::Client::new()
            .post(format!("{base}/internal/kick"))
            .header("Content-Type", "application/x-protobuf");
        for (k, v) in headers.pairs() {
            req = req.header(k, v);
        }
        let resp = req.body(body).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let _ = MAX_SKEW_SECS;
    }

    #[tokio::test]
    async fn replay_same_nonce_is_unauthorized() {
        let (cache, dispatcher) = seeded().await;
        let admin = ChatAdmin::new(
            cache,
            dispatcher.clone(),
            String::from_utf8(SECRET.to_vec()).unwrap(),
            Arc::new(MemoryHmacNonceGuard::new()),
        );
        let base = listen_admin(admin).await;
        let body = kick_body("alice");
        let headers = sign_internal_hmac(SECRET, "POST", "/internal/kick", &body).unwrap();
        let send = || {
            let mut req = reqwest::Client::new()
                .post(format!("{base}/internal/kick"))
                .header("Content-Type", "application/x-protobuf");
            for (k, v) in headers.pairs() {
                req = req.header(k, v);
            }
            req.body(body.clone())
        };
        let first = send().send().await.unwrap();
        assert_eq!(first.status(), StatusCode::NO_CONTENT);
        let second = send().send().await.unwrap();
        assert_eq!(second.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(second.text().await.unwrap(), "unauthorized");
        let pushed = dispatcher.recorded();
        assert_eq!(pushed.len(), 1);
        assert_eq!(pushed[0].channels, vec!["wg-1_alice_1".to_string()]);
    }

    #[tokio::test]
    async fn signed_kick_only_targets_body_account() {
        let (cache, dispatcher) = seeded().await;
        let admin = ChatAdmin::new(
            cache.clone(),
            dispatcher.clone(),
            String::from_utf8(SECRET.to_vec()).unwrap(),
            Arc::new(MemoryHmacNonceGuard::new()),
        );
        let base = listen_admin(admin).await;
        let body = kick_body("alice");
        let resp = post_kick(&base, signed("/internal/kick", &body)).await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        let pushed = dispatcher.recorded();
        assert_eq!(pushed.len(), 1);
        assert_eq!(pushed[0].channels, vec!["wg-1_alice_1".to_string()]);
        assert!(cache.list_locations("bob").await.unwrap().len() == 1);
        assert!(cache.list_locations("alice").await.is_err());
    }

    #[tokio::test]
    async fn nonce_backend_failure_is_unavailable() {
        let (cache, dispatcher) = seeded().await;
        let admin = ChatAdmin::new(
            cache,
            dispatcher.clone(),
            String::from_utf8(SECRET.to_vec()).unwrap(),
            Arc::new(FailGuard),
        );
        let base = listen_admin(admin).await;
        let body = kick_body("alice");
        let resp = post_kick(&base, signed("/internal/kick", &body)).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(dispatcher.recorded().is_empty());
    }
}
