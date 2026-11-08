use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, Notify};

use crate::{Conn, Error, MailboxFullHook, OpCode, WriteFullPolicy};

/// 写协程接收的任务。业务 Push、心跳 Pong、关闭全部走这里，
/// 避免读循环和写循环同时写同一条 TCP 流。
enum WriteOp {
    Frame { opcode: OpCode, payload: Bytes },
    Close,
}

/// N-to-1 write handle. Clone freely; the socket lives in one spawned task.
///
/// Not a business API: `Channel` and `TcpClient` share this so neither holds a
/// write-half Mutex across `write_frame`.
#[derive(Clone)]
pub struct WriteShared {
    tx: mpsc::Sender<WriteOp>,
    closed: Arc<AtomicBool>,
    aborted: Arc<AtomicBool>,
    abort: Arc<Notify>,
    write_full: WriteFullPolicy,
    on_mailbox_full: Option<MailboxFullHook>,
}

impl WriteShared {
    pub fn spawn<W>(
        mut writer: W,
        write_queue: usize,
        write_wait: Duration,
        write_full: WriteFullPolicy,
        on_mailbox_full: Option<MailboxFullHook>,
    ) -> Self
    where
        W: Conn + 'static,
    {
        let write_queue = write_queue.max(1);
        let (tx, mut rx) = mpsc::channel(write_queue);
        let closed = Arc::new(AtomicBool::new(false));
        let aborted = Arc::new(AtomicBool::new(false));
        let abort = Arc::new(Notify::new());
        let write = Self {
            tx: tx.clone(),
            closed: closed.clone(),
            aborted: aborted.clone(),
            abort: abort.clone(),
            write_full,
            on_mailbox_full,
        };

        let writer_closed = closed.clone();
        let writer_aborted = aborted.clone();
        tokio::spawn(async move {
            loop {
                if writer_aborted.load(Ordering::SeqCst) {
                    let _ = writer.shutdown().await;
                    break;
                }
                tokio::select! {
                    _ = abort.notified() => {
                        let _ = writer.shutdown().await;
                        break;
                    }
                    first = rx.recv() => {
                        let Some(first) = first else {
                            let _ = writer.shutdown().await;
                            break;
                        };
                        let mut batch = vec![first];
                        while let Ok(more) = rx.try_recv() {
                            batch.push(more);
                        }
                        let mut saw_close = false;
                        let mut frames = Vec::new();
                        for op in batch {
                            match op {
                                WriteOp::Frame { opcode, payload } => frames.push((opcode, payload)),
                                WriteOp::Close => saw_close = true,
                            }
                        }
                        let write_batch = async {
                            for (opcode, payload) in frames {
                                writer.write_frame(opcode, payload).await?;
                            }
                            writer.flush().await?;
                            if saw_close {
                                writer.shutdown().await?;
                            }
                            Ok::<(), Error>(())
                        };
                        match tokio::time::timeout(write_wait, write_batch).await {
                            Ok(Ok(())) => {
                                if saw_close {
                                    break;
                                }
                            }
                            _ => {
                                let _ = writer.shutdown().await;
                                break;
                            }
                        }
                    }
                }
            }
            writer_closed.store(true, Ordering::SeqCst);
        });

        write
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub async fn push_frame(&self, opcode: OpCode, payload: Bytes) -> Result<(), Error> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(Error::Closed);
        }
        let op = WriteOp::Frame { opcode, payload };
        match self.write_full {
            WriteFullPolicy::Block => self.tx.send(op).await.map_err(|_| Error::Closed),
            WriteFullPolicy::Disconnect => match self.tx.try_send(op) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(_)) => {
                    self.trip_full();
                    Err(Error::MailboxFull)
                }
                Err(TrySendError::Closed(_)) => Err(Error::Closed),
            },
        }
    }

    fn trip_full(&self) {
        if let Some(hook) = &self.on_mailbox_full {
            hook();
        }
        self.closed.store(true, Ordering::SeqCst);
        self.aborted.store(true, Ordering::SeqCst);
        let _ = self.tx.try_send(WriteOp::Close);
        self.abort.notify_waiters();
    }

    pub async fn close(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        match self.write_full {
            WriteFullPolicy::Block => {
                let _ = self.tx.send(WriteOp::Close).await;
            }
            WriteFullPolicy::Disconnect => {
                if self.tx.try_send(WriteOp::Close).is_err() {
                    self.aborted.store(true, Ordering::SeqCst);
                    self.abort.notify_waiters();
                }
            }
        }
    }
}
