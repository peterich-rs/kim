//! Royal account HTTP: register / login / logout / change-password.
//!
//! Bodies are uncompressed protobuf (`Content-Type: application/x-protobuf`).
//! Gzip is only for *responses*: reqwest sends `Accept-Encoding: gzip` and
//! decodes. Royal/axum do not decompress request `Content-Encoding`, and
//! `AuthReq` is tens of bytes — compressing it would add overhead. Caddy
//! `encode gzip zstd` may gzip larger responses at the edge.

use std::time::Duration;

use kim_protocol::pkt::{AuthReq, AuthResp, PasswordChangeReq, PasswordKeyResp};
use kim_protocol::{AccountId, PasswordSealPublic, PASSWORD_SEAL_ALG};
use prost::Message;
use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_TYPE,
};
use reqwest::StatusCode;

use crate::config::{DEFAULT_LOCAL_HTTP_ORIGIN, DEFAULT_PROD_HTTP_ORIGIN};
use crate::ClientError;

const CONTENT_PROTOBUF: &str = "application/x-protobuf";
const PASSWORD_MIN: usize = 8;
const PASSWORD_MAX: usize = 128;
const SEAL_KEY_TTL: Duration = Duration::from_secs(300);

#[derive(Clone)]
struct CachedSealKey {
    key: PasswordSealPublic,
    fetched_at: std::time::Instant,
}

/// Reject non-localhost `http://` Royal origins (TLS still required in production).
pub fn require_secure_auth_origin(base: &str) -> Result<(), ClientError> {
    let base = base.trim().trim_end_matches('/');
    if base.starts_with("https://") {
        return Ok(());
    }
    if is_loopback_http(base) {
        return Ok(());
    }
    Err(ClientError::InsecureOrigin)
}

fn is_loopback_http(base: &str) -> bool {
    let Some(rest) = base.strip_prefix("http://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split('%').next().unwrap_or(host); // drop zone id
    let host = if let Some(h) = host.strip_prefix('[') {
        h.split(']').next().unwrap_or(h)
    } else {
        host.split(':').next().unwrap_or(host)
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1" | "0:0:0:0:0:0:0:1")
}

/// JWT issued by Royal `/api/v1/auth/{register,login}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthSession {
    pub token: String,
    pub exp: i64,
    pub account: String,
}

/// HTTP client for the public Royal auth surface.
#[derive(Clone)]
pub struct AuthClient {
    http: reqwest::Client,
    base: String,
    seal_cache: std::sync::Arc<tokio::sync::Mutex<Option<CachedSealKey>>>,
}

impl AuthClient {
    /// Build a client. `user_agent` is sent on every request.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Other`] when `user_agent` is empty or not ASCII,
    /// or when the TLS stack cannot be initialized.
    pub fn new(
        base_url: impl Into<String>,
        user_agent: impl Into<String>,
    ) -> Result<Self, ClientError> {
        Self::with_timeout(base_url, user_agent, Duration::from_secs(15))
    }

    pub fn production(user_agent: impl Into<String>) -> Result<Self, ClientError> {
        Self::new(DEFAULT_PROD_HTTP_ORIGIN, user_agent)
    }

    pub fn local(user_agent: impl Into<String>) -> Result<Self, ClientError> {
        Self::new(DEFAULT_LOCAL_HTTP_ORIGIN, user_agent)
    }

    fn with_timeout(
        base_url: impl Into<String>,
        user_agent: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, ClientError> {
        let base = base_url.into().trim().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(ClientError::other("empty auth origin"));
        }
        require_secure_auth_origin(&base)?;
        let user_agent = user_agent.into();
        let user_agent = valid_header_value(&user_agent, "invalid user-agent")?;
        let mut default_headers = HeaderMap::new();
        default_headers.insert(ACCEPT, HeaderValue::from_static(CONTENT_PROTOBUF));
        default_headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
        );
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .default_headers(default_headers)
            .timeout(timeout)
            .gzip(true)
            .build()
            .map_err(|e| ClientError::other(e.to_string()))?;
        Ok(Self {
            http,
            base,
            seal_cache: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    pub async fn register(
        &self,
        account: &str,
        password: &str,
    ) -> Result<AuthSession, ClientError> {
        self.post_auth("/api/v1/auth/register", account, password, true)
            .await
    }

    pub async fn login(&self, account: &str, password: &str) -> Result<AuthSession, ClientError> {
        self.post_auth("/api/v1/auth/login", account, password, true)
            .await
    }

    /// `204` and `401` both succeed: the token is already unusable.
    pub async fn logout(&self, token: &str) -> Result<(), ClientError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(ClientError::InvalidToken);
        }
        let resp = self
            .http
            .post(format!("{}/api/v1/auth/logout", self.base))
            .header(AUTHORIZATION, bearer(token)?)
            .send()
            .await
            .map_err(|e| ClientError::other(e.to_string()))?;
        let status = resp.status();
        if status == StatusCode::NO_CONTENT || status == StatusCode::UNAUTHORIZED {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        Err(http_err(status, body))
    }

    pub async fn change_password(
        &self,
        token: &str,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), ClientError> {
        self.change_password_inner(token, old_password, new_password, true)
            .await
    }

    async fn change_password_inner(
        &self,
        token: &str,
        old_password: &str,
        new_password: &str,
        retry_key: bool,
    ) -> Result<(), ClientError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(ClientError::InvalidToken);
        }
        let old_password = valid_password(old_password)?;
        let new_password = valid_password(new_password)?;
        let seal = self.seal_key().await?;
        let (old_plain, old_sealed, new_plain, new_sealed, key_id) = match seal {
            Some(k) => {
                let old_sealed = k
                    .seal(old_password.as_bytes())
                    .map_err(|_| ClientError::PasswordSeal)?;
                let new_sealed = k
                    .seal(new_password.as_bytes())
                    .map_err(|_| ClientError::PasswordSeal)?;
                (
                    String::new(),
                    old_sealed,
                    String::new(),
                    new_sealed,
                    k.key_id.clone(),
                )
            }
            None => (
                old_password.to_string(),
                Vec::new(),
                new_password.to_string(),
                Vec::new(),
                String::new(),
            ),
        };
        let body = PasswordChangeReq {
            old_password: old_plain,
            new_password: new_plain,
            old_password_sealed: old_sealed,
            new_password_sealed: new_sealed,
            key_id,
        }
        .encode_to_vec();
        let resp = self
            .http
            .post(format!("{}/api/v1/auth/password", self.base))
            .header(AUTHORIZATION, bearer(token)?)
            .header(CONTENT_TYPE, CONTENT_PROTOBUF)
            .body(body)
            .send()
            .await
            .map_err(|e| ClientError::other(e.to_string()))?;
        let status = resp.status();
        if status == StatusCode::NO_CONTENT || status.is_success() {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        let err = http_err(status, body);
        if retry_key && err.is_password_key_id_mismatch() {
            self.invalidate_seal_cache().await;
            return Box::pin(self.change_password_inner(token, old_password, new_password, false))
                .await;
        }
        Err(err)
    }

    async fn post_auth(
        &self,
        path: &str,
        account: &str,
        password: &str,
        retry_key: bool,
    ) -> Result<AuthSession, ClientError> {
        let account = valid_account(account)?;
        let password = valid_password(password)?;
        let seal = self.seal_key().await?;
        let body = match seal {
            Some(k) => {
                let password_sealed = k
                    .seal(password.as_bytes())
                    .map_err(|_| ClientError::PasswordSeal)?;
                AuthReq {
                    account: account.to_string(),
                    password: String::new(),
                    password_sealed,
                    key_id: k.key_id.clone(),
                    ..Default::default()
                }
            }
            None => AuthReq {
                account: account.to_string(),
                password: password.to_string(),
                ..Default::default()
            },
        }
        .encode_to_vec();
        let resp = self
            .http
            .post(format!("{}{path}", self.base))
            .header(CONTENT_TYPE, CONTENT_PROTOBUF)
            .body(body)
            .send()
            .await
            .map_err(|e| ClientError::other(e.to_string()))?;
        let status = resp.status();
        let buf = resp
            .bytes()
            .await
            .map_err(|e| ClientError::other(e.to_string()))?;
        if !status.is_success() {
            let text = String::from_utf8_lossy(&buf).into_owned();
            let err = http_err(status, text);
            if retry_key && err.is_password_key_id_mismatch() {
                self.invalidate_seal_cache().await;
                return Box::pin(self.post_auth(path, account, password, false)).await;
            }
            return Err(err);
        }
        let decoded =
            AuthResp::decode(buf.as_ref()).map_err(|e| ClientError::other(e.to_string()))?;
        if decoded.token.is_empty() {
            return Err(ClientError::InvalidToken);
        }
        let account = if decoded.account.is_empty() {
            account.to_string()
        } else {
            decoded.account
        };
        Ok(AuthSession {
            token: decoded.token,
            exp: decoded.exp,
            account,
        })
    }

    async fn invalidate_seal_cache(&self) {
        *self.seal_cache.lock().await = None;
    }

    async fn seal_key(&self) -> Result<Option<PasswordSealPublic>, ClientError> {
        {
            let guard = self.seal_cache.lock().await;
            if let Some(cached) = guard.as_ref() {
                if cached.fetched_at.elapsed() < SEAL_KEY_TTL {
                    return Ok(Some(cached.key.clone()));
                }
            }
        }
        let resp = self
            .http
            .get(format!("{}/api/v1/auth/password-key", self.base))
            .send()
            .await
            .map_err(|e| ClientError::other(e.to_string()))?;
        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let buf = resp
            .bytes()
            .await
            .map_err(|e| ClientError::other(e.to_string()))?;
        if !status.is_success() {
            let text = String::from_utf8_lossy(&buf).into_owned();
            return Err(http_err(status, text));
        }
        let decoded =
            PasswordKeyResp::decode(buf.as_ref()).map_err(|e| ClientError::other(e.to_string()))?;
        if decoded.alg != PASSWORD_SEAL_ALG {
            return Err(ClientError::PasswordSeal);
        }
        let key = PasswordSealPublic::from_b64(decoded.key_id, &decoded.public_key_b64)
            .map_err(|_| ClientError::PasswordSeal)?;
        let mut guard = self.seal_cache.lock().await;
        *guard = Some(CachedSealKey {
            key: key.clone(),
            fetched_at: std::time::Instant::now(),
        });
        Ok(Some(key))
    }
}

fn valid_account(raw: &str) -> Result<&str, ClientError> {
    AccountId::parse(raw).map_err(|_| ClientError::InvalidAccount)?;
    Ok(raw.trim())
}

fn valid_password(raw: &str) -> Result<&str, ClientError> {
    if raw.len() < PASSWORD_MIN || raw.len() > PASSWORD_MAX {
        return Err(ClientError::InvalidPassword);
    }
    Ok(raw)
}

fn valid_header_value<'a>(raw: &'a str, err: &'static str) -> Result<&'a str, ClientError> {
    let s = raw.trim();
    if s.is_empty() || !s.bytes().all(|b| (32..=126).contains(&b)) {
        return Err(ClientError::other(err));
    }
    Ok(s)
}

fn bearer(token: &str) -> Result<HeaderValue, ClientError> {
    let value = format!("Bearer {token}");
    HeaderValue::from_str(&value).map_err(|_| ClientError::InvalidToken)
}

fn http_err(status: StatusCode, body: String) -> ClientError {
    ClientError::Http {
        status: status.as_u16(),
        body: body.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::body::Bytes;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::Router;
    use prost::Message;

    use crate::config::DEFAULT_CLIENT_USER_AGENT;

    use super::*;

    #[derive(Clone, Default)]
    struct Seen {
        user_agent: String,
        accept: String,
        accept_encoding: String,
        content_type: String,
        content_encoding: String,
        authorization: String,
        body: Vec<u8>,
    }

    async fn capture(
        State(seen): State<Arc<Mutex<Seen>>>,
        headers: HeaderMap,
        body: Bytes,
    ) -> (StatusCode, Bytes) {
        let mut g = seen.lock().unwrap_or_else(|e| e.into_inner());
        g.user_agent = headers
            .get(reqwest::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        g.accept = headers
            .get(ACCEPT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        g.accept_encoding = headers
            .get(reqwest::header::ACCEPT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        g.content_type = headers
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        g.content_encoding = headers
            .get(reqwest::header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        g.authorization = headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        g.body = body.to_vec();
        let resp = AuthResp {
            token: "tok.jwt".into(),
            exp: 99,
            account: "alice".into(),
            ..Default::default()
        };
        (StatusCode::OK, Bytes::from(resp.encode_to_vec()))
    }

    async fn capture_logout(
        State(seen): State<Arc<Mutex<Seen>>>,
        headers: HeaderMap,
    ) -> StatusCode {
        let mut g = seen.lock().unwrap_or_else(|e| e.into_inner());
        g.user_agent = headers
            .get(reqwest::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        g.authorization = headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        g.accept_encoding = headers
            .get(reqwest::header::ACCEPT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        StatusCode::NO_CONTENT
    }

    async fn serve(seen: Arc<Mutex<Seen>>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let app = Router::new()
            .route("/api/v1/auth/register", post(capture))
            .route("/api/v1/auth/login", post(capture))
            .route("/api/v1/auth/logout", post(capture_logout))
            .route("/api/v1/auth/password", post(capture_logout))
            .with_state(seen);
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn login_sends_ua_protobuf_and_accepts_gzip_without_request_gzip() {
        let seen = Arc::new(Mutex::new(Seen::default()));
        let base = serve(seen.clone()).await;
        let ua = "KIM/1.0.0 (Android; build 1)";
        let client = AuthClient::new(&base, ua).expect("client");
        let session = client.login("alice", "secret123").await.expect("login");
        assert_eq!(session.token, "tok.jwt");
        assert_eq!(session.account, "alice");
        let g = seen.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(g.user_agent, ua);
        assert_eq!(g.accept, CONTENT_PROTOBUF);
        assert_eq!(g.content_type, CONTENT_PROTOBUF);
        assert!(
            g.accept_encoding.contains("gzip"),
            "accept-encoding={}",
            g.accept_encoding
        );
        assert!(
            g.content_encoding.is_empty(),
            "request body must not be gzip: {}",
            g.content_encoding
        );
        let req = AuthReq::decode(g.body.as_slice()).expect("pb");
        assert_eq!(req.account, "alice");
        assert_eq!(req.password, "secret123");
    }

    #[tokio::test]
    async fn register_and_logout_and_password_use_same_headers() {
        let seen = Arc::new(Mutex::new(Seen::default()));
        let base = serve(seen.clone()).await;
        let ua = "KIM/1.0.0 (iOS; build 2)";
        let client = AuthClient::new(&base, ua).expect("client");
        let session = client
            .register("bob_1", "secret123")
            .await
            .expect("register");
        assert_eq!(session.token, "tok.jwt");
        client.logout(&session.token).await.expect("logout");
        {
            let g = seen.lock().unwrap_or_else(|e| e.into_inner());
            assert_eq!(g.user_agent, ua);
            assert_eq!(g.authorization, "Bearer tok.jwt");
            assert!(g.accept_encoding.contains("gzip"));
        }
        client
            .change_password(&session.token, "secret123", "secret456")
            .await
            .expect("password");
    }

    #[tokio::test]
    async fn rejects_invalid_input_before_http() {
        let client = AuthClient::new("http://127.0.0.1:9", DEFAULT_CLIENT_USER_AGENT).expect("c");
        assert!(matches!(
            client.login("ab", "secret123").await,
            Err(ClientError::InvalidAccount)
        ));
        assert!(matches!(
            client.login("alice", "short").await,
            Err(ClientError::InvalidPassword)
        ));
        assert!(matches!(
            client.logout("").await,
            Err(ClientError::InvalidToken)
        ));
        assert!(AuthClient::new("http://127.0.0.1:9", "").is_err());
    }

    #[tokio::test]
    async fn maps_http_error_body() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let app = Router::new().route(
            "/api/v1/auth/login",
            post(|| async { (StatusCode::UNAUTHORIZED, "账号或密码错误") }),
        );
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let client =
            AuthClient::new(format!("http://{addr}"), DEFAULT_CLIENT_USER_AGENT).expect("client");
        let err = client.login("alice", "secret123").await.expect_err("401");
        match err {
            ClientError::Http { status, body } => {
                assert_eq!(status, 401);
                assert!(body.contains("账号或密码错误"));
            }
            other => panic!("{other}"),
        }
    }

    #[test]
    fn rejects_insecure_non_localhost_http_origin() {
        assert!(matches!(
            require_secure_auth_origin("http://evil.example/api"),
            Err(ClientError::InsecureOrigin)
        ));
        assert!(require_secure_auth_origin("https://kim.ainexc.com").is_ok());
        assert!(require_secure_auth_origin("http://127.0.0.1:8080").is_ok());
        assert!(require_secure_auth_origin("http://localhost:8080").is_ok());
        assert!(require_secure_auth_origin("http://[::1]:8080").is_ok());
        assert!(require_secure_auth_origin("http://localhost.evil.com").is_err());
        assert!(require_secure_auth_origin("http://user@localhost:8080").is_ok());
        assert!(AuthClient::new("http://evil.example", DEFAULT_CLIENT_USER_AGENT).is_err());
    }

    #[tokio::test]
    async fn login_seals_password_when_server_publishes_key() {
        use axum::routing::get;
        use kim_protocol::PasswordSealKey;

        let key = PasswordSealKey::generate(Some("k-test".into()));
        let key_resp = PasswordKeyResp {
            key_id: key.key_id().to_string(),
            alg: key.alg().to_string(),
            public_key_b64: key.public_key_b64(),
        }
        .encode_to_vec();
        let seen = Arc::new(Mutex::new(Seen::default()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let app = Router::new()
            .route(
                "/api/v1/auth/password-key",
                get(move || {
                    let body = key_resp.clone();
                    async move { (StatusCode::OK, Bytes::from(body)) }
                }),
            )
            .route("/api/v1/auth/login", post(capture))
            .with_state(seen.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let client =
            AuthClient::new(format!("http://{addr}"), DEFAULT_CLIENT_USER_AGENT).expect("client");
        let session = client.login("alice", "secret123").await.expect("login");
        assert_eq!(session.token, "tok.jwt");
        let g = seen.lock().unwrap_or_else(|e| e.into_inner());
        let req = AuthReq::decode(g.body.as_slice()).expect("pb");
        assert!(req.password.is_empty(), "must not send plaintext");
        assert_eq!(req.key_id, "k-test");
        assert!(!req.password_sealed.is_empty());
        let opened = key.unseal(&req.password_sealed).expect("unseal");
        assert_eq!(opened, b"secret123");
    }

    #[tokio::test]
    async fn login_refetches_seal_key_on_key_id_mismatch() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        use axum::routing::get;
        use kim_protocol::{PasswordSealKey, PASSWORD_KEY_ID_MISMATCH};

        let key_old = PasswordSealKey::generate(Some("old".into()));
        let key_new = PasswordSealKey::generate(Some("new".into()));
        let old_resp = PasswordKeyResp {
            key_id: key_old.key_id().to_string(),
            alg: key_old.alg().to_string(),
            public_key_b64: key_old.public_key_b64(),
        }
        .encode_to_vec();
        let new_resp = PasswordKeyResp {
            key_id: key_new.key_id().to_string(),
            alg: key_new.alg().to_string(),
            public_key_b64: key_new.public_key_b64(),
        }
        .encode_to_vec();
        let fetches = Arc::new(AtomicUsize::new(0));
        let seen = Arc::new(Mutex::new(Seen::default()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let fetches_key = fetches.clone();
        let app = Router::new()
            .route(
                "/api/v1/auth/password-key",
                get(move || {
                    let n = fetches_key.fetch_add(1, Ordering::SeqCst);
                    let body = if n == 0 {
                        old_resp.clone()
                    } else {
                        new_resp.clone()
                    };
                    async move { (StatusCode::OK, Bytes::from(body)) }
                }),
            )
            .route(
                "/api/v1/auth/login",
                post(
                    |State(seen): State<Arc<Mutex<Seen>>>, body: Bytes| async move {
                        let req = AuthReq::decode(body.as_ref()).expect("pb");
                        {
                            let mut g = seen.lock().unwrap_or_else(|e| e.into_inner());
                            g.body = body.to_vec();
                        }
                        if req.key_id == "old" {
                            return (
                                StatusCode::BAD_REQUEST,
                                Bytes::from(PASSWORD_KEY_ID_MISMATCH),
                            );
                        }
                        let resp = AuthResp {
                            token: "tok.jwt".into(),
                            exp: 99,
                            account: "alice".into(),
                            ..Default::default()
                        };
                        (StatusCode::OK, Bytes::from(resp.encode_to_vec()))
                    },
                ),
            )
            .with_state(seen.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let client =
            AuthClient::new(format!("http://{addr}"), DEFAULT_CLIENT_USER_AGENT).expect("client");
        let session = client.login("alice", "secret123").await.expect("login");
        assert_eq!(session.token, "tok.jwt");
        assert_eq!(fetches.load(Ordering::SeqCst), 2);
        let g = seen.lock().unwrap_or_else(|e| e.into_inner());
        let req = AuthReq::decode(g.body.as_slice()).expect("pb");
        assert_eq!(req.key_id, "new");
        let opened = key_new.unseal(&req.password_sealed).expect("unseal");
        assert_eq!(opened, b"secret123");
    }
}
