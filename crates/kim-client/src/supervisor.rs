//! Reconnect + backoff + sync loop around [`KimClient`].

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use tokio::sync::{broadcast, Notify};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::config::ClientConfig;
use crate::events::{Event, InboxItem, IncomingTalk};
use crate::sync::{next_backoff, ConfirmGate, SyncEngine};
use crate::token::token_unusable;
use crate::ClientError;
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
    GroupCreate {
        group_id: String,
        members: Vec<String>,
    },
    AuthFailed {
        reason: String,
    },
}

struct Inner {
    client: Arc<KimClient>,
    state: StdMutex<LinkState>,
    events: broadcast::Sender<SessionEvent>,
    confirm: ConfirmGate,
    radio: Notify,
    stop: Notify,
    stopped: AtomicBool,
    attempt: AtomicU32,
    task: StdMutex<Option<JoinHandle<()>>>,
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
                radio: Notify::new(),
                stop: Notify::new(),
                stopped: AtomicBool::new(false),
                attempt: AtomicU32::new(0),
                task: StdMutex::new(None),
            }),
        }
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
            run_loop(running).await;
        }));
    }

    pub fn stop(&self) {
        if self.inner.stopped.swap(true, Ordering::SeqCst) {
            return;
        }
        set_state(&self.inner, LinkState::Offline);
        self.inner.stop.notify_waiters();
        self.inner.stop.notify_one();
        self.inner.radio.notify_waiters();
        self.inner.radio.notify_one();
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
        self.inner.radio.notify_waiters();
        self.inner.radio.notify_one();
    }

    pub fn client(&self) -> Arc<KimClient> {
        self.inner.client.clone()
    }
}

impl Drop for SessionSupervisor {
    fn drop(&mut self) {
        self.stop();
    }
}

fn set_state(inner: &Inner, state: LinkState) {
    *inner.state.lock().unwrap_or_else(|e| e.into_inner()) = state.clone();
    let _ = inner.events.send(SessionEvent::Link(state));
}

fn stopped(inner: &Inner) -> bool {
    inner.stopped.load(Ordering::SeqCst)
}

async fn run_loop(inner: Arc<Inner>) {
    let mut delay = Duration::from_secs(1);
    loop {
        if stopped(&inner) {
            set_state(&inner, LinkState::Offline);
            return;
        }
        set_state(&inner, LinkState::Connecting);
        match run_session(&inner).await {
            SessionEnd::Stop => {
                set_state(&inner, LinkState::Offline);
                return;
            }
            SessionEnd::AuthFailed(err) => {
                warn!(error = %err, "session auth failed");
                let _ = inner.events.send(SessionEvent::AuthFailed {
                    reason: err.to_string(),
                });
                set_state(&inner, LinkState::Offline);
                return;
            }
            SessionEnd::Drop(err) => {
                warn!(error = %err, "session dropped");
                let _ = inner.events.send(SessionEvent::SyncFailed(err.to_string()));
            }
        }
        if stopped(&inner) {
            set_state(&inner, LinkState::Offline);
            return;
        }
        let attempt = inner.attempt.fetch_add(1, Ordering::SeqCst) + 1;
        set_state(&inner, LinkState::Reconnecting { attempt });
        info!(attempt, delay_ms = delay.as_millis(), "reconnect backoff");
        tokio::select! {
            _ = tokio::time::sleep(delay) => {
                delay = next_backoff(delay);
            }
            _ = inner.radio.notified() => {
                delay = Duration::from_secs(1);
            }
            _ = inner.stop.notified() => {
                set_state(&inner, LinkState::Offline);
                return;
            }
        }
    }
}

enum SessionEnd {
    Stop,
    Drop(ClientError),
    AuthFailed(ClientError),
}

async fn run_session(inner: &Inner) -> SessionEnd {
    let client = inner.client.as_ref();
    let _ = client.disconnect().await;
    if stopped(inner) {
        return SessionEnd::Stop;
    }
    if let Some(err) = token_unusable(&client.login_token()) {
        return SessionEnd::AuthFailed(err);
    }
    tokio::select! {
        result = client.connect() => {
            if let Err(err) = result {
                return SessionEnd::Drop(err);
            }
        }
        _ = inner.stop.notified() => return SessionEnd::Stop,
    }
    if stopped(inner) {
        let _ = client.disconnect().await;
        return SessionEnd::Stop;
    }
    if let Err(err) = client.login().await {
        let _ = client.disconnect().await;
        if err.is_fatal_auth() {
            return SessionEnd::AuthFailed(err);
        }
        return SessionEnd::Drop(err);
    }
    inner.attempt.store(0, Ordering::SeqCst);
    set_state(inner, LinkState::Online);
    let mut engine = SyncEngine::new();
    match engine
        .run(client, &inner.events, &inner.confirm, &inner.stop)
        .await
    {
        Ok(_) => {}
        Err(err) => {
            if stopped(inner) {
                let _ = client.disconnect().await;
                return SessionEnd::Stop;
            }
            let _ = inner.events.send(SessionEvent::SyncFailed(err.to_string()));
            let _ = client.disconnect().await;
            return SessionEnd::Drop(err);
        }
    }
    loop {
        tokio::select! {
            result = client.recv() => {
                match result {
                    Ok(Event::Closed) | Err(_) => {
                        let _ = client.disconnect().await;
                        return SessionEnd::Drop(ClientError::from(kim_core::Error::Closed));
                    }
                    Ok(ev) => dispatch_event(inner, &mut engine, ev),
                }
            }
            _ = inner.stop.notified() => {
                let _ = client.disconnect().await;
                return SessionEnd::Stop;
            }
            _ = inner.radio.notified() => {}
        }
    }
}

fn dispatch_event(inner: &Inner, engine: &mut SyncEngine, event: Event) {
    match event {
        Event::Talk(t) if engine.observe(t.message_id) => {
            let _ = inner.events.send(SessionEvent::Talk(t));
        }
        Event::Kickout { channel_id } => {
            let _ = inner.events.send(SessionEvent::Kickout { channel_id });
        }
        Event::TokenRenew { token, exp } => {
            inner.client.store_token(token.clone());
            let _ = inner.events.send(SessionEvent::TokenRenew { token, exp });
        }
        Event::FriendRequest { from, nickname } => {
            let _ = inner
                .events
                .send(SessionEvent::FriendRequest { from, nickname });
        }
        Event::FriendAccepted { from, nickname } => {
            let _ = inner
                .events
                .send(SessionEvent::FriendAccepted { from, nickname });
        }
        Event::GroupCreate { group_id, members } => {
            let _ = inner
                .events
                .send(SessionEvent::GroupCreate { group_id, members });
        }
        _ => {}
    }
}
