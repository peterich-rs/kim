/// Result of a `chat.user.talk` / `chat.group.talk` Response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TalkResult {
    pub message_id: i64,
    pub send_time: i64,
    pub sequence: u32,
}

/// Product profile on friend list / search / incoming.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    pub account: String,
    pub nickname: String,
    #[serde(default)]
    pub avatar: String,
}

impl Profile {
    pub fn from_wire(account: String, nickname: String, avatar: String) -> Self {
        let nickname = if nickname.is_empty() {
            account.clone()
        } else {
            nickname
        };
        Self {
            account,
            nickname,
            avatar,
        }
    }

    pub fn encode_list(users: &[Self]) -> Result<String, String> {
        serde_json::to_string(users).map_err(|e| e.to_string())
    }

    pub fn encode_one(user: &Self) -> Result<String, String> {
        serde_json::to_string(user).map_err(|e| e.to_string())
    }
}

/// Inbox row from `chat.inbox.list` (`InboxItem` proto fields).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboxItem {
    pub dest: String,
    pub kind: i32,
    pub title: String,
    pub avatar: String,
    pub last_body: String,
    pub last_sender: String,
    pub last_message_id: i64,
    pub last_send_time: i64,
    pub unread: i32,
}

/// History row from `chat.history`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryItem {
    pub message_id: i64,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
    pub sender: String,
    pub send_time: i64,
    pub direction: i32,
}

/// Offline index row from `chat.offline.index`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageIndex {
    pub message_id: i64,
    pub direction: i32,
    pub send_time: i64,
    pub account_b: String,
    pub group: String,
}

/// Message body from `chat.offline.content`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub message_id: i64,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
}

/// One wire `MessageReq`. Mixed input is split by the caller into several of these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutgoingContent {
    Text(String),
    Image { url: String, extra: String },
    Voice { url: String, extra: String },
    Video { url: String, extra: String },
}

/// Unsolicited (or unmatched) inbound traffic after login.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Pong,
    Talk(IncomingTalk),
    TalkResp(TalkResult),
    Kickout {
        channel_id: String,
    },
    TokenRenew {
        token: String,
        exp: i64,
    },
    GroupCreate {
        group_id: String,
        members: Vec<String>,
    },
    FriendRequest {
        from: String,
        nickname: String,
    },
    FriendAccepted {
        from: String,
        nickname: String,
    },
    UserList {
        command: String,
        sequence: u32,
        users: Vec<Profile>,
    },
    Profile {
        sequence: u32,
        profile: Profile,
    },
    Inbox {
        sequence: u32,
        items: Vec<InboxItem>,
    },
    History {
        sequence: u32,
        dest: String,
        messages: Vec<HistoryItem>,
    },
    OfflinePage {
        sequence: u32,
        indexes: Vec<MessageIndex>,
        has_more: bool,
    },
    OfflineContent {
        sequence: u32,
        messages: Vec<Message>,
    },
    Status {
        command: String,
        status: i32,
        sequence: u32,
    },
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IncomingTalk {
    pub command: String,
    /// Thread id: peer account for 1:1, group id for `chat.group.talk`.
    pub dest: String,
    pub message_id: i64,
    pub sender: String,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
    pub send_time: i64,
}
