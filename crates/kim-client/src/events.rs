/// Result of a `chat.user.talk` / `chat.group.talk` Response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TalkResult {
    pub message_id: i64,
    pub send_time: i64,
    pub sequence: u32,
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
