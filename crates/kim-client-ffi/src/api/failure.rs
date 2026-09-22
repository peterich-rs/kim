//! Wire error for every public FFI `Result`. FRB turns this enum into a Dart
//! sealed class that implements `FrbException`. The type is not named `Error`:
//! flutter_rust_bridge collapses any type whose path ends in `Error` into
//! `AnyhowException`.

use kim_client::ClientError;
use kim_sdk::{status_to_sdk_error, SdkError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiFailure {
    #[error("not friends with {dest}")]
    NotFriends { dest: String },
    #[error("blocked by {dest}")]
    Blocked { dest: String },
    #[error("user not found: {dest}")]
    UserNotFound { dest: String },
    #[error("cannot chat with self")]
    CannotChatSelf,
    #[error("auth expired")]
    AuthExpired,
    #[error("unauthorized")]
    Unauthorized,
    #[error("invalid account")]
    InvalidAccount,
    #[error("invalid password")]
    InvalidPassword,
    #[error("account exists")]
    AccountExists,
    #[error("insecure origin")]
    InsecureOrigin,
    #[error("password seal unavailable")]
    PasswordSeal,
    #[error("already connected")]
    AlreadyConnected,
    #[error("not connected")]
    NotConnected,
    #[error("storage full")]
    StorageFull,
    #[error("sqlite busy")]
    SqliteBusy,
    #[error("disk error")]
    Disk { message: String },
    #[error("rate limited, retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: i64 },
    #[error("payload too large: {bytes} exceeds {max} bytes")]
    PayloadTooLarge { bytes: i64, max: i64 },
    #[error("unsupported media: {mime}")]
    UnsupportedMedia { mime: String },
    #[error("busy: {queue}")]
    Busy { queue: String },
    #[error("stale epoch: expected {expected}, actual {actual}")]
    StaleEpoch { expected: u64, actual: u64 },
    #[error("not found: {what}")]
    NotFound { what: String },
    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },
    #[error("protocol status {status}")]
    Protocol { status: i32 },
    #[error("http {status}")]
    Http { status: u16 },
    #[error("unavailable: {what}")]
    Unavailable { what: String },
    #[error("internal error")]
    Internal { message: String },
}

impl ApiFailure {
    #[must_use]
    pub(crate) fn unavailable(what: impl Into<String>) -> Self {
        Self::Unavailable { what: what.into() }
    }

    /// `dest` fills friend/block/not-found variants. Auth calls pass `""`.
    #[must_use]
    pub(crate) fn from_client(err: ClientError, dest: &str) -> Self {
        match err {
            ClientError::NotConnected
            | ClientError::NotLoggedIn
            | ClientError::HandshakeTimeout(_)
            | ClientError::Handshake(_) => Self::NotConnected,
            ClientError::AlreadyConnected => Self::AlreadyConnected,
            ClientError::Unauthorized | ClientError::InvalidToken => Self::Unauthorized,
            ClientError::InvalidAccount => Self::InvalidAccount,
            ClientError::InvalidPassword => Self::InvalidPassword,
            ClientError::InsecureOrigin => Self::InsecureOrigin,
            ClientError::PasswordSeal => Self::PasswordSeal,
            ClientError::Status(status) => Self::from(status_to_sdk_error(status, dest)),
            ClientError::Http { status: 401, .. } => Self::Unauthorized,
            ClientError::Http { status: 409, .. } => Self::AccountExists,
            ClientError::Http { status: 413, .. } => Self::PayloadTooLarge { bytes: 0, max: 0 },
            ClientError::Http { status: 415, .. } => Self::UnsupportedMedia {
                mime: String::new(),
            },
            ClientError::Http { status: 429, .. } => Self::RateLimited {
                retry_after_ms: 1000,
            },
            ClientError::Http { status, .. } => Self::Http { status },
            ClientError::Protocol(err) => Self::Internal {
                message: err.to_string(),
            },
            ClientError::Core(err) => Self::Internal {
                message: err.to_string(),
            },
            ClientError::Other(message) => Self::Internal { message },
        }
    }
}

impl From<ClientError> for ApiFailure {
    fn from(err: ClientError) -> Self {
        Self::from_client(err, "")
    }
}

impl From<SdkError> for ApiFailure {
    fn from(err: SdkError) -> Self {
        match err {
            SdkError::NotFriends { dest } => Self::NotFriends { dest },
            SdkError::Blocked { dest } => Self::Blocked { dest },
            SdkError::UserNotFound { dest } => Self::UserNotFound { dest },
            SdkError::CannotChatSelf => Self::CannotChatSelf,
            SdkError::AuthExpired => Self::AuthExpired,
            SdkError::Unauthorized => Self::Unauthorized,
            SdkError::NotConnected => Self::NotConnected,
            SdkError::StorageFull => Self::StorageFull,
            SdkError::SqliteBusy => Self::SqliteBusy,
            SdkError::Disk { message } => Self::Disk { message },
            SdkError::RateLimited { retry_after_ms } => Self::RateLimited { retry_after_ms },
            SdkError::PayloadTooLarge { bytes, max } => Self::PayloadTooLarge { bytes, max },
            SdkError::UnsupportedMedia { mime } => Self::UnsupportedMedia { mime },
            SdkError::Busy { queue } => Self::Busy { queue },
            SdkError::StaleEpoch { expected, actual } => Self::StaleEpoch { expected, actual },
            SdkError::NotFound { what } => Self::NotFound { what },
            SdkError::InvalidArgument { message } => Self::InvalidArgument { message },
            SdkError::Protocol { status } => Self::Protocol { status },
            SdkError::Internal { message } => Self::Internal { message },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_error_keeps_structured_fields() {
        let err = ApiFailure::from(SdkError::RateLimited {
            retry_after_ms: 2500,
        });
        assert!(matches!(
            err,
            ApiFailure::RateLimited {
                retry_after_ms: 2500
            }
        ));
        let err = ApiFailure::from(SdkError::Protocol { status: 3 });
        assert!(matches!(err, ApiFailure::Protocol { status: 3 }));
        let err = ApiFailure::from(SdkError::NotFriends { dest: "bob".into() });
        assert!(matches!(err, ApiFailure::NotFriends { dest } if dest == "bob"));
    }

    #[test]
    fn client_auth_and_status_map_without_display_text() {
        assert!(matches!(
            ApiFailure::from(ClientError::InvalidPassword),
            ApiFailure::InvalidPassword
        ));
        assert!(matches!(
            ApiFailure::from(ClientError::Http {
                status: 401,
                body: "账号或密码错误".into(),
            }),
            ApiFailure::Unauthorized
        ));
        assert!(matches!(
            ApiFailure::from(ClientError::Http {
                status: 409,
                body: "账号已存在".into(),
            }),
            ApiFailure::AccountExists
        ));
        let err = ApiFailure::from_client(ClientError::Status(109), "bob");
        assert!(matches!(err, ApiFailure::NotFriends { dest } if dest == "bob"));
        assert!(matches!(
            ApiFailure::from(ClientError::Status(109)),
            ApiFailure::NotFriends { .. }
        ));
    }
}
