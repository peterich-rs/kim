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
}

impl Profile {
    pub fn from_wire(account: String, nickname: String) -> Self {
        let nickname = if nickname.is_empty() {
            account.clone()
        } else {
            nickname
        };
        Self { account, nickname }
    }

    pub fn encode_list(users: &[Self]) -> Result<String, String> {
        serde_json::to_string(users).map_err(|e| e.to_string())
    }
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
    UserList {
        command: String,
        sequence: u32,
        users: Vec<Profile>,
    },
    Status {
        command: String,
        status: i32,
        sequence: u32,
    },
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncomingTalk {
    pub command: String,
    pub message_id: i64,
    pub sender: String,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
    pub send_time: i64,
}
