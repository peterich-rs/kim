/// In-memory session after a successful `login.signin`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemorySession {
    pub channel_id: String,
    pub account: String,
    pub token: String,
}

impl MemorySession {
    pub fn is_logged_in(&self) -> bool {
        !self.channel_id.is_empty()
    }
}
