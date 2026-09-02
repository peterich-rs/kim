//! Royal account HTTP: register / login / logout / change-password.
//!
//! Bodies are uncompressed protobuf (`Content-Type: application/x-protobuf`).
//! Gzip is only for *responses*: reqwest sends `Accept-Encoding: gzip` and
//! decodes. Royal/axum do not decompress request `Content-Encoding`, and
//! `AuthReq` is tens of bytes — compressing it would add overhead. Caddy
//! `encode gzip zstd` may gzip larger responses at the edge.

use std::time::Duration;

use kim_protocol::pkt::{AuthReq, AuthResp, PasswordChangeReq};
use prost::Message;
use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_TYPE,
};
use reqwest::StatusCode;

use crate::config::{DEFAULT_LOCAL_HTTP_ORIGIN, DEFAULT_PROD_HTTP_ORIGIN};
use crate::ClientError;

const CONTENT_PROTOBUF: &str = "application/x-protobuf";
const ACCOUNT_MIN: usize = 3;
const ACCOUNT_MAX: usize = 32;
const PASSWORD_MIN: usize = 8;
const PASSWORD_MAX: usize = 128;

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
        Ok(Self { http, base })
    }

    pub async fn register(
        &self,
        account: &str,
        password: &str,
    ) -> Result<AuthSession, ClientError> {
        self.post_auth("/api/v1/auth/register", account, password)
            .await
    }

    pub async fn login(&self, account: &str, password: &str) -> Result<AuthSession, ClientError> {
        self.post_auth("/api/v1/auth/login", account, password)
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
        let token = token.trim();
        if token.is_empty() {
            return Err(ClientError::InvalidToken);
        }
        let old_password = valid_password(old_password)?;
        let new_password = valid_password(new_password)?;
        let body = PasswordChangeReq {
            old_password: old_password.to_string(),
            new_password: new_password.to_string(),
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
        Err(http_err(status, body))
    }

    async fn post_auth(
        &self,
        path: &str,
        account: &str,
        password: &str,
    ) -> Result<AuthSession, ClientError> {
        let account = valid_account(account)?;
        let password = valid_password(password)?;
        let body = AuthReq {
            account: account.to_string(),
            password: password.to_string(),
            ..Default::default()
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
            return Err(http_err(status, text));
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
}

fn valid_account(raw: &str) -> Result<&str, ClientError> {
    let s = raw.trim();
    if s.len() < ACCOUNT_MIN || s.len() > ACCOUNT_MAX {
        return Err(ClientError::InvalidAccount);
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ClientError::InvalidAccount);
    }
    Ok(s)
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
}
