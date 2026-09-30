use super::failure::ApiFailure;
use kim_client::AuthClient;

/// JWT issued by Royal. Rust persists it via the secret-store channel.
pub struct AuthSession {
    pub token: String,
    pub exp: i64,
    pub account: String,
}

/// Royal `/api/v1/auth/*`. The origin is supplied by [`KimUiHandle::auth`]
/// from the settings table; UA comes from the bootstrap; secure-origin
/// enforcement happens at construction.
pub struct KimAuth {
    inner: AuthClient,
}

impl KimAuth {
    pub fn with_origin(origin: String) -> Result<Self, ApiFailure> {
        Ok(Self {
            inner: AuthClient::new(origin, kim_sdk::user_agent()).map_err(ApiFailure::from)?,
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
