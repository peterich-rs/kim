use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::Notify;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ChangedQuery {
    Timeline { dest: String },
    Inbox,
    Contacts,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CommitEffect {
    pub queries: BTreeSet<ChangedQuery>,
    /// Successful `replace_contacts` may clear a published sync error.
    /// Other contacts writes keep the last error until the next replace.
    pub clear_contacts_error: bool,
}

impl CommitEffect {
    pub fn empty() -> Self {
        Self {
            queries: BTreeSet::new(),
            clear_contacts_error: false,
        }
    }

    pub fn timeline(dest: impl Into<String>) -> Self {
        let mut e = Self::empty();
        e.queries
            .insert(ChangedQuery::Timeline { dest: dest.into() });
        e
    }

    pub fn inbox() -> Self {
        let mut e = Self::empty();
        e.queries.insert(ChangedQuery::Inbox);
        e
    }

    pub fn contacts() -> Self {
        let mut e = Self::empty();
        e.queries.insert(ChangedQuery::Contacts);
        e
    }

    pub fn contacts_replaced() -> Self {
        let mut e = Self::contacts();
        e.clear_contacts_error = true;
        e
    }

    pub fn merge(&mut self, other: CommitEffect) {
        self.queries.extend(other.queries);
        self.clear_contacts_error |= other.clear_contacts_error;
    }

    pub fn is_empty(&self) -> bool {
        self.queries.is_empty()
    }
}

/// Process-local notice. `sequence` is NOT a server cursor.
#[derive(Clone, Debug)]
pub(crate) struct CommitNotice {
    pub account: String,
    pub epoch: u64,
    pub sequence: u64,
    /// Monotonic commit time of the oldest coalesced change.
    pub recorded_at: Instant,
    pub queries: BTreeSet<ChangedQuery>,
    pub clear_contacts_error: bool,
}

pub(crate) struct ChangeLog {
    inner: Mutex<Option<CommitNotice>>,
    notify: Notify,
    sequence: AtomicU64,
    applied: AtomicU64,
    applied_notify: Notify,
}

impl ChangeLog {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            notify: Notify::new(),
            sequence: AtomicU64::new(0),
            applied: AtomicU64::new(0),
            applied_notify: Notify::new(),
        }
    }

    /// Call only after COMMIT, never while holding a DB connection across await.
    pub fn record(&self, account: String, epoch: u64, effect: CommitEffect) -> u64 {
        if effect.is_empty() {
            return self.sequence.load(Ordering::SeqCst);
        }
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match g.as_mut() {
            Some(n) if n.epoch == epoch && n.account == account => {
                n.queries.extend(effect.queries);
                n.sequence = seq;
                n.clear_contacts_error |= effect.clear_contacts_error;
            }
            _ => {
                *g = Some(CommitNotice {
                    account,
                    epoch,
                    sequence: seq,
                    recorded_at: Instant::now(),
                    queries: effect.queries,
                    clear_contacts_error: effect.clear_contacts_error,
                });
            }
        }
        drop(g);
        self.notify.notify_one();
        seq
    }

    /// Subscribe **before** checking the slot. Tokio `Notify` does not
    /// store a permit for a waiter that has not yet subscribed.
    pub async fn take(&self) -> CommitNotice {
        loop {
            let notified = self.notify.notified();
            if let Some(n) = self.inner.lock().unwrap_or_else(|e| e.into_inner()).take() {
                return n;
            }
            notified.await;
        }
    }

    pub fn take_pending(&self) -> Option<CommitNotice> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub fn mark_applied(&self, seq: u64) {
        self.applied.store(seq, Ordering::SeqCst);
        self.applied_notify.notify_waiters();
    }

    pub fn applied(&self) -> u64 {
        self.applied.load(Ordering::SeqCst)
    }

    pub async fn wait_applied(&self, seq: u64, timeout: std::time::Duration) -> bool {
        tokio::time::timeout(timeout, async {
            loop {
                let notified = self.applied_notify.notified();
                if self.applied() >= seq {
                    return;
                }
                notified.await;
            }
        })
        .await
        .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn empty_effect_does_not_bump_sequence() {
        let log = ChangeLog::new();
        assert_eq!(log.record("a".into(), 1, CommitEffect::empty()), 0);
        assert_eq!(log.sequence.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn coalesce_same_epoch_account() {
        let log = ChangeLog::new();
        let s1 = log.record("a".into(), 1, CommitEffect::timeline("bob"));
        let s2 = log.record("a".into(), 1, CommitEffect::inbox());
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
        let mut g = log.inner.lock().unwrap();
        let n = g.take().expect("notice");
        assert_eq!(n.sequence, 2);
        assert!(n
            .queries
            .contains(&ChangedQuery::Timeline { dest: "bob".into() }));
        assert!(n.queries.contains(&ChangedQuery::Inbox));
    }

    #[test]
    fn epoch_replace_clears_old_queries() {
        let log = ChangeLog::new();
        log.record("a".into(), 1, CommitEffect::timeline("bob"));
        log.record("a".into(), 2, CommitEffect::contacts());
        let mut g = log.inner.lock().unwrap();
        let n = g.take().expect("notice");
        assert_eq!(n.epoch, 2);
        assert_eq!(n.queries.len(), 1);
        assert!(n.queries.contains(&ChangedQuery::Contacts));
    }

    #[tokio::test]
    async fn changelog_no_lost_wakeup() {
        let log = std::sync::Arc::new(ChangeLog::new());
        let log2 = log.clone();
        let waiter = tokio::spawn(async move { log2.take().await });
        // Let take() subscribe before we record.
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        let seq = log.record("a".into(), 1, CommitEffect::inbox());
        let notice = tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("join")
            .expect("task");
        assert_eq!(notice.sequence, seq);
        assert!(notice.queries.contains(&ChangedQuery::Inbox));
    }
}
