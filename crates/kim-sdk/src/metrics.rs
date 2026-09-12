use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct SdkMetrics {
    pub enqueue_total: AtomicU64,
    pub persist_talk_total: AtomicU64,
    pub epoch_drop_total: AtomicU64,
    #[allow(dead_code)]
    pub timeline_resync_total: AtomicU64,
}

impl SdkMetrics {
    pub fn inc_enqueue(&self) {
        self.enqueue_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_persist_talk(&self) {
        self.persist_talk_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_epoch_drop(&self) {
        self.epoch_drop_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.enqueue_total.load(Ordering::Relaxed),
            self.persist_talk_total.load(Ordering::Relaxed),
            self.epoch_drop_total.load(Ordering::Relaxed),
        )
    }
}
