//! Offline catch-up: inbox.list then offline.index/content pages, persist-then-ack.

use std::collections::{HashMap, HashSet};
use tokio::sync::{broadcast, watch, Notify};
use tracing::debug;

use crate::events::{IncomingTalk, Message, MessageIndex};
use crate::supervisor::SessionEvent;
use crate::ClientError;
use crate::KimClient;
use kim_protocol::{CMD_CHAT_GROUP_TALK, CMD_CHAT_USER_TALK};

pub(crate) const SYNC_PAGE: usize = 200;
pub(crate) const INBOX_LIMIT: i32 = 200;

/// Dart persist-then-ack gate. `sync_confirm(cursor)` releases pages with max id ≤ cursor.
#[derive(Clone)]
pub(crate) struct ConfirmGate {
    tx: watch::Sender<i64>,
}

impl ConfirmGate {
    pub(crate) fn new() -> Self {
        let (tx, _) = watch::channel(0i64);
        Self { tx }
    }

    pub(crate) fn confirm(&self, cursor: i64) {
        self.tx.send_if_modified(|cur| {
            if cursor > *cur {
                *cur = cursor;
                true
            } else {
                false
            }
        });
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<i64> {
        self.tx.subscribe()
    }
}

pub(crate) async fn wait_confirm(
    rx: &mut watch::Receiver<i64>,
    needed: i64,
    stop: &Notify,
) -> Result<(), ClientError> {
    if needed <= 0 {
        return Ok(());
    }
    tokio::select! {
        result = rx.wait_for(|v| *v >= needed) => {
            result.map(|_| ()).map_err(|_| ClientError::other("confirm closed"))
        }
        _ = stop.notified() => Err(ClientError::other("stopped")),
    }
}

pub(crate) struct SyncEngine {
    seen: HashSet<i64>,
}

impl SyncEngine {
    pub(crate) fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    /// Returns true if this `message_id` was not seen before (should emit).
    pub(crate) fn observe(&mut self, message_id: i64) -> bool {
        if message_id == 0 {
            return true;
        }
        self.seen.insert(message_id)
    }

    pub(crate) async fn run(
        &mut self,
        client: &KimClient,
        events: &broadcast::Sender<SessionEvent>,
        confirm: &ConfirmGate,
        stop: &Notify,
    ) -> Result<usize, ClientError> {
        let account = client.session().account;
        let items = client.inbox_list(INBOX_LIMIT).await?;
        let _ = events.send(SessionEvent::Inbox(items));

        let mut pulled = 0usize;
        let mut confirm_rx = confirm.subscribe();
        loop {
            let indexes = client.offline_index().await?;
            if indexes.is_empty() {
                break;
            }
            let short = indexes.len() < SYNC_PAGE;
            let ids: Vec<i64> = indexes
                .iter()
                .map(|i| i.message_id)
                .take(SYNC_PAGE)
                .collect();
            let msgs = client.offline_content(&ids).await?;
            let talks = merge_offline(&account, &indexes, &msgs, &mut self.seen);
            let new_count = talks.len();
            for talk in talks {
                let _ = events.send(SessionEvent::Talk(talk));
            }
            pulled += new_count;
            let max_id = ids.iter().copied().max().unwrap_or(0);
            if new_count > 0 && max_id > 0 {
                let _ = events.send(SessionEvent::SyncProgress {
                    pulled,
                    page_pending: true,
                });
                wait_confirm(&mut confirm_rx, max_id, stop).await?;
            }
            client.ack_batch(&ids).await?;
            let _ = events.send(SessionEvent::SyncProgress {
                pulled,
                page_pending: false,
            });
            if short {
                break;
            }
        }
        let _ = events.send(SessionEvent::SyncDone { pulled });
        debug!(pulled, "sync done");
        Ok(pulled)
    }
}

fn merge_offline(
    account: &str,
    indexes: &[MessageIndex],
    msgs: &[Message],
    seen: &mut HashSet<i64>,
) -> Vec<IncomingTalk> {
    let by_id: HashMap<i64, &Message> = msgs.iter().map(|m| (m.message_id, m)).collect();
    let mut talks = Vec::new();
    for idx in indexes.iter().take(SYNC_PAGE) {
        let Some(msg) = by_id.get(&idx.message_id) else {
            continue;
        };
        if idx.message_id != 0 && !seen.insert(idx.message_id) {
            continue;
        }
        talks.push(talk_from_offline(account, idx, msg));
    }
    talks
}

fn talk_from_offline(account: &str, idx: &MessageIndex, msg: &Message) -> IncomingTalk {
    let group = !idx.group.is_empty();
    let dest = if group {
        idx.group.clone()
    } else {
        idx.account_b.clone()
    };
    let command = if group {
        CMD_CHAT_GROUP_TALK
    } else {
        CMD_CHAT_USER_TALK
    };
    // direction 1 = send (self), 0 = recv (peer / original sender in account_b).
    let sender = if idx.direction == 1 {
        account.to_string()
    } else {
        idx.account_b.clone()
    };
    IncomingTalk {
        command: command.to_string(),
        dest,
        message_id: msg.message_id,
        sender,
        msg_type: msg.msg_type,
        body: msg.body.clone(),
        extra: msg.extra.clone(),
        send_time: idx.send_time,
    }
}

pub(crate) fn next_backoff(current: std::time::Duration) -> std::time::Duration {
    current
        .saturating_mul(2)
        .min(std::time::Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_caps_at_60s() {
        let mut d = std::time::Duration::from_secs(1);
        for _ in 0..20 {
            d = next_backoff(d);
            assert!(d <= std::time::Duration::from_secs(60));
        }
        assert_eq!(d, std::time::Duration::from_secs(60));
    }

    #[test]
    fn duplicate_message_id_emitted_once() {
        let idx = MessageIndex {
            message_id: 5,
            direction: 0,
            send_time: 9,
            account_b: "bob".into(),
            group: String::new(),
        };
        let msg = Message {
            message_id: 5,
            msg_type: 1,
            body: "hi".into(),
            extra: String::new(),
        };
        let mut seen = HashSet::new();
        let first = merge_offline(
            "alice",
            &[idx.clone(), idx.clone()],
            std::slice::from_ref(&msg),
            &mut seen,
        );
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].message_id, 5);
        assert_eq!(first[0].dest, "bob");
        let second = merge_offline("alice", &[idx], &[msg], &mut seen);
        assert!(second.is_empty());
    }
}
