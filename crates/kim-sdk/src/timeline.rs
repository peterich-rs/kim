use crate::command::SendStatus;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageView {
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
    pub send_status: SendStatus,
}

#[derive(Clone, Debug)]
pub struct TimelineSnapshot {
    pub dest: String,
    pub version: u64,
    pub messages: Vec<MessageView>,
    pub pending: Vec<MessageView>,
    pub unread: i32,
    pub last_read_message_id: i64,
    pub has_more: bool,
    pub loading_older: bool,
    pub history_error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TimelineDelta {
    pub dest: String,
    pub from_version: u64,
    pub to_version: u64,
    pub upserts: Vec<MessageView>,
    pub deleted_keys: Vec<String>,
    pub unread: Option<i32>,
    pub last_read_message_id: Option<i64>,
}

#[derive(Clone, Debug)]
pub enum TimelineUpdate {
    Snapshot { snapshot: TimelineSnapshot },
    Delta { delta: TimelineDelta },
    Resync { dest: String, reason: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkStateView {
    Connecting,
    Online,
    Reconnecting { attempt: u32 },
    Offline,
}

#[derive(Clone, Debug)]
pub enum SessionUpdate {
    Link {
        state: LinkStateView,
        last_error: Option<String>,
    },
    Inbox {
        threads: Vec<ThreadView>,
    },
    ThreadUpsert {
        thread: ThreadView,
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
        phase: i32,
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
        contacts: Vec<PersonRef>,
    },
    AgentTurn {
        dest: String,
        state: AgentTurnState,
        text: String,
    },
    AgentCard {
        dest: String,
        card: AgentCard,
    },
    RustPanic {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentTurnState {
    Queued,
    Running,
    WaitingPermission,
    Done,
    Error,
    /// No user-visible reply and no successful side effect.
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentCard {
    pub v: i32,
    pub card_type: String,
    pub call_id: String,
    pub name: String,
    pub state: String,
    pub preview: String,
    pub ok: bool,
}

/// Lightweight contact row for `SessionUpdate::ContactsChanged` (P3 fills this).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonRef {
    pub account: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
    pub relation: String,
    pub kind: i32,
}

#[derive(Clone, Debug)]
pub struct ContactsSnapshot {
    pub version: u64,
    pub contacts: Vec<PersonRef>,
    pub sync_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadView {
    pub id: String,
    pub kind: i32,
    pub title: String,
    pub avatar: String,
    pub last_body: String,
    pub last_at: i64,
    pub unread: i32,
}

/// Sticky session fact. Watch the snapshot; do not rely on a one-shot event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionFault {
    IdentityExpired { reason: String },
    Kicked { channel_id: String },
}

impl SessionFault {
    #[must_use]
    pub fn from_sdk_error(err: &crate::SdkError) -> Option<Self> {
        err.ends_identity().then(|| Self::IdentityExpired {
            reason: err.to_string(),
        })
    }

    #[must_use]
    pub fn wire_kind(&self) -> &'static str {
        match self {
            Self::IdentityExpired { .. } => "auth_expired",
            Self::Kicked { .. } => "kickout",
        }
    }

    #[must_use]
    pub fn to_update(&self) -> SessionUpdate {
        match self {
            Self::IdentityExpired { reason } => SessionUpdate::AuthExpired {
                reason: reason.clone(),
            },
            Self::Kicked { channel_id } => SessionUpdate::Kickout {
                channel_id: channel_id.clone(),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct SessionSnapshot {
    pub link: LinkStateView,
    pub last_error: Option<String>,
    pub threads: Vec<ThreadView>,
    pub unread_total: i32,
}

impl Default for SessionSnapshot {
    fn default() -> Self {
        Self {
            link: LinkStateView::Offline,
            last_error: None,
            threads: Vec::new(),
            unread_total: 0,
        }
    }
}

#[must_use]
pub fn kind_from_name(name: &str) -> i32 {
    match name {
        "image" => kim_protocol::MESSAGE_TYPE_IMAGE,
        "video" => kim_protocol::MESSAGE_TYPE_VIDEO,
        _ => kim_protocol::MESSAGE_TYPE_TEXT,
    }
}

#[must_use]
pub fn kind_name(kind: i32) -> &'static str {
    match kind {
        kim_protocol::MESSAGE_TYPE_IMAGE => "image",
        kim_protocol::MESSAGE_TYPE_VIDEO => "video",
        _ => "text",
    }
}

#[must_use]
pub fn thread_kind_name(kind: i32) -> &'static str {
    if kind == kim_protocol::INBOX_KIND_GROUP {
        "group"
    } else {
        "user"
    }
}

#[must_use]
pub fn thread_kind_from_name(name: &str) -> i32 {
    if name == "group" {
        kim_protocol::INBOX_KIND_GROUP
    } else {
        kim_protocol::INBOX_KIND_USER
    }
}
