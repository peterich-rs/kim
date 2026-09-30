use crate::model::{OutgoingContent, ThreadKind};

#[derive(Clone, Debug)]
pub struct StartSession {
    pub url: String,
    pub token: String,
    pub user_agent: String,
    pub account: String,
}

pub type OutgoingPayload = OutgoingContent;

#[derive(Clone, Debug)]
pub struct SendMessageCommand {
    pub dest: String,
    pub kind: ThreadKind,
    pub payload: OutgoingPayload,
    pub client_id: Option<String>,
    pub batch_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CommandReceipt {
    pub request_id: String,
    pub client_id: String,
    pub dest: String,
    pub accepted_at: i64,
    pub send_status: SendStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendStatus {
    Pending,
    Uploading,
    Sending,
    Sent,
    Failed,
    Cancelled,
}

impl SendStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Uploading => "uploading",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    #[must_use]
    pub fn from_db(raw: &str) -> Self {
        match raw {
            "pending" | "sending" => Self::Pending,
            "uploading" => Self::Uploading,
            "sent" => Self::Sent,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Sent,
        }
    }

    #[must_use]
    pub fn message_status(self) -> &'static str {
        match self {
            Self::Pending | Self::Uploading | Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReadMarker {
    pub dest: String,
    pub kind: ThreadKind,
    pub visible_message_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationKey {
    pub dest: String,
    pub kind: ThreadKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConversationVisibility {
    pub generation: u64,
    pub foreground: bool,
    pub conversation: Option<ConversationKey>,
}

#[derive(Clone, Debug)]
pub struct TimelineQuery {
    pub dest: String,
    pub limit: i32,
}

#[derive(Clone, Debug)]
pub struct PageCursor {
    pub dest: String,
    pub before_at: i64,
    pub before_key: String,
    pub limit: i32,
    pub before_id: i64,
}

#[derive(Clone, Debug)]
pub struct MessagePage {
    pub dest: String,
    pub messages: Vec<crate::timeline::MessageView>,
    pub has_more: bool,
}
