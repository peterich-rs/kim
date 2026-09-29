//! Closed sets shared by the store and FFI. Protocol integers stay at the
//! sqlite bind and `kim-client` call.

use crate::error::SdkError;

pub const MEDIA_HOST: &str = "media.kim.ainexc.com";
const PREVIEW_MAX: usize = 36;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThreadKind {
    User,
    Group,
}

impl ThreadKind {
    #[must_use]
    pub fn as_wire(self) -> i32 {
        match self {
            Self::User => kim_protocol::INBOX_KIND_USER,
            Self::Group => kim_protocol::INBOX_KIND_GROUP,
        }
    }

    #[must_use]
    pub fn from_wire(kind: i32) -> Self {
        if kind == kim_protocol::INBOX_KIND_GROUP {
            Self::Group
        } else {
            Self::User
        }
    }

    #[must_use]
    pub fn as_db(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Group => "group",
        }
    }

    #[must_use]
    pub fn from_db(name: &str) -> Self {
        if name == "group" {
            Self::Group
        } else {
            Self::User
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaKind {
    Text,
    Image,
    Video,
    Voice,
    Card,
}

impl MediaKind {
    #[must_use]
    pub fn as_wire(self) -> i32 {
        match self {
            Self::Text | Self::Card => kim_protocol::MESSAGE_TYPE_TEXT,
            Self::Image => kim_protocol::MESSAGE_TYPE_IMAGE,
            Self::Video => kim_protocol::MESSAGE_TYPE_VIDEO,
            Self::Voice => kim_protocol::MESSAGE_TYPE_VOICE,
        }
    }

    #[must_use]
    pub fn from_wire(kind: i32) -> Self {
        match kind {
            kim_protocol::MESSAGE_TYPE_IMAGE => Self::Image,
            kim_protocol::MESSAGE_TYPE_VIDEO => Self::Video,
            kim_protocol::MESSAGE_TYPE_VOICE => Self::Voice,
            _ => Self::Text,
        }
    }

    #[must_use]
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Video => "video",
            Self::Voice => "voice",
            Self::Card => "card",
        }
    }

    pub fn from_db(name: &str) -> Result<Self, SdkError> {
        match name {
            "text" => Ok(Self::Text),
            "image" => Ok(Self::Image),
            "video" => Ok(Self::Video),
            "voice" => Ok(Self::Voice),
            "card" => Ok(Self::Card),
            other => Err(SdkError::InvalidArgument {
                message: format!("unknown media kind {other}"),
            }),
        }
    }

    #[must_use]
    pub fn is_media_preview(self) -> bool {
        matches!(self, Self::Image | Self::Video | Self::Voice | Self::Card)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Relation {
    Friend,
    Incoming,
    Outgoing,
}

impl Relation {
    #[must_use]
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Friend => "friend",
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
        }
    }

    pub fn from_db(name: &str) -> Result<Self, SdkError> {
        match name {
            "friend" => Ok(Self::Friend),
            "incoming" => Ok(Self::Incoming),
            "outgoing" => Ok(Self::Outgoing),
            other => Err(SdkError::InvalidArgument {
                message: format!("unknown relation {other}"),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProfileKind {
    User,
    Bot,
}

impl ProfileKind {
    #[must_use]
    pub fn as_wire(self) -> i32 {
        match self {
            Self::User => kim_protocol::PROFILE_KIND_USER,
            Self::Bot => kim_protocol::PROFILE_KIND_BOT,
        }
    }

    #[must_use]
    pub fn from_wire(kind: i32) -> Self {
        if kind == kim_protocol::PROFILE_KIND_BOT {
            Self::Bot
        } else {
            Self::User
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProfilePlacement {
    Local,
    Cloud,
}

impl ProfilePlacement {
    #[must_use]
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Cloud => "cloud",
        }
    }

    #[must_use]
    pub fn from_db(name: &str) -> Self {
        if name.trim().eq_ignore_ascii_case("cloud") {
            Self::Cloud
        } else {
            Self::Local
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreadPreview {
    Text { snippet: String },
    Media { kind: MediaKind },
    System { text: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutgoingContent {
    Text {
        body: String,
    },
    Image {
        path: String,
        mime: String,
        width: i32,
        height: i32,
        byte_size: i64,
    },
    Video {
        path: String,
        byte_size: i64,
    },
    Voice {
        path: String,
        byte_size: i64,
    },
}

impl OutgoingContent {
    #[must_use]
    pub fn media_kind(&self) -> MediaKind {
        match self {
            Self::Text { .. } => MediaKind::Text,
            Self::Image { .. } => MediaKind::Image,
            Self::Video { .. } => MediaKind::Video,
            Self::Voice { .. } => MediaKind::Voice,
        }
    }

    #[must_use]
    pub fn payload_type(&self) -> i32 {
        self.media_kind().as_wire()
    }

    #[must_use]
    pub fn body(&self) -> &str {
        match self {
            Self::Text { body }
            | Self::Image { path: body, .. }
            | Self::Video { path: body, .. }
            | Self::Voice { path: body, .. } => body,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentCardType {
    Tool,
    ActionRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentCardState {
    Pending,
    Ok,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentCard {
    pub call_id: String,
    pub name: String,
    pub card_type: AgentCardType,
    pub state: AgentCardState,
    pub preview: String,
    pub ok: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassifiedMessage {
    pub kind: MediaKind,
    pub card: Option<AgentCard>,
}

/// One classification at ingest. `msg_type` wins for voice/image/video.
/// Text (and unknown) bodies may be a card envelope or a media URL.
#[must_use]
pub fn classify_message(msg_type: i32, body: &str) -> ClassifiedMessage {
    match msg_type {
        kim_protocol::MESSAGE_TYPE_IMAGE => ClassifiedMessage {
            kind: MediaKind::Image,
            card: None,
        },
        kim_protocol::MESSAGE_TYPE_VIDEO => ClassifiedMessage {
            kind: MediaKind::Video,
            card: None,
        },
        kim_protocol::MESSAGE_TYPE_VOICE => ClassifiedMessage {
            kind: MediaKind::Voice,
            card: None,
        },
        _ => {
            if let Some(card) = parse_agent_card(body) {
                ClassifiedMessage {
                    kind: MediaKind::Card,
                    card: Some(card),
                }
            } else if let Some(kind) = media_kind_from_url(body) {
                ClassifiedMessage { kind, card: None }
            } else {
                ClassifiedMessage {
                    kind: MediaKind::Text,
                    card: None,
                }
            }
        }
    }
}

#[must_use]
pub fn parse_agent_card(body: &str) -> Option<AgentCard> {
    let value: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    let obj = value.as_object()?;
    let type_raw = obj.get("type").and_then(serde_json::Value::as_str);
    let card_type = match type_raw {
        Some("action_required") => AgentCardType::ActionRequired,
        Some("tool") => AgentCardType::Tool,
        None if obj.contains_key("call_id") => AgentCardType::Tool,
        _ => return None,
    };
    let state_raw = obj
        .get("state")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("pending");
    let state = match state_raw {
        "ok" => AgentCardState::Ok,
        "error" => AgentCardState::Error,
        _ => AgentCardState::Pending,
    };
    let ok = obj.get("ok").and_then(serde_json::Value::as_bool) == Some(true)
        || state == AgentCardState::Ok;
    Some(AgentCard {
        call_id: json_string(obj.get("call_id")),
        name: json_string(obj.get("name")),
        card_type,
        state,
        preview: json_string(obj.get("preview")),
        ok,
    })
}

fn json_string(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

#[must_use]
pub fn media_kind_from_url(body: &str) -> Option<MediaKind> {
    let url = body.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return extension_kind(url);
    }
    let path = url.split(['?', '#']).next().unwrap_or(url);
    if let Some(kind) = extension_kind(path) {
        return Some(kind);
    }
    let host = url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .unwrap_or("");
    if host.eq_ignore_ascii_case(MEDIA_HOST) {
        Some(MediaKind::Image)
    } else {
        None
    }
}

fn extension_kind(path: &str) -> Option<MediaKind> {
    let lower = path.to_ascii_lowercase();
    let stem = lower.split(['?', '#']).next().unwrap_or(&lower);
    if stem.ends_with(".mp4")
        || stem.ends_with(".mov")
        || stem.ends_with(".webm")
        || stem.ends_with(".m4v")
    {
        return Some(MediaKind::Video);
    }
    if stem.ends_with(".jpg")
        || stem.ends_with(".jpeg")
        || stem.ends_with(".png")
        || stem.ends_with(".webp")
        || stem.ends_with(".gif")
    {
        return Some(MediaKind::Image);
    }
    None
}

#[must_use]
pub fn thread_preview(sys: bool, kind: MediaKind, body: &str) -> ThreadPreview {
    if sys {
        return ThreadPreview::System {
            text: body.to_string(),
        };
    }
    if kind.is_media_preview() {
        return ThreadPreview::Media { kind };
    }
    ThreadPreview::Text {
        snippet: truncate_preview(body),
    }
}

#[must_use]
pub fn truncate_preview(body: &str) -> String {
    let mut collapsed = String::new();
    let mut pending_space = false;
    for ch in body.trim().chars() {
        if ch.is_whitespace() {
            pending_space = !collapsed.is_empty();
            continue;
        }
        if pending_space {
            collapsed.push(' ');
            pending_space = false;
        }
        collapsed.push(ch);
    }
    let count = collapsed.chars().count();
    if count <= PREVIEW_MAX {
        return collapsed;
    }
    let truncated: String = collapsed.chars().take(PREVIEW_MAX).collect();
    format!("{truncated}…")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentFlagRow {
    pub multi_profile: bool,
    pub server_identity: bool,
}

impl AgentFlagRow {
    #[must_use]
    pub fn from_json(raw: &str) -> Self {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
            return Self::default();
        };
        Self {
            multi_profile: value
                .get("multi_profile")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            server_identity: value
                .get("server_identity")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        }
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "multi_profile": self.multi_profile,
            "server_identity": self.server_identity,
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_message, parse_agent_card, thread_preview, truncate_preview, AgentCardState,
        AgentCardType, MediaKind, ThreadPreview,
    };

    #[test]
    fn classify_wire_types() {
        assert_eq!(
            classify_message(kim_protocol::MESSAGE_TYPE_TEXT, "hello").kind,
            MediaKind::Text
        );
        assert_eq!(
            classify_message(kim_protocol::MESSAGE_TYPE_IMAGE, "https://x/a.bin").kind,
            MediaKind::Image
        );
        assert_eq!(
            classify_message(kim_protocol::MESSAGE_TYPE_VOICE, "https://x/a.mp3").kind,
            MediaKind::Voice
        );
        assert_eq!(
            classify_message(kim_protocol::MESSAGE_TYPE_VIDEO, "note").kind,
            MediaKind::Video
        );
    }

    #[test]
    fn classify_media_host_and_query_extension() {
        let host = classify_message(
            kim_protocol::MESSAGE_TYPE_TEXT,
            "https://media.kim.ainexc.com/obj/1",
        );
        assert_eq!(host.kind, MediaKind::Image);
        let video = classify_message(1, "https://cdn.example/clip.mp4?token=1");
        assert_eq!(video.kind, MediaKind::Video);
        let image = classify_message(1, "https://cdn.example/a.jpeg?x=1");
        assert_eq!(image.kind, MediaKind::Image);
    }

    #[test]
    fn classify_card_envelope_and_missing_size_stays_image() {
        let raw = r#"{"v":1,"type":"tool","call_id":"c1","name":"shell","state":"ok","preview":"ls","ok":true}"#;
        let classified = classify_message(1, raw);
        assert_eq!(classified.kind, MediaKind::Card);
        let card = classified.card.expect("card");
        assert_eq!(card.call_id, "c1");
        assert_eq!(card.card_type, AgentCardType::Tool);
        assert_eq!(card.state, AgentCardState::Ok);
        assert!(card.ok);
        assert!(parse_agent_card(r#"{"w":1,"h":2}"#).is_none());
        assert_eq!(
            classify_message(kim_protocol::MESSAGE_TYPE_IMAGE, "x").kind,
            MediaKind::Image
        );
    }

    #[test]
    fn thread_preview_system_media_and_truncation() {
        assert_eq!(
            thread_preview(true, MediaKind::Text, "joined"),
            ThreadPreview::System {
                text: "joined".into()
            }
        );
        assert_eq!(
            thread_preview(
                false,
                MediaKind::Image,
                "https://media.kim.ainexc.com/a.png"
            ),
            ThreadPreview::Media {
                kind: MediaKind::Image
            }
        );
        let long = "a".repeat(40);
        match thread_preview(false, MediaKind::Text, &long) {
            ThreadPreview::Text { snippet } => {
                assert_eq!(snippet.chars().count(), 37);
                assert!(snippet.ends_with('…'));
            }
            other => panic!("expected text, got {other:?}"),
        }
        assert_eq!(truncate_preview("  hello   world  "), "hello world");
    }
}
