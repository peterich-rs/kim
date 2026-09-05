//! Session run loop: connect → login → serve (sync ∥ dispatch) → backoff.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::events::Event;
use crate::link::{next_backoff, DropReason, ProbeSource};
use crate::pump::Live;
use crate::supervisor::{set_state, Inner, LinkState, SessionEnd, SessionEvent};
use crate::sync::{SeenSet, SyncEngine};
use crate::token::token_unusable;
use crate::ClientError;
use kim_core::Error as CoreError;

pub(crate) async fn run_loop(inner: Arc<Inner>) {
    let mut delay = Duration::from_secs(1);
    loop {
        if inner.stopped.load(Ordering::SeqCst) {
            set_state(&inner, LinkState::Offline);
            return;
        }
        set_state(&inner, LinkState::Connecting);
        match run_session(&inner, &mut delay).await {
            SessionEnd::Stop => {
                record_drop(&inner, DropReason::Stop, 0);
                set_state(&inner, LinkState::Offline);
                return;
            }
            SessionEnd::AuthFailed(err) => {
                warn!(error = %err, "session auth failed");
                let _ = inner.events.send(SessionEvent::AuthFailed {
                    reason: err.to_string(),
                });
                record_drop(&inner, DropReason::AuthFailed, 0);
                set_state(&inner, LinkState::Offline);
                return;
            }
            SessionEnd::Kicked { channel_id } => {
                let _ = inner.events.send(SessionEvent::Kickout { channel_id });
                record_drop(&inner, DropReason::Kickout, 0);
                set_state(&inner, LinkState::Offline);
                return;
            }
            SessionEnd::Drop { err, reason } => {
                warn!(error = %err, reason = reason.as_str(), "session dropped");
                record_drop(&inner, reason, 0);
            }
        }
        if inner.stopped.load(Ordering::SeqCst) {
            set_state(&inner, LinkState::Offline);
            return;
        }
        let attempt = inner.attempt.fetch_add(1, Ordering::SeqCst) + 1;
        let last_reason = inner
            .last_drop_reason
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unwrap_or(DropReason::Closed);
        if last_reason == DropReason::ProbeFail {
            delay = Duration::from_secs(1);
        }
        record_drop(&inner, last_reason, attempt);
        set_state(&inner, LinkState::Reconnecting { attempt });
        info!(
            attempt,
            delay_ms = delay.as_millis(),
            reason = last_reason.as_str(),
            "reconnect backoff"
        );
        tokio::select! {
            _ = tokio::time::sleep(delay) => {
                delay = next_backoff(delay);
            }
            _ = inner.hints.notified() => {
                delay = Duration::from_secs(1);
            }
            _ = inner.stop.notified() => {
                set_state(&inner, LinkState::Offline);
                return;
            }
        }
    }
}

fn record_drop(inner: &Inner, reason: DropReason, _attempt: u32) {
    *inner
        .last_drop_reason
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(reason);
}

async fn run_session(inner: &Inner, delay: &mut Duration) -> SessionEnd {
    let client = inner.client.as_ref();
    let _ = client.disconnect().await;
    if inner.stopped.load(Ordering::SeqCst) {
        return SessionEnd::Stop;
    }
    if let Some(err) = token_unusable(&client.login_token()) {
        return SessionEnd::AuthFailed(err);
    }
    tokio::select! {
        result = client.connect() => {
            if let Err(err) = result {
                let reason = if matches!(err, ClientError::HandshakeTimeout(_)) {
                    DropReason::HandshakeTimeout
                } else {
                    DropReason::ConnectFail
                };
                record_drop(inner, reason, 0);
                return SessionEnd::Drop { err, reason };
            }
        }
        _ = inner.stop.notified() => return SessionEnd::Stop,
    }
    if inner.stopped.load(Ordering::SeqCst) {
        let _ = client.disconnect().await;
        return SessionEnd::Stop;
    }
    if let Err(err) = client.login().await {
        let _ = client.disconnect().await;
        if err.is_fatal_auth() {
            return SessionEnd::AuthFailed(err);
        }
        let reason = if matches!(err, ClientError::HandshakeTimeout(_)) {
            DropReason::HandshakeTimeout
        } else {
            DropReason::ConnectFail
        };
        record_drop(inner, reason, 0);
        return SessionEnd::Drop { err, reason };
    }
    inner.attempt.store(0, Ordering::SeqCst);
    *delay = Duration::from_secs(1);
    set_state(inner, LinkState::Online);

    let Some(live) = client.live().await else {
        record_drop(inner, DropReason::Closed, 0);
        return SessionEnd::Drop {
            err: ClientError::from(CoreError::Closed),
            reason: DropReason::Closed,
        };
    };

    serve(inner, &live).await
}

async fn serve(inner: &Inner, live: &Live) -> SessionEnd {
    let client = inner.client.as_ref();
    let engine = SyncEngine::new();
    let seen = engine.seen();
    let confirm_timeout = client.config().confirm_timeout;

    let dispatch = dispatch_loop(inner, live, seen);
    let mut death = live.death_rx();
    let sync = engine.run(
        client,
        &inner.events,
        &inner.confirm,
        &inner.stop,
        &mut death,
        confirm_timeout,
    );

    tokio::select! {
        end = dispatch => end,
        result = sync => {
            match result {
                Ok(_) => dispatch_loop(inner, live, engine.seen()).await,
                Err(err) => {
                    if inner.stopped.load(Ordering::SeqCst) {
                        let _ = client.disconnect().await;
                        return SessionEnd::Stop;
                    }
                    let death_reason = {
                        let death = live.death_rx();
                        let copied = *death.borrow();
                        copied
                    };
                    if let Some(reason) = death_reason {
                        if reason == DropReason::Kickout {
                            return SessionEnd::Kicked {
                                channel_id: String::new(),
                            };
                        }
                        if reason.is_fatal() {
                            return match reason {
                                DropReason::Stop => SessionEnd::Stop,
                                DropReason::AuthFailed => SessionEnd::AuthFailed(err),
                                _ => SessionEnd::Kicked {
                                    channel_id: String::new(),
                                },
                            };
                        }
                        record_drop(inner, reason, 0);
                        let _ = client.disconnect().await;
                        return SessionEnd::Drop { err, reason };
                    }
                    let reason = if err.to_string().contains("confirm-timeout") {
                        DropReason::ConfirmTimeout
                    } else {
                        DropReason::SyncFailed
                    };
                    if reason == DropReason::SyncFailed {
                        let _ = inner.events.send(SessionEvent::SyncFailed(err.to_string()));
                    }
                    record_drop(inner, reason, 0);
                    let _ = client.disconnect().await;
                    SessionEnd::Drop { err, reason }
                }
            }
        }
    }
}

async fn dispatch_loop(
    inner: &Inner,
    live: &Live,
    seen: Arc<std::sync::Mutex<SeenSet>>,
) -> SessionEnd {
    let client = inner.client.as_ref();
    loop {
        tokio::select! {
            result = client.recv() => {
                match result {
                    Ok(Event::Closed) | Err(_) => {
                        let reason = live
                            .death_rx()
                            .borrow()
                            .unwrap_or(DropReason::Closed);
                        let _ = client.disconnect().await;
                        record_drop(inner, reason, 0);
                        return SessionEnd::Drop {
                            err: ClientError::from(CoreError::Closed),
                            reason,
                        };
                    }
                    Ok(Event::Kickout { channel_id }) => {
                        let _ = inner.events.send(SessionEvent::Kickout {
                            channel_id: channel_id.clone(),
                        });
                        let _ = client.disconnect().await;
                        return SessionEnd::Kicked { channel_id };
                    }
                    Ok(ev) => dispatch_event(inner, &seen, ev),
                }
            }
            reason = live.wait_dead() => {
                info!(
                    reason = reason.as_str(),
                    last_frame_age_ms = live.last_read_age().as_millis(),
                    "link death"
                );
                if reason == DropReason::Kickout {
                    let _ = inner.events.send(SessionEvent::Kickout {
                        channel_id: String::new(),
                    });
                    let _ = client.disconnect().await;
                    return SessionEnd::Kicked {
                        channel_id: String::new(),
                    };
                }
                if reason == DropReason::Stop || inner.stopped.load(Ordering::SeqCst) {
                    let _ = client.disconnect().await;
                    return SessionEnd::Stop;
                }
                if reason.is_fatal() {
                    let _ = client.disconnect().await;
                    return SessionEnd::AuthFailed(ClientError::other(reason.as_str()));
                }
                record_drop(inner, reason, 0);
                let _ = client.disconnect().await;
                return SessionEnd::Drop {
                    err: ClientError::other(reason.as_str()),
                    reason,
                };
            }
            _ = inner.stop.notified() => {
                let _ = client.disconnect().await;
                return SessionEnd::Stop;
            }
            _ = inner.hints.notified() => {
                let src = ProbeSource::from_u8(inner.probe_source.load(Ordering::SeqCst));
                tracing::debug!(source = ?src, "link probe");
                live.request_probe();
            }
        }
    }
}

fn dispatch_event(inner: &Inner, seen: &Arc<std::sync::Mutex<SeenSet>>, event: Event) {
    match event {
        Event::Talk(t) => {
            let emit = seen
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .observe(t.message_id);
            if emit {
                let _ = inner.events.send(SessionEvent::Talk(t));
            }
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
