use std::time::Duration;

use kim_core::DEFAULT_LOGIN_WAIT;

/// Local WGateway (plaintext WS). Used by `pkt-client` and the web app:local.
pub const DEFAULT_LOCAL_URL: &str = "ws://127.0.0.1:8001/";
/// Production WGateway (WSS). Same host as the product web app.
pub const DEFAULT_PROD_URL: &str = "wss://kim.ainexc.com/";
/// `LoginReq.device`. Exclusive with other mobile sessions.
pub const DEFAULT_DEVICE: &str = "mobile";

/// How to reach WGateway. Token is never placed on the Upgrade URL.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub url: String,
    pub token: String,
    pub handshake_timeout: Duration,
}

impl ClientConfig {
    pub fn new(url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            token: token.into(),
            handshake_timeout: DEFAULT_LOGIN_WAIT,
        }
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
        let prod = ClientConfig::production("tok");
        assert_eq!(prod.url, DEFAULT_PROD_URL);
        assert_eq!(DEFAULT_DEVICE, "mobile");
    }
}
