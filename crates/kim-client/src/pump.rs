//! Dedicated reader + writer after login.
//!
//! `kim-ws` `read_frame` is not cancel-safe. A background reader owns the read
//! half for the life of the session; talks wait on oneshots keyed by sequence.

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use kim_core::{Conn, Error as CoreError, Frame, OpCode};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::AbortHandle;

use crate::events::Event;
use crate::wire::decode_event;
use crate::ClientError;

const WRITE_CAP: usize = 32;
const EVENT_CAP: usize = 64;

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
        rx.await.map_err(|_| ClientError::from(CoreError::Closed))?
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

    pub(crate) async fn ping(&self, payload: Bytes) -> Result<(), ClientError> {
        let (tx, rx) = oneshot::channel();
        *self.ping.lock().await = Some(tx);
        if let Err(err) = self.write_frame(OpCode::Binary, payload).await {
            self.ping.lock().await.take();
            return Err(err);
        }
        rx.await.map_err(|_| ClientError::from(CoreError::Closed))?;
        Ok(())
    }

    pub(crate) async fn recv(&self) -> Result<Event, ClientError> {
        let mut rx = self.events.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| ClientError::from(CoreError::Closed))
    }

    pub(crate) fn shutdown(&self) {
        let _ = self.writes.try_send(WriteCmd::Shutdown);
        self.reader_abort.abort();
        self.writer_abort.abort();
    }
}

pub(crate) fn start_split_pump(
    mut read: Box<dyn Conn + Send>,
    mut write: Box<dyn Conn + Send>,
) -> Arc<Live> {
    let (write_tx, mut write_rx) = mpsc::channel(WRITE_CAP);
    let (event_tx, event_rx) = mpsc::channel(EVENT_CAP);
    let pending = Arc::new(Mutex::new(HashMap::new()));
    let ping = Arc::new(Mutex::new(None));

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
    });

    let pending_r = pending.clone();
    let ping_r = ping.clone();
    let writes_r = write_tx.clone();
    let reader = tokio::spawn(async move {
        loop {
            match read_data_pumped(&mut *read, &writes_r).await {
                Ok(frame) => match decode_event(&frame) {
                    Ok(event) => {
                        let closed = matches!(event, Event::Closed);
                        dispatch(event, &pending_r, &ping_r, &event_tx).await;
                        if closed {
                            break;
                        }
                    }
                    Err(_) => {
                        dispatch(Event::Closed, &pending_r, &ping_r, &event_tx).await;
                        break;
                    }
                },
                Err(_) => {
                    dispatch(Event::Closed, &pending_r, &ping_r, &event_tx).await;
                    break;
                }
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
    })
}

async fn write_one(
    write: &mut dyn Conn,
    opcode: OpCode,
    payload: Bytes,
) -> Result<(), ClientError> {
    write.write_frame(opcode, payload).await?;
    write.flush().await?;
    Ok(())
}

async fn read_data_pumped(
    conn: &mut dyn Conn,
    writes: &mpsc::Sender<WriteCmd>,
) -> Result<Frame, ClientError> {
    loop {
        let frame = conn.read_frame().await?;
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
        | Event::Profile { sequence, .. } => {
            if let Some(tx) = pending.lock().await.remove(sequence) {
                let _ = tx.send(event);
                return;
            }
        }
        Event::Pong => {
            if let Some(tx) = ping.lock().await.take() {
                let _ = tx.send(());
                return;
            }
        }
        Event::Closed => {
            pending.lock().await.clear();
            ping.lock().await.take();
            let _ = events.send(event).await;
            return;
        }
        _ => {}
    }
    let _ = events.send(event).await;
}
