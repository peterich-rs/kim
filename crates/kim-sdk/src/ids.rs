//! Newtypes for account, dest, client message id, and session epoch.

pub use kim_protocol::{AccountId, DestId};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClientMessageId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionEpoch(pub u64);

impl ClientMessageId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Incoming/history rows use `m{messageId}` so they never collide with a UUID client id.
#[must_use]
pub fn incoming_message_key(message_id: i64, send_time: i64, sender: &str) -> String {
    if message_id != 0 {
        return format!("m{message_id}");
    }
    format!("talk-{send_time}-{sender}")
}

/// UUID v4 (8-4-4-4-12) used as the outbox clientId / row key.
#[must_use]
pub fn is_client_key(key: &str) -> bool {
    let b = key.as_bytes();
    b.len() == 36 && b[8] == b'-' && b[13] == b'-' && b[18] == b'-' && b[23] == b'-'
}

/// Prefer the local UUID row when a history/push row shares a message_id.
#[must_use]
pub fn prefer_key<'a>(a: &'a str, b: &'a str) -> &'a str {
    if is_client_key(a) && !is_client_key(b) {
        a
    } else if is_client_key(b) && !is_client_key(a) {
        b
    } else {
        a
    }
}
