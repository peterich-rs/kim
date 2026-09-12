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

#[derive(Clone, Debug)]
pub struct SessionSnapshot {
    pub link: LinkStateView,
    pub last_error: Option<String>,
    pub threads: Vec<ThreadView>,
}

impl Default for SessionSnapshot {
    fn default() -> Self {
        Self {
            link: LinkStateView::Offline,
            last_error: None,
            threads: Vec::new(),
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
