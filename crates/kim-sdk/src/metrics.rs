use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct SdkMetrics {
    pub enqueue_total: AtomicU64,
    pub persist_talk_total: AtomicU64,
    pub epoch_drop_total: AtomicU64,
    pub timeline_resync_total: AtomicU64,
    pub store_wipe_total: AtomicU64,
    query_refresh_total: AtomicU64,
    query_stale_skip_total: AtomicU64,
    query_apply_wait_timeout_total: AtomicU64,
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

    pub fn inc_store_wipe(&self) {
        self.store_wipe_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_timeline_resync(&self) {
        self.timeline_resync_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_query_refresh(&self) {
        self.query_refresh_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_query_stale_skip(&self) {
        self.query_stale_skip_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_query_apply_wait_timeout(&self) {
        self.query_apply_wait_timeout_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn store_wipe_total(&self) -> u64 {
        self.store_wipe_total.load(Ordering::Relaxed)
    }

    pub fn query_refresh_total(&self) -> u64 {
        self.query_refresh_total.load(Ordering::Relaxed)
    }

    pub fn query_stale_skip_total(&self) -> u64 {
        self.query_stale_skip_total.load(Ordering::Relaxed)
    }

    pub fn query_apply_wait_timeout_total(&self) -> u64 {
        self.query_apply_wait_timeout_total.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> (u64, u64, u64, u64) {
        (
            self.enqueue_total.load(Ordering::Relaxed),
            self.persist_talk_total.load(Ordering::Relaxed),
            self.epoch_drop_total.load(Ordering::Relaxed),
            self.store_wipe_total(),
        )
    }
}
