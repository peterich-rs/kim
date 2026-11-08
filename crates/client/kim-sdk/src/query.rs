use std::sync::Arc;

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::store::changes::{ChangeLog, ChangedQuery};
use crate::{KimSdk, TimelineUpdate};

#[derive(Clone)]
pub(crate) struct TimelineSub {
    pub account: String,
    pub epoch: u64,
    pub dest: String,
    pub limit: i32,
    pub tx: watch::Sender<TimelineUpdate>,
    /// Oldest sent row included in the expanded local window.
    pub older_bound: Option<(i64, String)>,
    /// The continuation state belongs to the SDK subscription, not Dart.
    pub has_more: bool,
    pub loading_older: bool,
    pub history_error: Option<String>,
    /// Latest-page hydrate in flight (open-thread catch-up, not load_older).
    pub hydrating: bool,
}

/// Refreshes subscribed query snapshots after committed store writes.
///
/// The task holds a [`std::sync::Weak`] to `Inner` so it cannot pin the SDK
/// or SQLite pool after the last `KimSdk` is dropped. `store_life` is cancelled
/// from `Inner::drop`; session reconnects must not stop this task.
pub(crate) fn spawn_query_publisher(
    sdk: KimSdk,
    changes: Arc<ChangeLog>,
    store_life: CancellationToken,
) {
    let weak = sdk.downgrade();
    drop(sdk);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = store_life.cancelled() => break,
                notice = changes.take() => {
                    let mut notice = notice;
                    // Drain writes that landed during `take()`. No sleep:
                    // command `wait_applied` must not pay a fixed batch
                    // window. Further commits during refresh coalesce into
                    // the next `take()`.
                    drain_pending(&changes, &mut notice);
                    tracing::debug!(
                        account = %notice.account,
                        epoch = notice.epoch,
                        sequence = notice.sequence,
                        queries = notice.queries.len(),
                        "refreshing committed queries"
                    );
                    let Some(inner) = weak.upgrade() else {
                        changes.mark_applied(notice.sequence);
                        break;
                    };
                    let sdk = KimSdk::from_inner(inner);
                    for query in notice.queries {
                        if store_life.is_cancelled() {
                            break;
                        }
                        match query {
                            ChangedQuery::Inbox => sdk.refresh_session_snapshot().await,
                            ChangedQuery::Timeline { dest } => {
                                sdk.refresh_timeline(&dest, notice.epoch, &notice.account).await;
                            }
                            ChangedQuery::Contacts => {
                                sdk.refresh_contacts_view(
                                    notice.epoch,
                                    notice.clear_contacts_error,
                                )
                                .await;
                            }
                        }
                    }
                    // Always advance the barrier, including on shutdown, so
                    // `wait_applied` cannot sit until the 2s timeout.
                    changes.mark_applied(notice.sequence);
                    if store_life.is_cancelled() {
                        break;
                    }
                    tracing::debug!(
                        account = %notice.account,
                        epoch = notice.epoch,
                        sequence = notice.sequence,
                        commit_to_snapshot_ms = notice.recorded_at.elapsed().as_millis(),
                        "committed query refresh complete"
                    );
                }
            }
        }
    });
}

fn drain_pending(changes: &ChangeLog, notice: &mut crate::store::changes::CommitNotice) {
    while let Some(next) = changes.take_pending() {
        if next.account == notice.account && next.epoch == notice.epoch {
            notice.queries.extend(next.queries);
            notice.sequence = next.sequence;
            notice.clear_contacts_error |= next.clear_contacts_error;
        } else {
            *notice = next;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use crate::KimSdk;

    #[tokio::test]
    async fn publisher_does_not_hold_strong_inner() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("kim-cache.db");
        let sdk = KimSdk::open(path.to_string_lossy().into_owned())
            .await
            .expect("open");
        assert_eq!(
            sdk.debug_inner_strong_count(),
            1,
            "query publisher must not pin Inner"
        );
    }
}
