use std::sync::Mutex;

use tokio::sync::mpsc;

/// Coarse presence for the desktop pet. Transcript bytes stay in Rust.
#[derive(Clone, Debug)]
pub struct AgentUiStatus {
    pub dest: String,
    pub phase: String,
}

pub struct UiBus {
    listeners: Mutex<Vec<mpsc::UnboundedSender<AgentUiStatus>>>,
}

impl UiBus {
    #[must_use]
    pub fn new() -> Self {
        Self {
            listeners: Mutex::new(Vec::new()),
        }
    }

    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<AgentUiStatus> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.listeners
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(tx);
        rx
    }

    pub fn publish(&self, status: AgentUiStatus) {
        let mut listeners = self.listeners.lock().unwrap_or_else(|err| err.into_inner());
        listeners.retain(|slot| slot.send(status.clone()).is_ok());
    }
}

impl Default for UiBus {
    fn default() -> Self {
        Self::new()
    }
}
