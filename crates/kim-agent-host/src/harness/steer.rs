use std::collections::HashMap;

#[derive(Default)]
pub struct SteerInbox {
    pending: HashMap<String, Vec<String>>,
}

impl SteerInbox {
    pub fn push(&mut self, session_id: &str, text: String) {
        self.pending
            .entry(session_id.to_string())
            .or_default()
            .push(text);
    }

    pub fn drain(&mut self, session_id: &str) -> Vec<String> {
        self.pending.remove(session_id).unwrap_or_default()
    }
}
