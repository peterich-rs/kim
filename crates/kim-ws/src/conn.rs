//! `read_frame` 不是 cancel-safe：超时后必须 drop 整条连接，禁止继续读。

use async_trait::async_trait;
use bytes::Bytes;
use fastwebsockets::{Frame, Payload, WebSocket, WebSocketRead, WebSocketWrite};
use tokio::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};

use kim_core::{Conn, Error, Frame as KimFrame, OpCode};

use crate::opcode::{from_ws, to_ws};

pub struct WsConn<S> {
    pub ws: WebSocket<S>,
    pub peer: Option<String>,
}

pub struct WsReadHalf<S>
where
    S: AsyncRead + AsyncWrite,
{
    inner: WebSocketRead<ReadHalf<S>>,
}

pub struct WsWriteHalf<S>
where
    S: AsyncRead + AsyncWrite,
{
    inner: WebSocketWrite<WriteHalf<S>>,
}

impl<S> WsConn<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub fn into_split(self) -> (WsReadHalf<S>, WsWriteHalf<S>) {
        let (read, write) = self.ws.split(tokio::io::split);
        (WsReadHalf { inner: read }, WsWriteHalf { inner: write })
    }
}

fn map_ws_err(e: fastwebsockets::WebSocketError) -> Error {
    let msg = e.to_string();
    if msg.to_lowercase().contains("too large") || msg.contains("max") {
        Error::FrameTooLarge {
            size: 0,
            max: 1024 * 1024,
        }
    } else {
        Error::other(msg)
    }
}

fn kim_frame(frame: Frame<'_>) -> Result<KimFrame, Error> {
    Ok(KimFrame {
        opcode: from_ws(frame.opcode)?,
        payload: Bytes::copy_from_slice(&frame.payload),
    })
}

fn ws_frame(opcode: OpCode, payload: Bytes) -> Frame<'static> {
    Frame::new(true, to_ws(opcode), None, Payload::Owned(payload.to_vec()))
}

#[async_trait]
impl<S> Conn for WsConn<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    async fn read_frame(&mut self) -> Result<KimFrame, Error> {
        let frame = self.ws.read_frame().await.map_err(map_ws_err)?;
        kim_frame(frame)
    }

    async fn write_frame(&mut self, opcode: OpCode, payload: Bytes) -> Result<(), Error> {
        self.ws
            .write_frame(ws_frame(opcode, payload))
            .await
            .map_err(map_ws_err)
    }

    async fn flush(&mut self) -> Result<(), Error> {
        // 写出完成以 write_frame 返回为准；今日无额外缓冲。
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        let _ = self
            .ws
            .write_frame(ws_frame(OpCode::Close, Bytes::new()))
            .await;
        Ok(())
    }

    fn peer_addr(&self) -> Option<String> {
        self.peer.clone()
    }
}

#[async_trait]
impl<S> Conn for WsReadHalf<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    async fn read_frame(&mut self) -> Result<KimFrame, Error> {
        let mut send_fn = |frame| async move {
            let _ = frame;
            Ok::<(), fastwebsockets::WebSocketError>(())
        };
        let frame = self
            .inner
            .read_frame(&mut send_fn)
            .await
            .map_err(map_ws_err)?;
        kim_frame(frame)
    }

    async fn write_frame(&mut self, _opcode: OpCode, _payload: Bytes) -> Result<(), Error> {
        Err(Error::other("read half cannot write"))
    }

    async fn flush(&mut self) -> Result<(), Error> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

#[async_trait]
impl<S> Conn for WsWriteHalf<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    async fn read_frame(&mut self) -> Result<KimFrame, Error> {
        Err(Error::other("write half cannot read"))
    }

    async fn write_frame(&mut self, opcode: OpCode, payload: Bytes) -> Result<(), Error> {
        self.inner
            .write_frame(ws_frame(opcode, payload))
            .await
            .map_err(map_ws_err)
    }

    async fn flush(&mut self) -> Result<(), Error> {
        // 写出完成以 write_frame 返回为准；今日无额外缓冲。
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        let _ = self
            .inner
            .write_frame(ws_frame(OpCode::Close, Bytes::new()))
            .await;
        Ok(())
    }
}
