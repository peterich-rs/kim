use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tracing::{debug, warn};

use crate::write_loop::WriteShared;
use crate::{ChannelHandle, Conn, Error, MessageListener, OpCode};

/// Full write mailbox: block (internal TcpClient/Chat uplink) or fail + disconnect (gateway downlink).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WriteFullPolicy {
    #[default]
    Block,
    Disconnect,
}

/// Optional hook when a Disconnect mailbox `try_send` fails.
pub type MailboxFullHook = Arc<dyn Fn() + Send + Sync>;
/// Extract a serial-lane key from a binary payload (`header.channel_id`). Empty/None uses the connection id.
pub type LaneKeyFn = Arc<dyn Fn(&[u8]) -> Option<Arc<str>> + Send + Sync>;

/// 连接的上层包装。Server 管理的是 Channel，不是裸 Conn。
///
/// Push 把字节丢进队列就返回；真正的 write 在独立写协程里。
#[derive(Clone)]
pub struct Channel {
    id: Arc<str>,
    write: WriteShared,
}

#[derive(Clone)]
pub struct ChannelOpts {
    pub read_wait: Duration,
    pub write_wait: Duration,
    /// 写队列长度。Disconnect 时满了 Push 失败并拆慢连接。
    pub write_queue: usize,
    pub write_full: WriteFullPolicy,
    /// Per-lane queue depth and default process-wide in-flight budget.
    pub max_in_flight: usize,
    /// Shared process semaphore. None → one semaphore per connection.
    pub in_flight: Option<Arc<Semaphore>>,
    pub lane_key: Option<LaneKeyFn>,
    pub on_mailbox_full: Option<MailboxFullHook>,
    /// Worker exits after this idle timeout; the next frame rebuilds the lane.
    pub lane_idle: Duration,
}

impl Default for ChannelOpts {
    fn default() -> Self {
        Self {
            read_wait: crate::DEFAULT_READ_WAIT,
            write_wait: crate::DEFAULT_WRITE_WAIT,
            write_queue: crate::DEFAULT_WRITE_QUEUE,
            write_full: WriteFullPolicy::Block,
            max_in_flight: crate::DEFAULT_MAX_IN_FLIGHT,
            in_flight: None,
            lane_key: None,
            on_mailbox_full: None,
            lane_idle: crate::DEFAULT_LANE_IDLE,
        }
    }
}

impl Channel {
    /// 握手完成后：读半边给读循环，写半边给写协程。
    pub fn pair<R, W>(
        id: impl Into<Arc<str>>,
        reader: R,
        writer: W,
        opts: ChannelOpts,
    ) -> (Self, ChannelReadLoop<R>)
    where
        R: Conn + 'static,
        W: Conn + 'static,
    {
        let id = id.into();
        let write = WriteShared::spawn(
            writer,
            opts.write_queue,
            opts.write_wait,
            opts.write_full,
            opts.on_mailbox_full.clone(),
        );

        let max_in_flight = opts.max_in_flight.max(1);
        let in_flight = opts
            .in_flight
            .unwrap_or_else(|| Arc::new(Semaphore::new(max_in_flight)));
        let channel = Self {
            id: id.clone(),
            write: write.clone(),
        };
        let read_loop = ChannelReadLoop {
            id,
            reader,
            write,
            read_wait: opts.read_wait,
            max_in_flight,
            in_flight,
            lane_key: opts.lane_key,
            lane_idle: opts.lane_idle,
        };
        (channel, read_loop)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Cheap clone of the connection id. `ChannelMap` keys off this so insert
    /// does not allocate a second `String`.
    pub fn id_arc(&self) -> Arc<str> {
        self.id.clone()
    }

    pub fn is_closed(&self) -> bool {
        self.write.is_closed()
    }

    pub async fn close(&self) {
        self.write.close().await;
    }
}

#[async_trait]
impl ChannelHandle for Channel {
    fn id(&self) -> &str {
        &self.id
    }

    async fn push(&self, payload: Bytes) -> Result<(), Error> {
        self.write.push_frame(OpCode::Binary, payload).await
    }
}

/// 占着读半边、阻塞直到连接断开。只应被一个任务运行。
pub struct ChannelReadLoop<R> {
    id: Arc<str>,
    reader: R,
    write: WriteShared,
    read_wait: Duration,
    max_in_flight: usize,
    in_flight: Arc<Semaphore>,
    lane_key: Option<LaneKeyFn>,
    lane_idle: Duration,
}

struct LaneJob {
    payload: Bytes,
    _permit: OwnedSemaphorePermit,
}

struct LaneSet {
    txs: Arc<Mutex<HashMap<Arc<str>, mpsc::Sender<LaneJob>>>>,
    live: Arc<AtomicUsize>,
    in_flight: Arc<Semaphore>,
    lane_cap: usize,
    lane_idle: Duration,
    listener: Arc<dyn MessageListener>,
    handle: PushHandle,
}

impl LaneSet {
    fn new(
        in_flight: Arc<Semaphore>,
        lane_cap: usize,
        lane_idle: Duration,
        listener: Arc<dyn MessageListener>,
        handle: PushHandle,
    ) -> Self {
        Self {
            txs: Arc::new(Mutex::new(HashMap::new())),
            live: Arc::new(AtomicUsize::new(0)),
            in_flight,
            lane_cap,
            lane_idle,
            listener,
            handle,
        }
    }

    #[cfg(test)]
    fn lane_len(&self) -> usize {
        self.txs.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    fn spawn_lane(
        &self,
        map: &mut HashMap<Arc<str>, mpsc::Sender<LaneJob>>,
        key: Arc<str>,
    ) -> mpsc::Sender<LaneJob> {
        let (tx, rx) = mpsc::channel(self.lane_cap);
        map.insert(key.clone(), tx.clone());
        let listener = self.listener.clone();
        let handle = self.handle.clone();
        let txs = self.txs.clone();
        let live = self.live.clone();
        let idle = self.lane_idle;
        // Weak identity only: a strong `mine` sender would keep rx open after
        // drain() clears `txs`, delaying disconnect until lane_idle / drain wait.
        let mine = tx.downgrade();
        live.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(async move {
            let mut rx = rx;
            loop {
                match tokio::time::timeout(idle, rx.recv()).await {
                    Ok(Some(job)) => {
                        if let Err(err) = listener.receive(&handle, job.payload).await {
                            warn!(%err, "message listener failed");
                        }
                    }
                    Ok(None) => break,
                    Err(_) => {
                        let mut map = txs.lock().unwrap_or_else(|e| e.into_inner());
                        if let Ok(job) = rx.try_recv() {
                            drop(map);
                            if let Err(err) = listener.receive(&handle, job.payload).await {
                                warn!(%err, "message listener failed");
                            }
                            continue;
                        }
                        if map
                            .get(&key)
                            .is_some_and(|s| mine.upgrade().is_some_and(|u| u.same_channel(s)))
                        {
                            map.remove(&key);
                        }
                        break;
                    }
                }
            }
            live.fetch_sub(1, Ordering::SeqCst);
        });
        tx
    }

    async fn enqueue(&self, key: Arc<str>, payload: Bytes) -> Result<(), Error> {
        let permit = self
            .in_flight
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| Error::Closed)?;
        enum Admit {
            Sent,
            Retry(LaneJob),
            Wait(mpsc::Sender<LaneJob>, LaneJob),
        }
        let mut job = LaneJob {
            payload,
            _permit: permit,
        };
        loop {
            // try_send while holding the map lock so an idle worker cannot
            // retire a lane whose sender was already cloned for this enqueue.
            let admit = {
                let mut map = self.txs.lock().unwrap_or_else(|e| e.into_inner());
                let tx = match map.get(&key) {
                    Some(tx) => tx.clone(),
                    None => self.spawn_lane(&mut map, key.clone()),
                };
                match tx.try_send(job) {
                    Ok(()) => Admit::Sent,
                    Err(TrySendError::Full(j)) => Admit::Wait(tx, j),
                    Err(TrySendError::Closed(j)) => {
                        if map.get(&key).is_some_and(|s| s.same_channel(&tx)) {
                            map.remove(&key);
                        }
                        Admit::Retry(j)
                    }
                }
            };
            match admit {
                Admit::Sent => return Ok(()),
                Admit::Retry(j) => job = j,
                Admit::Wait(tx, j) => match tx.send(j).await {
                    Ok(()) => return Ok(()),
                    Err(returned) => job = returned.0,
                },
            }
        }
    }

    async fn drain(self) {
        self.txs.lock().unwrap_or_else(|e| e.into_inner()).clear();
        let deadline = Instant::now() + crate::DEFAULT_DRAIN_WAIT;
        while self.live.load(Ordering::SeqCst) > 0 {
            if Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

impl<R: Conn> ChannelReadLoop<R> {
    pub async fn run(mut self, listener: Arc<dyn MessageListener>) -> Result<(), Error> {
        let handle = PushHandle {
            id: self.id.clone(),
            write: self.write.clone(),
        };
        let lanes = LaneSet::new(
            self.in_flight.clone(),
            self.max_in_flight,
            self.lane_idle,
            listener,
            handle,
        );
        let result = self.read_until_err(&lanes).await;
        lanes.drain().await;
        self.write.close().await;
        result
    }

    fn lane_id(&self, payload: &[u8]) -> Arc<str> {
        if let Some(f) = &self.lane_key {
            if let Some(k) = f(payload) {
                if !k.is_empty() {
                    return k;
                }
            }
        }
        self.id.clone()
    }

    async fn read_until_err(&mut self, lanes: &LaneSet) -> Result<(), Error> {
        loop {
            if self.write.is_closed() {
                return Err(Error::Closed);
            }
            // Timeout around the whole read_frame. kim-ws read_frame is not cancel-safe:
            // on timeout we drop the connection and never continue a half frame.
            let frame = match tokio::time::timeout(self.read_wait, self.reader.read_frame()).await {
                Ok(Ok(frame)) => frame,
                Ok(Err(err)) => return Err(err),
                Err(_) => return Err(Error::Closed),
            };

            match frame.opcode {
                OpCode::Close => return Err(Error::Closed),
                OpCode::Ping => {
                    debug!(channel = %self.id, "recv ping, reply pong");
                    self.write.push_frame(OpCode::Pong, Bytes::new()).await?;
                }
                OpCode::Pong => {
                    debug!(channel = %self.id, "recv pong");
                }
                OpCode::Binary | OpCode::Text => {
                    if frame.payload.is_empty() {
                        continue;
                    }
                    let key = self.lane_id(&frame.payload);
                    lanes.enqueue(key, frame.payload).await?;
                }
                OpCode::Continuation => {}
            }
        }
    }
}

/// 读循环交给 MessageListener 的手柄：能回消息，不能关连接。
/// receive 不在读任务上跑；同一 lane（channel_id）仍 FIFO。
#[derive(Clone)]
struct PushHandle {
    id: Arc<str>,
    write: WriteShared,
}

#[async_trait]
impl ChannelHandle for PushHandle {
    fn id(&self) -> &str {
        &self.id
    }

    async fn push(&self, payload: Bytes) -> Result<(), Error> {
        self.write.push_frame(OpCode::Binary, payload).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, Frame};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;
    use tokio::sync::{mpsc, Notify, Semaphore};
    use tokio::time::{timeout, Duration};

    struct NullConn;

    #[async_trait]
    impl Conn for NullConn {
        async fn read_frame(&mut self) -> Result<Frame, Error> {
            std::future::pending().await
        }
        async fn write_frame(&mut self, _opcode: OpCode, _payload: Bytes) -> Result<(), Error> {
            Ok(())
        }
        async fn flush(&mut self) -> Result<(), Error> {
            Ok(())
        }
        async fn shutdown(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }

    struct RecConn {
        tx: mpsc::UnboundedSender<&'static str>,
    }

    #[async_trait]
    impl Conn for RecConn {
        async fn read_frame(&mut self) -> Result<Frame, Error> {
            std::future::pending().await
        }
        async fn write_frame(&mut self, opcode: OpCode, _payload: Bytes) -> Result<(), Error> {
            if opcode == OpCode::Binary {
                let _ = self.tx.send("binary");
            }
            Ok(())
        }
        async fn flush(&mut self) -> Result<(), Error> {
            let _ = self.tx.send("flush");
            Ok(())
        }
        async fn shutdown(&mut self) -> Result<(), Error> {
            let _ = self.tx.send("close");
            Ok(())
        }
    }

    struct CountFlush {
        frames: Arc<AtomicUsize>,
        flushes: Arc<AtomicUsize>,
        seen: mpsc::UnboundedSender<&'static str>,
    }

    #[async_trait]
    impl Conn for CountFlush {
        async fn read_frame(&mut self) -> Result<Frame, Error> {
            std::future::pending().await
        }
        async fn write_frame(&mut self, _opcode: OpCode, _payload: Bytes) -> Result<(), Error> {
            self.frames.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn flush(&mut self) -> Result<(), Error> {
            self.flushes.fetch_add(1, Ordering::SeqCst);
            let _ = self.seen.send("flush");
            Ok(())
        }
        async fn shutdown(&mut self) -> Result<(), Error> {
            let _ = self.seen.send("close");
            Ok(())
        }
    }

    struct ScriptedRead {
        frames: StdMutex<std::collections::VecDeque<Frame>>,
    }

    #[async_trait]
    impl Conn for ScriptedRead {
        async fn read_frame(&mut self) -> Result<Frame, Error> {
            let next = self
                .frames
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop_front();
            match next {
                Some(f) => {
                    tokio::task::yield_now().await;
                    Ok(f)
                }
                None => std::future::pending().await,
            }
        }
        async fn write_frame(&mut self, _opcode: OpCode, _payload: Bytes) -> Result<(), Error> {
            Ok(())
        }
        async fn flush(&mut self) -> Result<(), Error> {
            Ok(())
        }
        async fn shutdown(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }

    struct OpcodeRec {
        tx: mpsc::UnboundedSender<OpCode>,
    }

    #[async_trait]
    impl Conn for OpcodeRec {
        async fn read_frame(&mut self) -> Result<Frame, Error> {
            std::future::pending().await
        }
        async fn write_frame(&mut self, opcode: OpCode, _payload: Bytes) -> Result<(), Error> {
            let _ = self.tx.send(opcode);
            Ok(())
        }
        async fn flush(&mut self) -> Result<(), Error> {
            Ok(())
        }
        async fn shutdown(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }

    struct HangWrite {
        entered: Arc<Notify>,
    }

    #[async_trait]
    impl Conn for HangWrite {
        async fn read_frame(&mut self) -> Result<Frame, Error> {
            std::future::pending().await
        }
        async fn write_frame(&mut self, _opcode: OpCode, _payload: Bytes) -> Result<(), Error> {
            self.entered.notify_waiters();
            std::future::pending().await
        }
        async fn flush(&mut self) -> Result<(), Error> {
            Ok(())
        }
        async fn shutdown(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }

    fn payload_lane_key() -> LaneKeyFn {
        Arc::new(|p: &[u8]| Some(Arc::from(String::from_utf8_lossy(p).as_ref())))
    }

    #[tokio::test]
    async fn push_then_close_emits_binary_then_close() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (ch, _read_loop) = Channel::pair(
            "c1",
            NullConn,
            RecConn { tx },
            ChannelOpts {
                write_wait: Duration::from_secs(2),
                ..ChannelOpts::default()
            },
        );
        ch.push(Bytes::from_static(b"hi")).await.unwrap();
        ch.close().await;
        let mut got = Vec::new();
        for _ in 0..3 {
            got.push(
                timeout(Duration::from_secs(2), rx.recv())
                    .await
                    .unwrap()
                    .unwrap(),
            );
        }
        assert_eq!(got, ["binary", "flush", "close"]);
    }

    #[tokio::test]
    async fn n_frames_then_close_flushes_once() {
        let frames = Arc::new(AtomicUsize::new(0));
        let flushes = Arc::new(AtomicUsize::new(0));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (ch, _read_loop) = Channel::pair(
            "c1",
            NullConn,
            CountFlush {
                frames: frames.clone(),
                flushes: flushes.clone(),
                seen: tx,
            },
            ChannelOpts {
                write_wait: Duration::from_secs(2),
                ..ChannelOpts::default()
            },
        );
        for _ in 0..4 {
            ch.push(Bytes::from_static(b"x")).await.unwrap();
        }
        ch.close().await;
        let mut saw_flush = false;
        let mut saw_close = false;
        for _ in 0..8 {
            match timeout(Duration::from_secs(2), rx.recv()).await {
                Ok(Some("flush")) => saw_flush = true,
                Ok(Some("close")) => {
                    saw_close = true;
                    break;
                }
                _ => break,
            }
        }
        assert!(saw_flush);
        assert!(saw_close);
        assert_eq!(frames.load(Ordering::SeqCst), 4);
        assert!(flushes.load(Ordering::SeqCst) >= 1);
        assert_ne!(flushes.load(Ordering::SeqCst), 0);
    }

    struct FifoListener {
        order: Arc<StdMutex<Vec<&'static str>>>,
        join_gate: Arc<Notify>,
        join_started: Arc<Notify>,
        talk_started: Arc<AtomicBool>,
    }

    #[async_trait]
    impl MessageListener for FifoListener {
        async fn receive(&self, _handle: &dyn ChannelHandle, payload: Bytes) -> Result<(), Error> {
            match payload.as_ref() {
                b"join" => {
                    self.order
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push("join_start");
                    self.join_started.notify_waiters();
                    self.join_gate.notified().await;
                    self.order
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push("join_end");
                }
                b"talk" => {
                    self.talk_started.store(true, Ordering::SeqCst);
                    self.order
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push("talk");
                }
                _ => {}
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn same_lane_join_blocks_talk() {
        let order = Arc::new(StdMutex::new(Vec::new()));
        let join_gate = Arc::new(Notify::new());
        let join_started = Arc::new(Notify::new());
        let talk_started = Arc::new(AtomicBool::new(false));
        let listener = Arc::new(FifoListener {
            order: order.clone(),
            join_gate: join_gate.clone(),
            join_started: join_started.clone(),
            talk_started: talk_started.clone(),
        });
        let frames = std::collections::VecDeque::from([
            Frame::binary(Bytes::from_static(b"join")),
            Frame::binary(Bytes::from_static(b"talk")),
        ]);
        let (_ch, read_loop) = Channel::pair(
            "user-1",
            ScriptedRead {
                frames: StdMutex::new(frames),
            },
            NullConn,
            ChannelOpts {
                read_wait: Duration::from_secs(5),
                ..ChannelOpts::default()
            },
        );
        tokio::spawn(async move {
            let _ = read_loop.run(listener).await;
        });
        timeout(Duration::from_secs(2), join_started.notified())
            .await
            .expect("join should start");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !talk_started.load(Ordering::SeqCst),
            "talk must not run while join is in the same lane"
        );
        join_gate.notify_waiters();
        timeout(Duration::from_secs(2), async {
            loop {
                if talk_started.load(Ordering::SeqCst) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("talk should run after join");
        assert_eq!(
            *order.lock().unwrap_or_else(|e| e.into_inner()),
            ["join_start", "join_end", "talk"]
        );
    }

    struct OverlapListener {
        a_started: Arc<Notify>,
        b_started: Arc<Notify>,
        a_running: Arc<AtomicBool>,
        b_running: Arc<AtomicBool>,
        saw_overlap: Arc<AtomicBool>,
        gate: Arc<Notify>,
    }

    #[async_trait]
    impl MessageListener for OverlapListener {
        async fn receive(&self, _handle: &dyn ChannelHandle, payload: Bytes) -> Result<(), Error> {
            match payload.as_ref() {
                b"ch-a" => {
                    self.a_running.store(true, Ordering::SeqCst);
                    if self.b_running.load(Ordering::SeqCst) {
                        self.saw_overlap.store(true, Ordering::SeqCst);
                    }
                    self.a_started.notify_waiters();
                    self.gate.notified().await;
                    self.a_running.store(false, Ordering::SeqCst);
                }
                b"ch-b" => {
                    self.b_running.store(true, Ordering::SeqCst);
                    if self.a_running.load(Ordering::SeqCst) {
                        self.saw_overlap.store(true, Ordering::SeqCst);
                    }
                    self.b_started.notify_waiters();
                    self.gate.notified().await;
                    self.b_running.store(false, Ordering::SeqCst);
                }
                _ => {}
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn different_lanes_can_overlap() {
        let a_started = Arc::new(Notify::new());
        let b_started = Arc::new(Notify::new());
        let gate = Arc::new(Notify::new());
        let saw_overlap = Arc::new(AtomicBool::new(false));
        let listener = Arc::new(OverlapListener {
            a_started: a_started.clone(),
            b_started: b_started.clone(),
            a_running: Arc::new(AtomicBool::new(false)),
            b_running: Arc::new(AtomicBool::new(false)),
            saw_overlap: saw_overlap.clone(),
            gate: gate.clone(),
        });
        let frames = std::collections::VecDeque::from([
            Frame::binary(Bytes::from_static(b"ch-a")),
            Frame::binary(Bytes::from_static(b"ch-b")),
        ]);
        let (_ch, read_loop) = Channel::pair(
            "gw-1",
            ScriptedRead {
                frames: StdMutex::new(frames),
            },
            NullConn,
            ChannelOpts {
                read_wait: Duration::from_secs(5),
                lane_key: Some(payload_lane_key()),
                ..ChannelOpts::default()
            },
        );
        tokio::spawn(async move {
            let _ = read_loop.run(listener).await;
        });
        timeout(Duration::from_secs(2), async {
            a_started.notified().await;
            b_started.notified().await;
        })
        .await
        .expect("both lanes should start");
        assert!(
            saw_overlap.load(Ordering::SeqCst),
            "different channel_ids may run concurrent"
        );
        gate.notify_waiters();
    }

    struct SlowListener {
        started: Arc<Notify>,
        gate: Arc<Notify>,
    }

    #[async_trait]
    impl MessageListener for SlowListener {
        async fn receive(&self, _handle: &dyn ChannelHandle, _payload: Bytes) -> Result<(), Error> {
            self.started.notify_waiters();
            self.gate.notified().await;
            Ok(())
        }
    }

    #[tokio::test]
    async fn ping_answered_during_slow_handler() {
        let started = Arc::new(Notify::new());
        let gate = Arc::new(Notify::new());
        let listener = Arc::new(SlowListener {
            started: started.clone(),
            gate: gate.clone(),
        });
        let frames = std::collections::VecDeque::from([
            Frame::binary(Bytes::from_static(b"slow")),
            Frame::ping(),
        ]);
        let (wtx, mut wrx) = mpsc::unbounded_channel();
        let (_ch, read_loop) = Channel::pair(
            "c1",
            ScriptedRead {
                frames: StdMutex::new(frames),
            },
            OpcodeRec { tx: wtx },
            ChannelOpts {
                read_wait: Duration::from_secs(5),
                write_wait: Duration::from_secs(2),
                ..ChannelOpts::default()
            },
        );
        tokio::spawn(async move {
            let _ = read_loop.run(listener).await;
        });
        timeout(Duration::from_secs(2), started.notified())
            .await
            .expect("slow handler should start");
        let op = timeout(Duration::from_secs(2), wrx.recv())
            .await
            .expect("pong should not wait for the handler")
            .expect("pong opcode");
        assert_eq!(op, OpCode::Pong);
        gate.notify_waiters();
    }

    #[tokio::test]
    async fn write_full_disconnect_does_not_block() {
        let entered = Arc::new(Notify::new());
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        let (ch, _read_loop) = Channel::pair(
            "slow",
            NullConn,
            HangWrite {
                entered: entered.clone(),
            },
            ChannelOpts {
                write_queue: 4,
                write_full: WriteFullPolicy::Disconnect,
                write_wait: Duration::from_secs(30),
                on_mailbox_full: Some(Arc::new(move || {
                    hits2.fetch_add(1, Ordering::SeqCst);
                })),
                ..ChannelOpts::default()
            },
        );
        ch.push(Bytes::from_static(b"first")).await.unwrap();
        timeout(Duration::from_secs(2), entered.notified())
            .await
            .expect("writer should take the first frame");
        let mut saw_full = false;
        for _ in 0..16 {
            match timeout(
                Duration::from_millis(200),
                ch.push(Bytes::from_static(b"x")),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(Error::MailboxFull)) | Ok(Err(Error::Closed)) => {
                    saw_full = true;
                    break;
                }
                Ok(Err(e)) => panic!("unexpected error {e}"),
                Err(_) => panic!("Disconnect push blocked forever"),
            }
        }
        assert!(saw_full, "try_send must fail instead of blocking");
        assert!(ch.is_closed());
        assert!(hits.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn in_flight_full_stops_second_lane() {
        let a_started = Arc::new(Notify::new());
        let b_started = Arc::new(Notify::new());
        let gate = Arc::new(Notify::new());
        let saw_overlap = Arc::new(AtomicBool::new(false));
        let listener = Arc::new(OverlapListener {
            a_started: a_started.clone(),
            b_started: b_started.clone(),
            a_running: Arc::new(AtomicBool::new(false)),
            b_running: Arc::new(AtomicBool::new(false)),
            saw_overlap: saw_overlap.clone(),
            gate: gate.clone(),
        });
        let frames = std::collections::VecDeque::from([
            Frame::binary(Bytes::from_static(b"ch-a")),
            Frame::binary(Bytes::from_static(b"ch-b")),
        ]);
        let (_ch, read_loop) = Channel::pair(
            "gw-1",
            ScriptedRead {
                frames: StdMutex::new(frames),
            },
            NullConn,
            ChannelOpts {
                read_wait: Duration::from_secs(5),
                max_in_flight: 1,
                lane_key: Some(payload_lane_key()),
                ..ChannelOpts::default()
            },
        );
        tokio::spawn(async move {
            let _ = read_loop.run(listener).await;
        });
        timeout(Duration::from_secs(2), a_started.notified())
            .await
            .expect("first lane starts");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !saw_overlap.load(Ordering::SeqCst),
            "process semaphore must stop a second in-flight handler"
        );
        gate.notify_waiters();
        let _ = timeout(Duration::from_secs(2), b_started.notified()).await;
    }

    #[tokio::test]
    async fn idle_lane_retires() {
        struct RecvOnce {
            saw: Arc<AtomicUsize>,
            gate: Arc<Notify>,
        }
        #[async_trait]
        impl MessageListener for RecvOnce {
            async fn receive(
                &self,
                _handle: &dyn ChannelHandle,
                _payload: Bytes,
            ) -> Result<(), Error> {
                self.saw.fetch_add(1, Ordering::SeqCst);
                self.gate.notify_waiters();
                Ok(())
            }
        }
        let write = WriteShared::spawn(
            NullConn,
            8,
            Duration::from_secs(2),
            WriteFullPolicy::Block,
            None,
        );
        let handle = PushHandle {
            id: Arc::from("c1"),
            write,
        };
        let saw = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(Notify::new());
        let lanes = LaneSet::new(
            Arc::new(Semaphore::new(8)),
            8,
            Duration::from_millis(50),
            Arc::new(RecvOnce {
                saw: saw.clone(),
                gate: gate.clone(),
            }),
            handle,
        );
        lanes
            .enqueue(Arc::from("k"), Bytes::from_static(b"x"))
            .await
            .unwrap();
        timeout(Duration::from_secs(1), gate.notified())
            .await
            .expect("handler ran");
        assert_eq!(lanes.lane_len(), 1);
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(lanes.lane_len(), 0);
        assert_eq!(saw.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn drain_unblocks_before_idle_timeout() {
        struct RecvOnce {
            saw: Arc<AtomicUsize>,
            gate: Arc<Notify>,
        }
        #[async_trait]
        impl MessageListener for RecvOnce {
            async fn receive(
                &self,
                _handle: &dyn ChannelHandle,
                _payload: Bytes,
            ) -> Result<(), Error> {
                self.saw.fetch_add(1, Ordering::SeqCst);
                self.gate.notify_waiters();
                Ok(())
            }
        }
        let write = WriteShared::spawn(
            NullConn,
            8,
            Duration::from_secs(2),
            WriteFullPolicy::Block,
            None,
        );
        let handle = PushHandle {
            id: Arc::from("c1"),
            write,
        };
        let saw = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(Notify::new());
        let lanes = LaneSet::new(
            Arc::new(Semaphore::new(8)),
            8,
            Duration::from_secs(30),
            Arc::new(RecvOnce {
                saw: saw.clone(),
                gate: gate.clone(),
            }),
            handle,
        );
        lanes
            .enqueue(Arc::from("k"), Bytes::from_static(b"x"))
            .await
            .unwrap();
        timeout(Duration::from_secs(1), gate.notified())
            .await
            .expect("handler ran");
        timeout(Duration::from_secs(2), lanes.drain())
            .await
            .expect("drain must observe sender closure instead of waiting out idle");
        assert_eq!(saw.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn enqueue_during_idle_retirement_delivers() {
        struct CountRecv {
            saw: Arc<AtomicUsize>,
        }
        #[async_trait]
        impl MessageListener for CountRecv {
            async fn receive(
                &self,
                _handle: &dyn ChannelHandle,
                _payload: Bytes,
            ) -> Result<(), Error> {
                self.saw.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
        let write = WriteShared::spawn(
            NullConn,
            8,
            Duration::from_secs(2),
            WriteFullPolicy::Block,
            None,
        );
        let handle = PushHandle {
            id: Arc::from("c1"),
            write,
        };
        let saw = Arc::new(AtomicUsize::new(0));
        let lanes = LaneSet::new(
            Arc::new(Semaphore::new(32)),
            8,
            Duration::from_millis(5),
            Arc::new(CountRecv { saw: saw.clone() }),
            handle,
        );
        const N: usize = 200;
        for i in 0..N {
            lanes
                .enqueue(Arc::from("k"), Bytes::from_static(b"x"))
                .await
                .unwrap();
            if i % 15 == 14 {
                tokio::time::sleep(Duration::from_millis(8)).await;
            }
        }
        timeout(Duration::from_secs(2), async {
            loop {
                if saw.load(Ordering::SeqCst) >= N {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("every enqueued frame must be delivered across idle retirement");
        timeout(Duration::from_secs(2), lanes.drain())
            .await
            .expect("drain after retirement stress");
    }
}
