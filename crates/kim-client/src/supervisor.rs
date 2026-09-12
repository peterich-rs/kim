//! Reconnect + backoff + sync loop around [`KimClient`].

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::{broadcast, Notify};
use tokio::task::JoinHandle;

use crate::config::ClientConfig;
use crate::events::{InboxItem, IncomingTalk};
use crate::link::{machine, DropReason, ProbeSource};
use crate::persist::PersistHook;
use crate::sync::ConfirmGate;
use crate::KimClient;

const EVENTS_CAP: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkState {
    Connecting,
    Online,
    Reconnecting { attempt: u32 },
    Offline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    Link(LinkState),
    Inbox(Vec<InboxItem>),
    Talk(IncomingTalk),
    SyncPage {
        page_id: i64,
        talks: Vec<IncomingTalk>,
    },
    SyncProgress {
        pulled: usize,
        page_pending: bool,
    },
    SyncDone {
        pulled: usize,
    },
    SyncFailed(String),
    Kickout {
        channel_id: String,
    },
    TokenRenew {
        token: String,
        exp: i64,
    },
    FriendRequest {
        from: String,
        nickname: String,
    },
    FriendAccepted {
        from: String,
        nickname: String,
    },
    ProfileUpdated {
        profile: crate::events::Profile,
    },
    PresenceUpdated {
        account: String,
        status: i32,
        last_seen: i64,
    },
    TypingUpdated {
        typer: String,
        dest: String,
        kind: i32,
        active: bool,
    },
    ReceiptRead {
        reader: String,
        dest: String,
        kind: i32,
        message_id: i64,
    },
    GroupCreate {
        group_id: String,
        members: Vec<String>,
    },
    AuthFailed {
        reason: String,
    },
}

pub(crate) struct Inner {
    pub client: Arc<KimClient>,
    pub state: StdMutex<LinkState>,
    pub events: broadcast::Sender<SessionEvent>,
    pub confirm: ConfirmGate,
    pub hints: Notify,
    pub probe_source: AtomicU8,
    pub stop: Notify,
    pub stopped: AtomicBool,
    pub attempt: AtomicU32,
    pub last_drop_reason: StdMutex<Option<DropReason>>,
    pub task: StdMutex<Option<JoinHandle<()>>>,
    pub persist: StdMutex<Option<Arc<dyn PersistHook>>>,
    pub live_persist: StdMutex<Option<tokio::sync::mpsc::Sender<IncomingTalk>>>,
}

pub(crate) enum SessionEnd {
    Stop,
    Drop {
        err: crate::ClientError,
        reason: DropReason,
    },
    AuthFailed(crate::ClientError),
    Kicked {
        channel_id: String,
    },
}

pub struct SessionSupervisor {
    inner: Arc<Inner>,
}

impl SessionSupervisor {
    /// Build a supervisor without starting the reconnect loop.
    pub fn new(config: ClientConfig) -> Self {
        let (events, _) = broadcast::channel(EVENTS_CAP);
        Self {
            inner: Arc::new(Inner {
                client: Arc::new(KimClient::new(config)),
                state: StdMutex::new(LinkState::Connecting),
                events,
                confirm: ConfirmGate::new(),
                hints: Notify::new(),
                probe_source: AtomicU8::new(0),
                stop: Notify::new(),
                stopped: AtomicBool::new(false),
                attempt: AtomicU32::new(0),
                last_drop_reason: StdMutex::new(None),
                task: StdMutex::new(None),
                persist: StdMutex::new(None),
                live_persist: StdMutex::new(None),
            }),
        }
    }

    /// Install persist-then-ack. Live Talk is `try_send`; full queue does not ACK or stall the reader.
    pub fn with_persist(self, hook: Arc<dyn PersistHook>) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<IncomingTalk>(64);
        *self.inner.persist.lock().unwrap_or_else(|e| e.into_inner()) = Some(hook.clone());
        *self
            .inner
            .live_persist
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(tx);
        let client = self.inner.client.clone();
        tokio::spawn(async move {
            while let Some(talk) = rx.recv().await {
                let id = talk.message_id;
                if hook
                    .persist_talks(&[talk], crate::persist::UnreadPolicy::IfInserted)
                    .await
                    .is_ok()
                    && id != 0
                {
                    let _ = client.ack(id).await;
                }
            }
        });
        self
    }

    /// start = loop { connect → login → sync → recv }, reconnect with backoff.
    pub fn start(config: ClientConfig) -> Self {
        let supervisor = Self::new(config);
        supervisor.ensure_running();
        supervisor
    }

    /// Subscribe first, then start the loop so the first events are not missed.
    pub fn ensure_running(&self) {
        if self.inner.stopped.load(Ordering::SeqCst) {
            return;
        }
        let mut task = self.inner.task.lock().unwrap_or_else(|e| e.into_inner());
        if task.is_some() {
            return;
        }
        let running = self.inner.clone();
        *task = Some(tokio::spawn(async move {
            machine::run_loop(running).await;
        }));
    }

    pub fn stop(&self) {
        if self.inner.stopped.swap(true, Ordering::SeqCst) {
            return;
        }
        set_state(&self.inner, LinkState::Offline);
        self.inner.stop.notify_waiters();
        self.inner.stop.notify_one();
        self.inner.hints.notify_waiters();
        self.inner.hints.notify_one();
        if let Some(handle) = self
            .inner
            .task
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            handle.abort();
        }
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            let client = self.inner.client.clone();
            rt.spawn(async move {
                let _ = client.disconnect().await;
            });
        }
    }

    pub fn events(&self) -> broadcast::Receiver<SessionEvent> {
        self.inner.events.subscribe()
    }

    pub fn state(&self) -> LinkState {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn sync_confirm(&self, cursor: i64) {
        self.inner.confirm.confirm(cursor);
    }

    pub fn notify_radio_up(&self) {
        self.inner.attempt.store(0, Ordering::SeqCst);
        ProbeSource::store(&self.inner.probe_source, ProbeSource::Radio);
        self.inner.hints.notify_waiters();
        self.inner.hints.notify_one();
    }

    pub fn notify_foreground(&self) {
        self.inner.attempt.store(0, Ordering::SeqCst);
        ProbeSource::store(&self.inner.probe_source, ProbeSource::Foreground);
        self.inner.hints.notify_waiters();
        self.inner.hints.notify_one();
    }

    pub fn client(&self) -> Arc<KimClient> {
        self.inner.client.clone()
    }

    pub fn last_drop_reason(&self) -> Option<DropReason> {
        *self
            .inner
            .last_drop_reason
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for SessionSupervisor {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(crate) fn set_state(inner: &Inner, state: LinkState) {
    *inner.state.lock().unwrap_or_else(|e| e.into_inner()) = state.clone();
    let _ = inner.events.send(SessionEvent::Link(state));
}
