use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use tokio::io::{
    AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter, ReadHalf, WriteHalf,
};
use tokio::net::TcpStream;

use kim_core::{Conn, Error, Frame, OpCode};

use crate::codec::{decode_frame, header_bytes};

/// 一条 TCP 连接。握手阶段读写都走它；握手后拆成读半边 / 写半边。
pub struct TcpConn<S = TcpStream> {
    stream: S,
    read_buf: BytesMut,
    peer: Option<String>,
}

pub struct TcpReadHalf<S = TcpStream> {
    stream: ReadHalf<S>,
    read_buf: BytesMut,
}

pub struct TcpWriteHalf<S = TcpStream> {
    stream: BufWriter<WriteHalf<S>>,
}

/// 明文别名：内部链路（Chat、TcpClient、InnerTcpDialer）继续用它。
pub type PlainTcpConn = TcpConn<TcpStream>;
pub type PlainTcpReadHalf = TcpReadHalf<TcpStream>;
pub type PlainTcpWriteHalf = TcpWriteHalf<TcpStream>;

impl<S> TcpConn<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub fn with_peer(stream: S, peer: Option<String>) -> Self {
        Self {
            stream,
            read_buf: BytesMut::with_capacity(4096),
            peer,
        }
    }

    pub fn into_split(self) -> (TcpReadHalf<S>, TcpWriteHalf<S>) {
        let (read, write) = tokio::io::split(self.stream);
        (
            TcpReadHalf {
                stream: read,
                read_buf: self.read_buf,
            },
            TcpWriteHalf {
                stream: BufWriter::with_capacity(8192, write),
            },
        )
    }
}

impl TcpConn<TcpStream> {
    /// 明文构造：`set_nodelay` + `peer_addr`。签名保持不变。
    pub fn new(stream: TcpStream) -> Self {
        let _ = stream.set_nodelay(true);
        let peer = stream.peer_addr().ok().map(|a| a.to_string());
        Self::with_peer(stream, peer)
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

async fn write_frame_parts<W: AsyncWriteExt + Unpin>(
    stream: &mut W,
    opcode: OpCode,
    payload: &[u8],
) -> Result<(), Error> {
    let header = header_bytes(opcode, payload.len());
    stream.write_all(&header).await?;
    stream.write_all(payload).await?;
    Ok(())
}

#[async_trait]
impl<S> Conn for TcpConn<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    async fn read_frame(&mut self) -> Result<Frame, Error> {
        fill_and_decode(&mut self.stream, &mut self.read_buf).await
    }

    async fn write_frame(&mut self, opcode: OpCode, payload: Bytes) -> Result<(), Error> {
        write_frame_parts(&mut self.stream, opcode, &payload).await
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
impl<S> Conn for TcpReadHalf<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
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
impl<S> Conn for TcpWriteHalf<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    async fn read_frame(&mut self) -> Result<Frame, Error> {
        Err(Error::other("write half cannot read"))
    }

    async fn write_frame(&mut self, opcode: OpCode, payload: Bytes) -> Result<(), Error> {
        write_frame_parts(&mut self.stream, opcode, &payload).await
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
