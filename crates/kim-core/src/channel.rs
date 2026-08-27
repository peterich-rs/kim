use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::mpsc;
use tracing::debug;

use crate::{Agent, Conn, Error, MessageListener, OpCode};

/// 写协程接收的任务。业务 Push、心跳 Pong、关闭全部走这里，
/// 避免读循环和写循环同时写同一条 TCP 流。
enum WriteOp {
    Frame { opcode: OpCode, payload: Bytes },
    Close,
}

/// 连接的上层包装。Server 管理的是 Channel，不是裸 Conn。
///
/// Push 把字节丢进队列就返回；真正的 write 在独立写协程里。
#[derive(Clone)]
pub struct Channel {
    id: Arc<str>,
    tx: mpsc::Sender<WriteOp>,
    closed: Arc<AtomicBool>,
}

pub struct ChannelOpts {
    pub read_wait: Duration,
    pub write_wait: Duration,
    /// 写队列长度。满了 Push 会失败，避免慢连接把内存撑爆。
    pub write_queue: usize,
}

impl Default for ChannelOpts {
    fn default() -> Self {
        Self {
            read_wait: crate::DEFAULT_READ_WAIT,
            write_wait: crate::DEFAULT_WRITE_WAIT,
            write_queue: 64,
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
        let (tx, mut rx) = mpsc::channel(opts.write_queue);
        let closed = Arc::new(AtomicBool::new(false));
        let write_wait = opts.write_wait;

        let mut writer = writer;
        let writer_closed = closed.clone();
        tokio::spawn(async move {
            while let Some(op) = rx.recv().await {
                match op {
                    WriteOp::Frame { opcode, payload } => {
                        let write = writer.write_frame(opcode, payload);
                        match tokio::time::timeout(write_wait, write).await {
                            Ok(Ok(())) => {
                                if writer.flush().await.is_err() {
                                    break;
                                }
                            }
                            _ => break,
                        }
                    }
                    WriteOp::Close => {
                        let _ = writer.shutdown().await;
                        break;
                    }
                }
            }
            writer_closed.store(true, Ordering::SeqCst);
        });

        let channel = Self {
            id: id.clone(),
            tx: tx.clone(),
            closed: closed.clone(),
        };
        let read_loop = ChannelReadLoop {
            id,
            reader,
            tx,
            closed,
            read_wait: opts.read_wait,
        };
        (channel, read_loop)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub async fn close(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = self.tx.send(WriteOp::Close).await;
    }
}

#[async_trait]
impl Agent for Channel {
    fn id(&self) -> &str {
        &self.id
    }

    async fn push(&self, payload: Bytes) -> Result<(), Error> {
        send_binary(&self.tx, &self.closed, payload).await
    }
}

/// 占着读半边、阻塞直到连接断开。只应被一个任务运行。
pub struct ChannelReadLoop<R> {
    id: Arc<str>,
    reader: R,
    tx: mpsc::Sender<WriteOp>,
    closed: Arc<AtomicBool>,
    read_wait: Duration,
}

impl<R: Conn> ChannelReadLoop<R> {
    pub async fn run(mut self, listener: Arc<dyn MessageListener>) -> Result<(), Error> {
        let result = self.read_until_err(listener).await;
        self.closed.store(true, Ordering::SeqCst);
        let _ = self.tx.send(WriteOp::Close).await;
        result
    }

    async fn read_until_err(&mut self, listener: Arc<dyn MessageListener>) -> Result<(), Error> {
        loop {
            let frame = match tokio::time::timeout(self.read_wait, self.reader.read_frame()).await {
                Ok(Ok(frame)) => frame,
                Ok(Err(err)) => return Err(err),
                Err(_) => return Err(Error::Closed),
            };

            match frame.opcode {
                OpCode::Close => return Err(Error::Closed),
                OpCode::Ping => {
                    debug!(channel = %self.id, "recv ping, reply pong");
                    self.tx
                        .send(WriteOp::Frame {
                            opcode: OpCode::Pong,
                            payload: Bytes::new(),
                        })
                        .await
                        .map_err(|_| Error::Closed)?;
                }
                OpCode::Pong => {
                    debug!(channel = %self.id, "recv pong");
                }
                OpCode::Binary | OpCode::Text => {
                    if frame.payload.is_empty() {
                        continue;
                    }
                    let agent = ChannelAgent {
                        id: self.id.clone(),
                        tx: self.tx.clone(),
                        closed: self.closed.clone(),
                    };
                    listener.receive(&agent, frame.payload).await;
                }
                OpCode::Continuation => {}
            }
        }
    }
}

/// 读循环交给 MessageListener 的手柄：能回消息，不能关连接。
struct ChannelAgent {
    id: Arc<str>,
    tx: mpsc::Sender<WriteOp>,
    closed: Arc<AtomicBool>,
}

#[async_trait]
impl Agent for ChannelAgent {
    fn id(&self) -> &str {
        &self.id
    }

    async fn push(&self, payload: Bytes) -> Result<(), Error> {
        send_binary(&self.tx, &self.closed, payload).await
    }
}

async fn send_binary(
    tx: &mpsc::Sender<WriteOp>,
    closed: &AtomicBool,
    payload: Bytes,
) -> Result<(), Error> {
    if closed.load(Ordering::SeqCst) {
        return Err(Error::Closed);
    }
    tx.send(WriteOp::Frame {
        opcode: OpCode::Binary,
        payload,
    })
    .await
    .map_err(|_| Error::Closed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, Frame};
    use tokio::sync::mpsc;
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
            Ok(())
        }
        async fn shutdown(&mut self) -> Result<(), Error> {
            let _ = self.tx.send("close");
            Ok(())
        }
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
        let first = timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let second = timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!((first, second), ("binary", "close"));
    }
}
