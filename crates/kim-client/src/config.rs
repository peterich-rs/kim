use std::time::Duration;

use kim_core::{DEFAULT_HEARTBEAT, DEFAULT_LOGIN_WAIT};

/// Local WGateway (plaintext WS). Used by `pkt-client` and the web app:local.
pub const DEFAULT_LOCAL_URL: &str = "ws://127.0.0.1:8001/";
/// Production WGateway (WSS). Same host as the product web app.
pub const DEFAULT_PROD_URL: &str = "wss://kim.ainexc.com/";
/// Royal HTTP origin next to local WGateway.
pub const DEFAULT_LOCAL_HTTP_ORIGIN: &str = "http://127.0.0.1:8080";
/// Royal HTTP origin on the product host (Caddy `/api/v1/auth/*`).
pub const DEFAULT_PROD_HTTP_ORIGIN: &str = "https://kim.ainexc.com";
/// `LoginReq.device` fallback. Exclusive with other mobile sessions.
pub const DEFAULT_DEVICE: &str = "mobile";
/// Desktop-class `LoginReq.device`. Coexists with mobile / web / cli.
pub const DESKTOP_DEVICE: &str = "desktop";
/// Fallback User-Agent when the UI does not pass one.
pub const DEFAULT_CLIENT_USER_AGENT: &str = "KIM/0.1 (kim-client)";

/// `LoginReq.device` for a compiled client OS.
///
/// macOS / Windows / Linux report [`DESKTOP_DEVICE`] so they can sit next to a
/// phone. iOS / Android (and anything else) stay [`DEFAULT_DEVICE`].
#[must_use]
pub fn device_for_target_os(os: &str) -> &'static str {
    match os {
        "macos" | "windows" | "linux" => DESKTOP_DEVICE,
        _ => DEFAULT_DEVICE,
    }
}

/// How to reach WGateway. Token is never placed on the Upgrade URL.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub url: String,
    pub token: String,
    pub handshake_timeout: Duration,
    pub user_agent: String,
    /// `LoginReq.device`. See [`device_for_target_os`].
    pub device: String,
    /// Periodic CODE_PING interval (fire-and-forget).
    pub heartbeat: Duration,
    /// Client watchdog: last_read older than this is IdleTimeout.
    pub read_idle: Duration,
    /// Online probe wait-for-Pong budget.
    pub probe_timeout: Duration,
    /// Dart persist-then-ack gate timeout.
    pub confirm_timeout: Duration,
}

impl ClientConfig {
    pub fn new(url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            token: token.into(),
            handshake_timeout: DEFAULT_LOGIN_WAIT,
            user_agent: DEFAULT_CLIENT_USER_AGENT.to_string(),
            device: DEFAULT_DEVICE.to_string(),
            heartbeat: DEFAULT_HEARTBEAT,
            read_idle: DEFAULT_HEARTBEAT * 3,
            probe_timeout: Duration::from_secs(5),
            confirm_timeout: Duration::from_secs(15),
        }
    }

    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        let user_agent = user_agent.into();
        if !user_agent.trim().is_empty() {
            self.user_agent = user_agent;
        }
        self
    }

    #[must_use]
    pub fn with_device(mut self, device: impl Into<String>) -> Self {
        let device = device.into();
        if !device.trim().is_empty() {
            self.device = device;
        }
        self
    }

    pub fn local(token: impl Into<String>) -> Self {
        Self::new(DEFAULT_LOCAL_URL, token)
    }

    pub fn production(token: impl Into<String>) -> Self {
        Self::new(DEFAULT_PROD_URL, token)
    }

    /// `KIM_WS_URL` overrides the URL when set and non-empty.
    pub fn with_env_url(mut self) -> Self {
        if let Ok(url) = std::env::var("KIM_WS_URL") {
            let url = url.trim();
            if !url.is_empty() {
                self.url = url.to_string();
            }
        }
        self
    }
}

/// Map a WGateway URL to the Royal HTTP origin used for `/api/v1/auth/*`.
///
/// Local `ws://127.0.0.1:8001` is Royal `:8080`. Production WSS on
/// `kim.ainexc.com` is `https://kim.ainexc.com`. Other `ws`/`wss` URLs keep
/// host and swap the scheme; gateway port `8001` becomes Royal `8080`.
#[must_use]
pub fn http_origin_from_ws(ws_url: &str) -> String {
    let raw = ws_url.trim();
    if raw.is_empty() || raw.starts_with("wss://kim.ainexc.com") {
        return DEFAULT_PROD_HTTP_ORIGIN.to_string();
    }
    if raw.contains("127.0.0.1:8001") || raw.contains("localhost:8001") {
        return DEFAULT_LOCAL_HTTP_ORIGIN.to_string();
    }
    let http = if let Some(rest) = raw.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = raw.strip_prefix("ws://") {
        format!("http://{rest}")
    } else {
        raw.to_string()
    };
    let Some((scheme, after)) = http.split_once("://") else {
        return http;
    };
    let hostport = after.split('/').next().unwrap_or(after);
    let hostport = hostport
        .strip_suffix(":8001")
        .map(|h| format!("{h}:8080"))
        .unwrap_or_else(|| hostport.to_string());
    format!("{scheme}://{hostport}")
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self::local("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_wgateway_only() {
        assert_eq!(DEFAULT_LOCAL_URL, "ws://127.0.0.1:8001/");
        assert_eq!(DEFAULT_PROD_URL, "wss://kim.ainexc.com/");
        assert!(DEFAULT_LOCAL_URL.starts_with("ws://"));
        assert!(DEFAULT_PROD_URL.starts_with("wss://"));
    }

    #[test]
    fn constructors() {
        let local = ClientConfig::local("tok");
        assert_eq!(local.url, DEFAULT_LOCAL_URL);
        assert_eq!(local.token, "tok");
        assert_eq!(local.user_agent, DEFAULT_CLIENT_USER_AGENT);
        let prod = ClientConfig::production("tok");
        assert_eq!(prod.url, DEFAULT_PROD_URL);
        assert_eq!(DEFAULT_DEVICE, "mobile");
        assert_eq!(local.device, DEFAULT_DEVICE);
        let ua = ClientConfig::local("tok").with_user_agent("KIM/1.0 (iOS)");
        assert_eq!(ua.user_agent, "KIM/1.0 (iOS)");
        let desktop = ClientConfig::local("tok").with_device(DESKTOP_DEVICE);
        assert_eq!(desktop.device, DESKTOP_DEVICE);
        let empty = ClientConfig::local("tok").with_device("  ");
        assert_eq!(empty.device, DEFAULT_DEVICE);
    }

    #[test]
    fn device_for_os_desktop_vs_mobile() {
        assert_eq!(device_for_target_os("macos"), DESKTOP_DEVICE);
        assert_eq!(device_for_target_os("windows"), DESKTOP_DEVICE);
        assert_eq!(device_for_target_os("linux"), DESKTOP_DEVICE);
        assert_eq!(device_for_target_os("ios"), DEFAULT_DEVICE);
        assert_eq!(device_for_target_os("android"), DEFAULT_DEVICE);
        assert_eq!(device_for_target_os(""), DEFAULT_DEVICE);
    }

    #[test]
    fn http_origin_follows_wgateway() {
        assert_eq!(
            http_origin_from_ws(DEFAULT_PROD_URL),
            DEFAULT_PROD_HTTP_ORIGIN
        );
        assert_eq!(
            http_origin_from_ws(DEFAULT_LOCAL_URL),
            DEFAULT_LOCAL_HTTP_ORIGIN
        );
        assert_eq!(
            http_origin_from_ws("ws://127.0.0.1:8001/chat"),
            DEFAULT_LOCAL_HTTP_ORIGIN
        );
        assert_eq!(
            http_origin_from_ws("wss://gw.example:8001/"),
            "https://gw.example:8080"
        );
        assert_eq!(http_origin_from_ws(""), DEFAULT_PROD_HTTP_ORIGIN);
    }
}
