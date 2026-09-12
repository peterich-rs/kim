use crate::media::MediaRef;

#[derive(Clone, Debug)]
pub struct StartSession {
    pub url: String,
    pub token: String,
    pub user_agent: String,
    pub account: String,
}

#[derive(Clone, Debug)]
pub enum OutgoingPayload {
    Text { body: String },
    Image { media: MediaRef },
    Video { url: String, extra: String },
}

#[derive(Clone, Debug)]
pub struct SendMessageCommand {
    pub dest: String,
    pub kind: i32,
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
    pub kind: i32,
    pub visible_message_id: i64,
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

impl OutgoingPayload {
    #[must_use]
    pub fn payload_type(&self) -> i32 {
        match self {
            Self::Text { .. } => kim_protocol::MESSAGE_TYPE_TEXT,
            Self::Image { .. } => kim_protocol::MESSAGE_TYPE_IMAGE,
            Self::Video { .. } => kim_protocol::MESSAGE_TYPE_VIDEO,
        }
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::Image { .. } => "image",
            Self::Video { .. } => "video",
        }
    }
}
