use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;

use kim_core::{Conn, Error, Frame, OpCode};

use crate::codec::{decode_frame, encode_frame};

/// 一条 TCP 连接。握手阶段读写都走它；握手后拆成读半边 / 写半边。
pub struct TcpConn {
    stream: TcpStream,
    read_buf: BytesMut,
    peer: Option<String>,
}

pub struct TcpReadHalf {
    stream: ReadHalf<TcpStream>,
    read_buf: BytesMut,
}

pub struct TcpWriteHalf {
    stream: WriteHalf<TcpStream>,
}

impl TcpConn {
    pub fn new(stream: TcpStream) -> Self {
        let _ = stream.set_nodelay(true);
        let peer = stream.peer_addr().ok().map(|a| a.to_string());
        Self {
            stream,
            read_buf: BytesMut::with_capacity(4096),
            peer,
        }
    }

    pub fn into_split(self) -> (TcpReadHalf, TcpWriteHalf) {
        let (read, write) = tokio::io::split(self.stream);
        (
            TcpReadHalf {
                stream: read,
                read_buf: self.read_buf,
            },
            TcpWriteHalf { stream: write },
        )
    }
}

async fn fill_and_decode<R: AsyncReadExt + Unpin>(
    stream: &mut R,
    buf: &mut BytesMut,
) -> Result<Frame, Error> {
    loop {
        if let Some(frame) = decode_frame(buf)? {
            return Ok(frame);
        }
        let n = stream.read_buf(buf).await?;
        if n == 0 {
            return Err(Error::Closed);
        }
    }
}

async fn write_all<W: AsyncWriteExt + Unpin>(
    stream: &mut W,
    opcode: OpCode,
    payload: &[u8],
) -> Result<(), Error> {
    let buf = encode_frame(opcode, payload);
    stream.write_all(&buf).await?;
    Ok(())
}

#[async_trait]
impl Conn for TcpConn {
    async fn read_frame(&mut self) -> Result<Frame, Error> {
        fill_and_decode(&mut self.stream, &mut self.read_buf).await
    }

    async fn write_frame(&mut self, opcode: OpCode, payload: Bytes) -> Result<(), Error> {
        write_all(&mut self.stream, opcode, &payload).await
    }

    async fn flush(&mut self) -> Result<(), Error> {
        self.stream.flush().await?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        self.stream.shutdown().await?;
        Ok(())
    }

    fn peer_addr(&self) -> Option<String> {
        self.peer.clone()
    }
}

#[async_trait]
impl Conn for TcpReadHalf {
    async fn read_frame(&mut self) -> Result<Frame, Error> {
        fill_and_decode(&mut self.stream, &mut self.read_buf).await
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
impl Conn for TcpWriteHalf {
    async fn read_frame(&mut self) -> Result<Frame, Error> {
        Err(Error::other("write half cannot read"))
    }

    async fn write_frame(&mut self, opcode: OpCode, payload: Bytes) -> Result<(), Error> {
        write_all(&mut self.stream, opcode, &payload).await
    }

    async fn flush(&mut self) -> Result<(), Error> {
        self.stream.flush().await?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        self.stream.shutdown().await?;
        Ok(())
    }
}
