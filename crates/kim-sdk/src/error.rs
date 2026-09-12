use thiserror::Error;

#[derive(Debug, Error)]
pub enum SdkError {
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
    #[error("internal error")]
    Internal { message: String },
}

impl SdkError {
    /// Transport / congestion. Not disk-full or protocol business codes.
    #[must_use]
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::NotConnected | Self::Busy { .. } | Self::SqliteBusy | Self::RateLimited { .. }
        )
    }

    /// Send retry. Aligns with web `isRetryable`: status 3 or 3xx; never 99 / 1xx / 111.
    #[must_use]
    pub fn retryable_send(&self) -> bool {
        if self.retryable() {
            return true;
        }
        match self {
            Self::Protocol { status: 3 } => true,
            Self::Protocol { status } if (300..400).contains(status) => true,
            _ => false,
        }
    }
}

pub(crate) fn map_client(err: kim_client::ClientError, dest: &str) -> SdkError {
    use kim_client::ClientError;
    match err {
        ClientError::NotConnected
        | ClientError::NotLoggedIn
        | ClientError::HandshakeTimeout(_)
        | ClientError::Handshake(_) => SdkError::NotConnected,
        ClientError::Unauthorized | ClientError::InvalidToken => SdkError::Unauthorized,
        ClientError::Status(109) => SdkError::NotFriends {
            dest: dest.to_string(),
        },
        ClientError::Status(110) => SdkError::Blocked {
            dest: dest.to_string(),
        },
        ClientError::Status(108) => SdkError::UserNotFound {
            dest: dest.to_string(),
        },
        ClientError::Status(105) => SdkError::Unauthorized,
        ClientError::Status(status) => SdkError::Protocol { status },
        ClientError::Http { status: 401, .. } => SdkError::Unauthorized,
        ClientError::Http { status: 429, .. } => SdkError::RateLimited {
            retry_after_ms: 1000,
        },
        other => SdkError::Internal {
            message: other.to_string(),
        },
    }
}

pub(crate) fn map_sqlx(err: sqlx::Error) -> SdkError {
    match err {
        sqlx::Error::Database(db) => match db.code().as_deref() {
            Some("5" | "6") => SdkError::SqliteBusy,
            Some("13") => SdkError::StorageFull,
            _ => SdkError::Disk {
                message: "sqlite error".into(),
            },
        },
        sqlx::Error::PoolTimedOut => SdkError::SqliteBusy,
        _ => SdkError::Disk {
            message: "sqlite error".into(),
        },
    }
}
