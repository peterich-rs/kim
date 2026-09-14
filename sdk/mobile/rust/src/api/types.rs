use kim_sdk::{
    CommandReceipt, LinkStateView, MessagePage, MessageView, SendStatus, SessionSnapshot,
    SessionUpdate, ThreadView, TimelineDelta, TimelineSnapshot, TimelineUpdate,
};

pub enum SendStatusDto {
    Pending,
    Uploading,
    Sending,
    Sent,
    Failed,
    Cancelled,
}

pub enum LinkStateDto {
    Connecting,
    Online,
    Reconnecting { attempt: u32 },
    Offline,
}

pub struct MessageViewDto {
    pub key: String,
    pub dest: String,
    pub sender: String,
    pub body: String,
    pub local_path: Option<String>,
    pub at: i64,
    pub sys: bool,
    pub kind: i32,
    pub width: i32,
    pub height: i32,
    pub message_id: i64,
    pub batch_id: Option<String>,
    pub send_status: SendStatusDto,
}

pub struct ThreadViewDto {
    pub id: String,
    pub kind: i32,
    pub title: String,
    pub avatar: String,
    pub last_body: String,
    pub last_at: i64,
    pub unread: i32,
}

pub struct TimelineSnapshotDto {
    pub dest: String,
    pub version: u64,
    pub messages: Vec<MessageViewDto>,
    pub pending: Vec<MessageViewDto>,
    pub unread: i32,
    pub last_read_message_id: i64,
    pub has_more: bool,
}

pub struct TimelineDeltaDto {
    pub dest: String,
    pub from_version: u64,
    pub to_version: u64,
    pub upserts: Vec<MessageViewDto>,
    pub deleted_keys: Vec<String>,
    pub unread: Option<i32>,
    pub last_read_message_id: Option<i64>,
}

pub enum TimelineUpdateDto {
    Snapshot { snapshot: TimelineSnapshotDto },
    Delta { delta: TimelineDeltaDto },
    Resync { dest: String, reason: String },
}

pub struct PersonDto {
    pub account: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
    pub relation: String,
    pub kind: i32,
}

pub struct RoomMemberDto {
    pub account: String,
    pub status: i32,
    pub last_seen: i64,
}

#[flutter_rust_bridge::frb(unignore)]
pub struct BotDto {
    pub dest: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
    pub model: String,
    pub thinking_effort: String,
    pub context_tokens: i32,
    pub visibility: String,
}

pub struct ProfileDto {
    pub account: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
    pub kind: i32,
}

pub struct MessagePageDto {
    pub dest: String,
    pub messages: Vec<MessageViewDto>,
    pub has_more: bool,
}

#[flutter_rust_bridge::frb(unignore)]
pub struct CommandAckDto {
    pub request_id: String,
    pub client_id: String,
    pub dest: String,
    pub accepted_at: i64,
    pub send_status: SendStatusDto,
}

pub struct SessionSnapshotDto {
    pub link: LinkStateDto,
    pub last_error: Option<String>,
    pub threads: Vec<ThreadViewDto>,
    pub unread_total: i32,
}

pub enum SessionUpdateDto {
    Link {
        state: LinkStateDto,
        last_error: Option<String>,
    },
    Inbox {
        threads: Vec<ThreadViewDto>,
    },
    ThreadUpsert {
        thread: ThreadViewDto,
    },
    SyncProgress {
        pulled: u64,
        catching_up: bool,
    },
    Kickout {
        channel_id: String,
    },
    AuthExpired {
        reason: String,
    },
    TokenRenew {
        token: String,
        exp: i64,
    },
    FriendRequest {
        from: String,
        nickname: String,
    },
    FriendAccepted {
        from: String,
        nickname: String,
    },
    ProfileUpdated {
        account: String,
        nickname: String,
        avatar: String,
    },
    Presence {
        account: String,
        status: i32,
        last_seen: i64,
    },
    Typing {
        typer: String,
        dest: String,
        kind: i32,
        active: bool,
    },
    ReceiptRead {
        reader: String,
        dest: String,
        kind: i32,
        message_id: i64,
    },
    GroupCreate {
        group_id: String,
        members: Vec<String>,
    },
    ContactsChanged {
        contacts: Vec<PersonDto>,
    },
    RustPanic {
        message: String,
    },
}

#[flutter_rust_bridge::frb(unignore)]
pub struct SettingsDto {
    pub ws_url: String,
    pub http_origin: String,
    pub env: String,
    pub locale: String,
    pub account: String,
}

#[flutter_rust_bridge::frb(unignore)]
pub enum TokenPersistDto {
    Write { token: String },
    Clear,
}

#[flutter_rust_bridge::frb(unignore)]
pub struct LocalMediaDto {
    pub local_path: String,
    pub byte_size: i64,
    pub width: i32,
    pub height: i32,
}

pub struct SdkErrorDto {
    pub kind: String,
    pub message: String,
}

impl From<SendStatus> for SendStatusDto {
    fn from(v: SendStatus) -> Self {
        match v {
            SendStatus::Pending => Self::Pending,
            SendStatus::Uploading => Self::Uploading,
            SendStatus::Sending => Self::Sending,
            SendStatus::Sent => Self::Sent,
            SendStatus::Failed => Self::Failed,
            SendStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<LinkStateView> for LinkStateDto {
    fn from(v: LinkStateView) -> Self {
        match v {
            LinkStateView::Connecting => Self::Connecting,
            LinkStateView::Online => Self::Online,
            LinkStateView::Reconnecting { attempt } => Self::Reconnecting { attempt },
            LinkStateView::Offline => Self::Offline,
        }
    }
}

impl From<MessageView> for MessageViewDto {
    fn from(v: MessageView) -> Self {
        Self {
            key: v.key,
            dest: v.dest,
            sender: v.sender,
            body: v.body,
            local_path: v.local_path,
            at: v.at,
            sys: v.sys,
            kind: v.kind,
            width: v.width,
            height: v.height,
            message_id: v.message_id,
            batch_id: v.batch_id,
            send_status: v.send_status.into(),
        }
    }
}

impl From<ThreadView> for ThreadViewDto {
    fn from(v: ThreadView) -> Self {
        Self {
            id: v.id,
            kind: v.kind,
            title: v.title,
            avatar: v.avatar,
            last_body: v.last_body,
            last_at: v.last_at,
            unread: v.unread,
        }
    }
}

impl From<TimelineSnapshot> for TimelineSnapshotDto {
    fn from(v: TimelineSnapshot) -> Self {
        Self {
            dest: v.dest,
            version: v.version,
            messages: v.messages.into_iter().map(Into::into).collect(),
            pending: v.pending.into_iter().map(Into::into).collect(),
            unread: v.unread,
            last_read_message_id: v.last_read_message_id,
            has_more: v.has_more,
        }
    }
}

impl From<TimelineDelta> for TimelineDeltaDto {
    fn from(v: TimelineDelta) -> Self {
        Self {
            dest: v.dest,
            from_version: v.from_version,
            to_version: v.to_version,
            upserts: v.upserts.into_iter().map(Into::into).collect(),
            deleted_keys: v.deleted_keys,
            unread: v.unread,
            last_read_message_id: v.last_read_message_id,
        }
    }
}

impl From<TimelineUpdate> for TimelineUpdateDto {
    fn from(v: TimelineUpdate) -> Self {
        match v {
            TimelineUpdate::Snapshot { snapshot } => Self::Snapshot {
                snapshot: snapshot.into(),
            },
            TimelineUpdate::Delta { delta } => Self::Delta {
                delta: delta.into(),
            },
            TimelineUpdate::Resync { dest, reason } => Self::Resync { dest, reason },
        }
    }
}

impl From<MessagePage> for MessagePageDto {
    fn from(v: MessagePage) -> Self {
        Self {
            dest: v.dest,
            messages: v.messages.into_iter().map(Into::into).collect(),
            has_more: v.has_more,
        }
    }
}

impl From<CommandReceipt> for CommandAckDto {
    fn from(v: CommandReceipt) -> Self {
        Self {
            request_id: v.request_id,
            client_id: v.client_id,
            dest: v.dest,
            accepted_at: v.accepted_at,
            send_status: v.send_status.into(),
        }
    }
}

impl From<SessionSnapshot> for SessionSnapshotDto {
    fn from(v: SessionSnapshot) -> Self {
        Self {
            link: v.link.into(),
            last_error: v.last_error,
            threads: v.threads.into_iter().map(Into::into).collect(),
            unread_total: v.unread_total,
        }
    }
}

impl From<SessionUpdate> for SessionUpdateDto {
    fn from(v: SessionUpdate) -> Self {
        match v {
            SessionUpdate::Link { state, last_error } => Self::Link {
                state: state.into(),
                last_error,
            },
            SessionUpdate::Inbox { threads } => Self::Inbox {
                threads: threads.into_iter().map(Into::into).collect(),
            },
            SessionUpdate::ThreadUpsert { thread } => Self::ThreadUpsert {
                thread: thread.into(),
            },
            SessionUpdate::SyncProgress {
                pulled,
                catching_up,
            } => Self::SyncProgress {
                pulled,
                catching_up,
            },
            SessionUpdate::Kickout { channel_id } => Self::Kickout { channel_id },
            SessionUpdate::AuthExpired { reason } => Self::AuthExpired { reason },
            SessionUpdate::TokenRenew { token, exp } => Self::TokenRenew { token, exp },
            SessionUpdate::FriendRequest { from, nickname } => {
                Self::FriendRequest { from, nickname }
            }
            SessionUpdate::FriendAccepted { from, nickname } => {
                Self::FriendAccepted { from, nickname }
            }
            SessionUpdate::ProfileUpdated {
                account,
                nickname,
                avatar,
            } => Self::ProfileUpdated {
                account,
                nickname,
                avatar,
            },
            SessionUpdate::Presence {
                account,
                status,
                last_seen,
            } => Self::Presence {
                account,
                status,
                last_seen,
            },
            SessionUpdate::Typing {
                typer,
                dest,
                kind,
                active,
            } => Self::Typing {
                typer,
                dest,
                kind,
                active,
            },
            SessionUpdate::ReceiptRead {
                reader,
                dest,
                kind,
                message_id,
            } => Self::ReceiptRead {
                reader,
                dest,
                kind,
                message_id,
            },
            SessionUpdate::GroupCreate { group_id, members } => {
                Self::GroupCreate { group_id, members }
            }
            SessionUpdate::ContactsChanged { contacts } => Self::ContactsChanged {
                contacts: contacts
                    .into_iter()
                    .map(|p| PersonDto {
                        account: p.account,
                        nickname: p.nickname,
                        avatar: p.avatar,
                        bio: p.bio,
                        relation: p.relation,
                        kind: 1,
                    })
                    .collect(),
            },
            SessionUpdate::RustPanic { message } => Self::RustPanic { message },
        }
    }
}

impl PersonDto {
    pub(crate) fn from_profile(p: kim_client::Profile, relation: &str) -> Self {
        Self {
            account: p.account,
            nickname: p.nickname,
            avatar: p.avatar,
            bio: p.bio,
            relation: relation.into(),
            kind: p.kind,
        }
    }
}

impl From<kim_client::Profile> for ProfileDto {
    fn from(p: kim_client::Profile) -> Self {
        Self {
            account: p.account,
            nickname: p.nickname,
            avatar: p.avatar,
            bio: p.bio,
            kind: p.kind,
        }
    }
}

impl From<kim_client::Profile> for PersonDto {
    fn from(p: kim_client::Profile) -> Self {
        Self::from_profile(p, "none")
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
