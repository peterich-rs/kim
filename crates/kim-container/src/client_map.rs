use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use kim_naming::DefaultRegistration;
use kim_tcp::TcpClient;

pub const YOUNG: u8 = 0;
pub const ADULT: u8 = 1;

pub struct ClientSlot {
    pub reg: DefaultRegistration,
    pub client: Arc<TcpClient>,
    pub state: Arc<AtomicU8>,
}

#[derive(Default)]
pub struct ClientMap {
    inner: HashMap<String, ClientSlot>,
}

impl ClientMap {
    pub fn get(&self, id: &str) -> Option<&ClientSlot> {
        self.inner.get(id)
    }

    pub fn insert(&mut self, slot: ClientSlot) {
        self.inner.insert(slot.reg.service_id.clone(), slot);
    }

    pub fn contains(&self, id: &str) -> bool {
        self.inner.contains_key(id)
    }

    pub fn adult_services(&self) -> Vec<DefaultRegistration> {
        let mut v: Vec<_> = self
            .inner
            .values()
            .filter(|s| s.state.load(Ordering::SeqCst) == ADULT)
            .map(|s| s.reg.clone())
            .collect();
        v.sort_by(|a, b| a.service_id.cmp(&b.service_id));
        v
    }

}
