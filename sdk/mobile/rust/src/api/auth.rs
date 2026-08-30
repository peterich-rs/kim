use kim_client::{http_origin_from_ws as map_origin, AuthClient};

use super::rt;

/// JWT issued by Royal. UI stores it in Keychain / Keystore.
pub struct AuthSession {
    pub token: String,
    pub exp: i64,
    pub account: String,
}

/// Royal `/api/v1/auth/*`. Protobuf HTTP; User-Agent is required.
pub struct KimAuth {
    inner: AuthClient,
}

impl KimAuth {
    #[flutter_rust_bridge::frb(sync)]
    pub fn new(base_url: String, user_agent: String) -> Result<Self, String> {
        Ok(Self {
            inner: AuthClient::new(base_url, user_agent).map_err(|e| e.to_string())?,
        })
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn register(&self, account: String, password: String) -> Result<AuthSession, String> {
        rt().block_on(self.inner.register(&account, &password))
            .map(Into::into)
            .map_err(|e| e.to_string())
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn login(&self, account: String, password: String) -> Result<AuthSession, String> {
        rt().block_on(self.inner.login(&account, &password))
            .map(Into::into)
            .map_err(|e| e.to_string())
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn logout(&self, token: String) -> Result<(), String> {
        rt().block_on(self.inner.logout(&token))
            .map_err(|e| e.to_string())
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn change_password(
        &self,
        token: String,
        old_password: String,
        new_password: String,
    ) -> Result<(), String> {
        rt().block_on(
            self.inner
                .change_password(&token, &old_password, &new_password),
        )
        .map_err(|e| e.to_string())
    }
}

impl From<kim_client::AuthSession> for AuthSession {
    fn from(s: kim_client::AuthSession) -> Self {
        Self {
            token: s.token,
            exp: s.exp,
            account: s.account,
        }
    }
}

#[flutter_rust_bridge::frb(sync)]
pub fn http_origin_from_ws(ws_url: String) -> String {
    map_origin(&ws_url)
}
