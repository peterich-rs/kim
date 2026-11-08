use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Debug)]
pub struct PermissionEvent {
    pub dest: String,
    pub call_id: String,
    pub name: String,
    pub preview: String,
}

pub struct PermissionMailbox {
    waiters: Mutex<HashMap<String, oneshot::Sender<bool>>>,
    listeners: Mutex<Vec<mpsc::UnboundedSender<PermissionEvent>>>,
}

impl PermissionMailbox {
    #[must_use]
    pub fn new() -> Self {
        Self {
            waiters: Mutex::new(HashMap::new()),
            listeners: Mutex::new(Vec::new()),
        }
    }

    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<PermissionEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.listeners
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(tx);
        rx
    }

    pub fn publish(&self, event: PermissionEvent) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        self.waiters
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(event.call_id.clone(), tx);
        let mut listeners = self.listeners.lock().unwrap_or_else(|err| err.into_inner());
        listeners.retain(|slot| slot.send(event.clone()).is_ok());
        rx
    }

    pub fn respond(&self, call_id: &str, allow: bool) {
        if let Some(tx) = self
            .waiters
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(call_id)
        {
            let _ = tx.send(allow);
        }
    }
}

impl Default for PermissionMailbox {
    fn default() -> Self {
        Self::new()
    }
}
