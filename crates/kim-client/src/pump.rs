//! Dedicated reader + writer after login.
//!
//! `kim-ws` `read_frame` is not cancel-safe. A background reader owns the read
//! half for the life of the session; talks wait on oneshots keyed by sequence.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use bytes::Bytes;
use kim_core::{Conn, Error as CoreError, Frame, OpCode, DEFAULT_WRITE_WAIT};
use tokio::sync::{mpsc, oneshot, watch, Mutex, Notify};
use tokio::task::AbortHandle;
use tokio::time::{interval_at, timeout, Instant, MissedTickBehavior};
use tracing::warn;

use crate::events::Event;
use crate::link::DropReason;
use crate::wire::{decode_event, encode_ping};
use crate::ClientError;

const WRITE_CAP: usize = 32;
const EVENT_CAP: usize = 64;

pub(crate) type TokenSink = Arc<dyn Fn(String, i64) + Send + Sync>;

pub(crate) struct PumpOpts {
    pub heartbeat: Duration,
    pub read_idle: Duration,
    pub probe_timeout: Duration,
    pub token_sink: TokenSink,
}

pub(crate) enum WriteCmd {
    Frame {
        opcode: OpCode,
        payload: Bytes,
        done: oneshot::Sender<Result<(), ClientError>>,
    },
    Shutdown,
}

pub(crate) struct Live {
    writes: mpsc::Sender<WriteCmd>,
    events: Mutex<mpsc::Receiver<Event>>,
    pending: Arc<Mutex<HashMap<u32, oneshot::Sender<Event>>>>,
    ping: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    reader_abort: AbortHandle,
    writer_abort: AbortHandle,
    heartbeat_abort: AbortHandle,
    watchdog_abort: AbortHandle,
    last_read: Arc<StdMutex<Instant>>,
    death: watch::Sender<Option<DropReason>>,
    ping_now: Arc<Notify>,
    grace_until: Arc<StdMutex<Option<Instant>>>,
    probe_timeout: Duration,
}

impl Live {
    pub(crate) async fn write_frame(
        &self,
        opcode: OpCode,
        payload: Bytes,
    ) -> Result<(), ClientError> {
        let (done, rx) = oneshot::channel();
        self.writes
            .send(WriteCmd::Frame {
                opcode,
                payload,
                done,
            })
            .await
            .map_err(|_| ClientError::from(CoreError::Closed))?;
        match timeout(DEFAULT_WRITE_WAIT, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(ClientError::from(CoreError::Closed)),
            Err(_) => {
                die(&self.death, DropReason::WriteTimeout);
                Err(ClientError::other(DropReason::WriteTimeout.as_str()))
            }
        }
    }

    pub(crate) async fn write_wait<T>(
        &self,
        payload: Bytes,
        seq: u32,
        mut take: impl FnMut(&Event) -> Option<Result<T, ClientError>>,
    ) -> Result<T, ClientError> {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(seq, tx);
        if let Err(err) = self.write_frame(OpCode::Binary, payload).await {
            self.pending.lock().await.remove(&seq);
            return Err(err);
        }
        let event = rx.await.map_err(|_| ClientError::from(CoreError::Closed))?;
        take(&event).ok_or_else(|| ClientError::other("mismatched response"))?
    }

    /// Probe path only: write CODE_PING and wait for Pong.
    pub(crate) async fn ping(&self, payload: Bytes) -> Result<(), ClientError> {
        let (tx, rx) = oneshot::channel();
        *self.ping.lock().await = Some(tx);
        if let Err(err) = self.write_frame(OpCode::Binary, payload).await {
            self.ping.lock().await.take();
            return Err(err);
        }
        timeout(self.probe_timeout, rx)
            .await
            .map_err(|_| ClientError::other(DropReason::ProbeFail.as_str()))?
            .map_err(|_| ClientError::from(CoreError::Closed))?;
        Ok(())
    }

    pub(crate) async fn recv(&self) -> Result<Event, ClientError> {
        let mut rx = self.events.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| ClientError::from(CoreError::Closed))
    }

    pub(crate) fn last_read_age(&self) -> Duration {
        let t = *self.last_read.lock().unwrap_or_else(|e| e.into_inner());
        Instant::now().saturating_duration_since(t)
    }

    pub(crate) fn death_rx(&self) -> watch::Receiver<Option<DropReason>> {
        self.death.subscribe()
    }

    pub(crate) async fn wait_dead(&self) -> DropReason {
        let mut rx = self.death.subscribe();
        wait_dead(&mut rx).await
    }

    pub(crate) fn request_probe(&self) {
        let until = Instant::now() + self.probe_timeout + Duration::from_secs(1);
        *self.grace_until.lock().unwrap_or_else(|e| e.into_inner()) = Some(until);
        self.ping_now.notify_waiters();
        self.ping_now.notify_one();
    }

    pub(crate) fn shutdown(&self) {
        let _ = self.writes.try_send(WriteCmd::Shutdown);
        self.reader_abort.abort();
        self.writer_abort.abort();
        self.heartbeat_abort.abort();
        self.watchdog_abort.abort();
        die(&self.death, DropReason::Stop);
    }
}

pub(crate) async fn wait_dead(rx: &mut watch::Receiver<Option<DropReason>>) -> DropReason {
    loop {
        if let Some(reason) = *rx.borrow() {
            return reason;
        }
        if rx.changed().await.is_err() {
            return DropReason::Closed;
        }
    }
}

pub(crate) fn die(death: &watch::Sender<Option<DropReason>>, reason: DropReason) {
    let _ = death.send_if_modified(|cur| {
        if cur.is_none() {
            *cur = Some(reason);
            true
        } else {
            false
        }
    });
}

fn touch_read(last_read: &StdMutex<Instant>) {
    *last_read.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
}

pub(crate) fn start_split_pump(
    mut read: Box<dyn Conn + Send>,
    mut write: Box<dyn Conn + Send>,
    opts: PumpOpts,
) -> Arc<Live> {
    let (write_tx, mut write_rx) = mpsc::channel(WRITE_CAP);
    let (event_tx, event_rx) = mpsc::channel(EVENT_CAP);
    let pending = Arc::new(Mutex::new(HashMap::new()));
    let ping = Arc::new(Mutex::new(None));
    let last_read = Arc::new(StdMutex::new(Instant::now()));
    let (death, _) = watch::channel(None);
    let ping_now = Arc::new(Notify::new());
    let grace_until = Arc::new(StdMutex::new(None));

    let death_w = death.clone();
    let writer = tokio::spawn(async move {
        while let Some(cmd) = write_rx.recv().await {
            match cmd {
                WriteCmd::Frame {
                    opcode,
                    payload,
                    done,
                } => {
                    let result = write_one(&mut *write, opcode, payload).await;
                    let _ = done.send(result);
                }
                WriteCmd::Shutdown => {
                    let _ = write.write_frame(OpCode::Close, Bytes::new()).await;
                    let _ = write.shutdown().await;
                    break;
                }
            }
        }
        let _ = death_w;
    });

    let pending_r = pending.clone();
    let ping_r = ping.clone();
    let writes_r = write_tx.clone();
    let last_read_r = last_read.clone();
    let death_r = death.clone();
    let token_sink = opts.token_sink.clone();
    let reader = tokio::spawn(async move {
        loop {
            match read_data_pumped(&mut *read, &writes_r, &last_read_r).await {
                Ok(frame) => match decode_event(&frame) {
                    Ok(event) => {
                        let closed = matches!(event, Event::Closed);
                        if closed {
                            die(&death_r, DropReason::Closed);
                        }
                        if let Event::Kickout { .. } = &event {
                            die(&death_r, DropReason::Kickout);
                        }
                        dispatch(event, &pending_r, &ping_r, &event_tx, &token_sink, &death_r)
                            .await;
                        if closed {
                            break;
                        }
                    }
                    Err(_) => {
                        die(&death_r, DropReason::Decode);
                        dispatch(
                            Event::Closed,
                            &pending_r,
                            &ping_r,
                            &event_tx,
                            &token_sink,
                            &death_r,
                        )
                        .await;
                        break;
                    }
                },
                Err(_) => {
                    die(&death_r, DropReason::ReadError);
                    dispatch(
                        Event::Closed,
                        &pending_r,
                        &ping_r,
                        &event_tx,
                        &token_sink,
                        &death_r,
                    )
                    .await;
                    break;
                }
            }
        }
    });

    let writes_h = write_tx.clone();
    let ping_h = ping.clone();
    let death_h = death.clone();
    let ping_now_h = ping_now.clone();
    let grace_h = grace_until.clone();
    let heartbeat = opts.heartbeat;
    let probe_timeout = opts.probe_timeout;
    let mut death_hb = death.subscribe();
    let heartbeat_task = tokio::spawn(async move {
        let start = Instant::now() + heartbeat;
        let mut tick = interval_at(start, heartbeat);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    if fire_ping(&writes_h, &death_h).await.is_err() {
                        die(&death_h, DropReason::WriteTimeout);
                        break;
                    }
                }
                _ = ping_now_h.notified() => {
                    match probe_ping(&writes_h, &ping_h, probe_timeout).await {
                        Ok(()) => {
                            *grace_h.lock().unwrap_or_else(|e| e.into_inner()) = None;
                        }
                        Err(_) => {
                            die(&death_h, DropReason::ProbeFail);
                            break;
                        }
                    }
                }
                _ = wait_dead(&mut death_hb) => break,
            }
        }
    });

    let last_read_w = last_read.clone();
    let grace_w = grace_until.clone();
    let death_w = death.clone();
    let mut death_wd = death.subscribe();
    let read_idle = opts.read_idle;
    let watchdog = tokio::spawn(async move {
        let mut tick = interval_at(Instant::now() + read_idle, read_idle / 3);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    let now = Instant::now();
                    let skip = grace_w
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .is_some_and(|t| now < t);
                    if skip {
                        continue;
                    }
                    let last = *last_read_w.lock().unwrap_or_else(|e| e.into_inner());
                    if now.saturating_duration_since(last) > read_idle {
                        die(&death_w, DropReason::IdleTimeout);
                        break;
                    }
                }
                _ = wait_dead(&mut death_wd) => break,
            }
        }
    });

    Arc::new(Live {
        writes: write_tx,
        events: Mutex::new(event_rx),
        pending,
        ping,
        reader_abort: reader.abort_handle(),
        writer_abort: writer.abort_handle(),
        heartbeat_abort: heartbeat_task.abort_handle(),
        watchdog_abort: watchdog.abort_handle(),
        last_read,
        death,
        ping_now,
        grace_until,
        probe_timeout,
    })
}

async fn write_one(
    write: &mut dyn Conn,
    opcode: OpCode,
    payload: Bytes,
) -> Result<(), ClientError> {
    match timeout(DEFAULT_WRITE_WAIT, async {
        write.write_frame(opcode, payload).await?;
        write.flush().await?;
        Ok::<(), ClientError>(())
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(ClientError::other(DropReason::WriteTimeout.as_str())),
    }
}

async fn fire_ping(
    writes: &mpsc::Sender<WriteCmd>,
    death: &watch::Sender<Option<DropReason>>,
) -> Result<(), ClientError> {
    let (done, rx) = oneshot::channel();
    writes
        .send(WriteCmd::Frame {
            opcode: OpCode::Binary,
            payload: encode_ping(),
            done,
        })
        .await
        .map_err(|_| ClientError::from(CoreError::Closed))?;
    match timeout(DEFAULT_WRITE_WAIT, rx).await {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(err))) => Err(err),
        Ok(Err(_)) => Err(ClientError::from(CoreError::Closed)),
        Err(_) => {
            die(death, DropReason::WriteTimeout);
            Err(ClientError::other(DropReason::WriteTimeout.as_str()))
        }
    }
}

async fn probe_ping(
    writes: &mpsc::Sender<WriteCmd>,
    ping: &Mutex<Option<oneshot::Sender<()>>>,
    probe_timeout: Duration,
) -> Result<(), ClientError> {
    let (tx, rx) = oneshot::channel();
    *ping.lock().await = Some(tx);
    let (done, wrx) = oneshot::channel();
    if writes
        .send(WriteCmd::Frame {
            opcode: OpCode::Binary,
            payload: encode_ping(),
            done,
        })
        .await
        .is_err()
    {
        ping.lock().await.take();
        return Err(ClientError::from(CoreError::Closed));
    }
    match timeout(DEFAULT_WRITE_WAIT, wrx).await {
        Ok(Ok(Ok(()))) => {}
        _ => {
            ping.lock().await.take();
            return Err(ClientError::other(DropReason::ProbeFail.as_str()));
        }
    }
    match timeout(probe_timeout, rx).await {
        Ok(Ok(())) => Ok(()),
        _ => {
            ping.lock().await.take();
            Err(ClientError::other(DropReason::ProbeFail.as_str()))
        }
    }
}

async fn read_data_pumped(
    conn: &mut dyn Conn,
    writes: &mpsc::Sender<WriteCmd>,
    last_read: &StdMutex<Instant>,
) -> Result<Frame, ClientError> {
    loop {
        let frame = conn.read_frame().await?;
        touch_read(last_read);
        match frame.opcode {
            OpCode::Close => return Err(ClientError::from(CoreError::Closed)),
            OpCode::Ping => {
                let (done, rx) = oneshot::channel();
                if writes
                    .send(WriteCmd::Frame {
                        opcode: OpCode::Pong,
                        payload: Bytes::new(),
                        done,
                    })
                    .await
                    .is_err()
                {
                    return Err(ClientError::from(CoreError::Closed));
                }
                let _ = rx.await;
            }
            OpCode::Pong | OpCode::Continuation => {}
            OpCode::Binary | OpCode::Text => return Ok(frame),
        }
    }
}

async fn dispatch(
    event: Event,
    pending: &Mutex<HashMap<u32, oneshot::Sender<Event>>>,
    ping: &Mutex<Option<oneshot::Sender<()>>>,
    events: &mpsc::Sender<Event>,
    token_sink: &TokenSink,
    death: &watch::Sender<Option<DropReason>>,
) {
    match &event {
        Event::TalkResp(r) => {
            if let Some(tx) = pending.lock().await.remove(&r.sequence) {
                let _ = tx.send(event);
                return;
            }
        }
        Event::Status { sequence, .. }
        | Event::UserList { sequence, .. }
        | Event::Profile { sequence, .. }
        | Event::Inbox { sequence, .. }
        | Event::History { sequence, .. }
        | Event::OfflinePage { sequence, .. }
        | Event::OfflineContent { sequence, .. } => {
            if let Some(tx) = pending.lock().await.remove(sequence) {
                let _ = tx.send(event);
                return;
            }
        }
        Event::Pong => {
            if let Some(tx) = ping.lock().await.take() {
                let _ = tx.send(());
            }
            return;
        }
        Event::TokenRenew { token, exp } => {
            token_sink(token.clone(), *exp);
        }
        Event::Kickout { .. } => {
            die(death, DropReason::Kickout);
        }
        Event::Closed => {
            die(death, DropReason::Closed);
            pending.lock().await.clear();
            ping.lock().await.take();
            let _ = events.try_send(event);
            return;
        }
        _ => {}
    }
    if events.try_send(event).is_err() {
        warn!("events_dropped");
    }
}
