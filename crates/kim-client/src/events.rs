/// Result of a `chat.user.talk` / `chat.group.talk` / `chat.bot.reply` Response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TalkResult {
    pub message_id: i64,
    pub send_time: i64,
    pub sequence: u32,
}

/// Owner-only bot runtime projection. Never includes secrets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BotConfig {
    pub model: String,
    pub thinking_effort: String,
    pub context_tokens: Option<i32>,
    pub visibility: String,
}

/// Product profile on friend list / search / incoming.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    pub account: String,
    pub nickname: String,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub bio: String,
    /// `PROFILE_KIND_USER` (1) or `PROFILE_KIND_BOT` (2).
    #[serde(default)]
    pub kind: i32,
}

impl Profile {
    pub fn from_wire(
        account: String,
        nickname: String,
        avatar: String,
        bio: String,
        kind: i32,
    ) -> Self {
        let nickname = if nickname.is_empty() {
            account.clone()
        } else {
            nickname
        };
        Self {
            account,
            nickname,
            avatar,
            bio,
            kind: kim_protocol::profile_kind(kind),
        }
    }

    pub fn is_bot(&self) -> bool {
        self.kind == kim_protocol::PROFILE_KIND_BOT
    }

    pub fn encode_list(users: &[Self]) -> Result<String, String> {
        serde_json::to_string(users).map_err(|e| e.to_string())
    }

    pub fn encode_one(user: &Self) -> Result<String, String> {
        serde_json::to_string(user).map_err(|e| e.to_string())
    }
}

/// Inbox row from `chat.inbox.list` (`InboxItem` proto fields).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
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
    pub last_read_message_id: i64,
    pub max_message_id: i64,
    pub state_version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ConversationReadState {
    pub dest: String,
    pub kind: i32,
    pub last_read_message_id: i64,
    pub max_message_id: i64,
    pub unread: i32,
    pub version: u64,
    pub exists: bool,
}

impl ConversationReadState {
    pub fn from_proto(p: kim_protocol::pkt::ConversationReadState) -> Self {
        Self {
            dest: p.dest,
            kind: p.kind,
            last_read_message_id: p.last_read_message_id,
            max_message_id: p.max_message_id,
            unread: p.unread,
            version: p.version,
            exists: p.exists,
        }
    }

    pub fn from_inbox_item(item: &InboxItem) -> Self {
        Self {
            dest: item.dest.clone(),
            kind: item.kind,
            last_read_message_id: item.last_read_message_id,
            max_message_id: item.max_message_id.max(item.last_message_id),
            unread: item.unread,
            version: item.state_version,
            exists: true,
        }
    }
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
    BotPending {
        sequence: u32,
        items: Vec<BotPendingItem>,
    },
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
    /// Push `chat.user.updated` — friend (or self other device) profile snapshot.
    ProfileUpdated {
        profile: Profile,
    },
    /// Push `chat.presence` — one entry from a PresencePush batch.
    PresenceUpdated {
        account: String,
        status: i32,
        last_seen: i64,
    },
    /// Push `chat.typing` — conversation-scoped typing indicator.
    TypingUpdated {
        typer: String,
        dest: String,
        kind: i32,
        active: bool,
        phase: i32,
    },
    /// Push `chat.receipt.read` — DM peer read watermark.
    ReceiptRead {
        reader: String,
        dest: String,
        kind: i32,
        message_id: i64,
    },
    /// Response `chat.inbox.read` with authoritative state (empty on old servers).
    ConversationRead {
        sequence: u32,
        state: ConversationReadState,
    },
    /// Response `chat.inbox.states`.
    ConversationStates {
        sequence: u32,
        states: Vec<ConversationReadState>,
    },
    /// Push `chat.inbox.read.sync` — self-account read watermark.
    ConversationReadSync {
        account: String,
        state: ConversationReadState,
    },
    /// Response `chat.room.enter` snapshot.
    RoomEnter {
        sequence: u32,
        presence: Vec<PresenceEntry>,
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
    AgentSpecSync {
        sequence: u32,
        records: Vec<AgentSpecRecord>,
        accounts: Vec<AgentProviderAccount>,
    },
    AgentSpecUpsert {
        sequence: u32,
        record: AgentSpecRecord,
    },
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AgentSpecRecord {
    pub profile_id: String,
    pub nickname: String,
    pub server_account: String,
    pub spec: Vec<u8>,
    pub key_ciphertext: Vec<u8>,
    pub updated_at: i64,
    pub deleted_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AgentProviderAccount {
    pub id: String,
    pub vendor_id: String,
    pub base_url: String,
    pub key_ref: String,
    pub display_name: String,
    pub models: Vec<String>,
    pub updated_at: i64,
    pub deleted_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PresenceEntry {
    pub account: String,
    pub status: i32,
    pub last_seen: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BotPendingItem {
    pub message_id: i64,
    pub body: String,
    pub send_time: i64,
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
