use kim_sdk::{SessionUpdate, TimelineUpdate};

pub struct TimelineUpdateDto {
    pub kind: String,
    pub dest: String,
    pub version: u64,
    pub from_version: u64,
    pub to_version: u64,
    pub unread: i32,
    pub has_more: bool,
    pub reason: String,
}

pub struct SessionUpdateDto {
    pub kind: String,
    pub channel_id: String,
    pub reason: String,
    pub token: String,
    pub exp: i64,
    pub from: String,
    pub nickname: String,
    pub pulled: u64,
    pub catching_up: bool,
    pub last_error: Option<String>,
    pub inbox_count: i32,
}

pub struct SdkErrorDto {
    pub kind: String,
    pub message: String,
}

impl From<TimelineUpdate> for TimelineUpdateDto {
    fn from(v: TimelineUpdate) -> Self {
        match v {
            TimelineUpdate::Snapshot { snapshot } => Self {
                kind: "snapshot".into(),
                dest: snapshot.dest,
                version: snapshot.version,
                from_version: 0,
                to_version: snapshot.version,
                unread: snapshot.unread,
                has_more: snapshot.has_more,
                reason: String::new(),
            },
            TimelineUpdate::Delta { delta } => Self {
                kind: "delta".into(),
                dest: delta.dest,
                version: delta.to_version,
                from_version: delta.from_version,
                to_version: delta.to_version,
                unread: delta.unread.unwrap_or(-1),
                has_more: false,
                reason: String::new(),
            },
            TimelineUpdate::Resync { dest, reason } => Self {
                kind: "resync".into(),
                dest,
                version: 0,
                from_version: 0,
                to_version: 0,
                unread: 0,
                has_more: false,
                reason,
            },
        }
    }
}

impl From<SessionUpdate> for SessionUpdateDto {
    fn from(v: SessionUpdate) -> Self {
        let mut dto = Self {
            kind: String::new(),
            channel_id: String::new(),
            reason: String::new(),
            token: String::new(),
            exp: 0,
            from: String::new(),
            nickname: String::new(),
            pulled: 0,
            catching_up: false,
            last_error: None,
            inbox_count: 0,
        };
        match v {
            SessionUpdate::Kickout { channel_id } => {
                dto.kind = "kickout".into();
                dto.channel_id = channel_id;
            }
            SessionUpdate::AuthExpired { reason } => {
                dto.kind = "auth_expired".into();
                dto.reason = reason;
            }
            SessionUpdate::TokenRenew { token, exp } => {
                dto.kind = "token".into();
                dto.token = token;
                dto.exp = exp;
            }
            SessionUpdate::FriendRequest { from, nickname } => {
                dto.kind = "friend".into();
                dto.from = from;
                dto.nickname = nickname;
            }
            SessionUpdate::FriendAccepted { from, nickname } => {
                dto.kind = "friend_accepted".into();
                dto.from = from;
                dto.nickname = nickname;
            }
            SessionUpdate::Inbox { threads } => {
                dto.kind = "inbox".into();
                dto.inbox_count = i32::try_from(threads.len()).unwrap_or(i32::MAX);
            }
            SessionUpdate::SyncProgress {
                pulled,
                catching_up,
            } => {
                dto.kind = "sync_progress".into();
                dto.pulled = pulled;
                dto.catching_up = catching_up;
            }
            SessionUpdate::Link { last_error, .. } => {
                dto.kind = "link".into();
                dto.last_error = last_error;
            }
            _ => dto.kind = "other".into(),
        }
        dto
    }
}

impl From<kim_sdk::SdkError> for SdkErrorDto {
    fn from(err: kim_sdk::SdkError) -> Self {
        use kim_sdk::SdkError::*;
        let kind = match &err {
            NotFriends { .. } => "not_friends",
            Blocked { .. } => "blocked",
            UserNotFound { .. } => "user_not_found",
            CannotChatSelf => "cannot_chat_self",
            AuthExpired => "auth_expired",
            Unauthorized => "unauthorized",
            NotConnected => "not_connected",
            StorageFull => "storage_full",
            SqliteBusy => "sqlite_busy",
            Disk { .. } => "disk",
            RateLimited { .. } => "rate_limited",
            PayloadTooLarge { .. } => "payload_too_large",
            UnsupportedMedia { .. } => "unsupported_media",
            Busy { .. } => "busy",
            StaleEpoch { .. } => "stale_epoch",
            NotFound { .. } => "not_found",
            InvalidArgument { .. } => "invalid_argument",
            Protocol { .. } => "protocol",
            Internal { .. } => "internal",
        };
        Self {
            kind: kind.into(),
            message: err.to_string(),
        }
    }
}
