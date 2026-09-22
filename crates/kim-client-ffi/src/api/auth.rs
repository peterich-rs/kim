use super::failure::ApiFailure;
use kim_client::{http_origin_from_ws as map_origin, AuthClient};

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
    pub fn new(base_url: String, user_agent: String) -> Result<Self, ApiFailure> {
        Ok(Self {
            inner: AuthClient::new(base_url, user_agent).map_err(ApiFailure::from)?,
        })
    }

    pub async fn register(
        &self,
        account: String,
        password: String,
    ) -> Result<AuthSession, ApiFailure> {
        self.inner
            .register(&account, &password)
            .await
            .map(Into::into)
            .map_err(ApiFailure::from)
    }

    pub async fn login(
        &self,
        account: String,
        password: String,
    ) -> Result<AuthSession, ApiFailure> {
        self.inner
            .login(&account, &password)
            .await
            .map(Into::into)
            .map_err(ApiFailure::from)
    }

    pub async fn logout(&self, token: String) -> Result<(), ApiFailure> {
        self.inner.logout(&token).await.map_err(ApiFailure::from)
    }

    pub async fn change_password(
        &self,
        token: String,
        old_password: String,
        new_password: String,
    ) -> Result<(), ApiFailure> {
        self.inner
            .change_password(&token, &old_password, &new_password)
            .await
            .map_err(ApiFailure::from)
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
