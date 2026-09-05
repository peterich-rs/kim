//! Offline catch-up: inbox.list then offline.index/content pages, persist-then-ack.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use tokio::sync::{broadcast, watch, Notify};
use tracing::{debug, warn};

use crate::events::{IncomingTalk, Message, MessageIndex};
use crate::link::DropReason;
use crate::pump::wait_dead;
use crate::supervisor::SessionEvent;
use crate::ClientError;
use crate::KimClient;
use kim_protocol::{CMD_CHAT_GROUP_TALK, CMD_CHAT_USER_TALK};

pub(crate) const SYNC_PAGE: usize = 200;
pub(crate) const INBOX_LIMIT: i32 = 200;
const SEEN_CAP: usize = 4096;

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
    death: &mut watch::Receiver<Option<DropReason>>,
    confirm_timeout: Duration,
    retry: Option<&broadcast::Sender<SessionEvent>>,
    retry_page: Option<(i64, Vec<IncomingTalk>)>,
) -> Result<(), ClientError> {
    if needed <= 0 {
        return Ok(());
    }
    if let Some(reason) = *death.borrow() {
        return Err(ClientError::other(reason.as_str()));
    }
    let first = wait_confirm_once(rx, needed, stop, death, confirm_timeout).await;
    if first.is_ok() {
        return first;
    }
    if first
        .as_ref()
        .err()
        .is_some_and(|e| e.to_string() != "confirm-timeout")
    {
        return first;
    }
    if let (Some(events), Some((page_id, talks))) = (retry, retry_page) {
        let _ = events.send(SessionEvent::SyncPage { page_id, talks });
        wait_confirm_once(rx, needed, stop, death, confirm_timeout).await?;
        return Ok(());
    }
    Err(ClientError::other(DropReason::ConfirmTimeout.as_str()))
}

async fn wait_confirm_once(
    rx: &mut watch::Receiver<i64>,
    needed: i64,
    stop: &Notify,
    death: &mut watch::Receiver<Option<DropReason>>,
    confirm_timeout: Duration,
) -> Result<(), ClientError> {
    tokio::select! {
        result = rx.wait_for(|v| *v >= needed) => {
            result.map(|_| ()).map_err(|_| ClientError::other("confirm closed"))
        }
        _ = stop.notified() => Err(ClientError::other("stopped")),
        reason = wait_dead(death) => Err(ClientError::other(reason.as_str())),
        _ = tokio::time::sleep(confirm_timeout) => {
            Err(ClientError::other("confirm-timeout"))
        }
    }
}

pub(crate) struct SeenSet {
    seen: HashSet<i64>,
    seen_order: VecDeque<i64>,
}

impl SeenSet {
    pub(crate) fn new() -> Self {
        Self {
            seen: HashSet::new(),
            seen_order: VecDeque::new(),
        }
    }

    pub(crate) fn observe(&mut self, message_id: i64) -> bool {
        if message_id == 0 {
            return true;
        }
        if !self.seen.insert(message_id) {
            return false;
        }
        self.seen_order.push_back(message_id);
        while self.seen_order.len() > SEEN_CAP {
            if let Some(old) = self.seen_order.pop_front() {
                self.seen.remove(&old);
            }
        }
        true
    }
}

pub(crate) struct SyncEngine {
    seen: Arc<StdMutex<SeenSet>>,
}

impl SyncEngine {
    pub(crate) fn new() -> Self {
        Self {
            seen: Arc::new(StdMutex::new(SeenSet::new())),
        }
    }

    pub(crate) fn seen(&self) -> Arc<StdMutex<SeenSet>> {
        self.seen.clone()
    }

    pub(crate) async fn run(
        &self,
        client: &KimClient,
        events: &broadcast::Sender<SessionEvent>,
        confirm: &ConfirmGate,
        stop: &Notify,
        death: &mut watch::Receiver<Option<DropReason>>,
        confirm_timeout: Duration,
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
            let talks = {
                let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
                merge_offline(&account, &indexes, &msgs, |id| seen.observe(id))
            };
            let new_count = talks.len();
            pulled += new_count;
            let max_id = ids.iter().copied().max().unwrap_or(0);
            if new_count > 0 && max_id > 0 {
                if events
                    .send(SessionEvent::SyncPage {
                        page_id: max_id,
                        talks: talks.clone(),
                    })
                    .is_err()
                {
                    warn!(page_id = max_id, "offline page not delivered");
                    return Err(ClientError::other("offline page not delivered"));
                }
                wait_confirm(
                    &mut confirm_rx,
                    max_id,
                    stop,
                    death,
                    confirm_timeout,
                    Some(events),
                    Some((max_id, talks)),
                )
                .await?;
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
    mut unseen: impl FnMut(i64) -> bool,
) -> Vec<IncomingTalk> {
    let by_id: HashMap<i64, &Message> = msgs.iter().map(|m| (m.message_id, m)).collect();
    let mut talks = Vec::new();
    for idx in indexes.iter().take(SYNC_PAGE) {
        let Some(msg) = by_id.get(&idx.message_id) else {
            continue;
        };
        if idx.message_id != 0 && !unseen(idx.message_id) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut seen = SeenSet::new();
        let first = merge_offline(
            "alice",
            &[idx.clone(), idx.clone()],
            std::slice::from_ref(&msg),
            |id| seen.observe(id),
        );
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].message_id, 5);
        assert_eq!(first[0].dest, "bob");
        let second = merge_offline("alice", &[idx], &[msg], |id| seen.observe(id));
        assert!(second.is_empty());
    }
}
