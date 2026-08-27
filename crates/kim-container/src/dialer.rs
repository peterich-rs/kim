use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{Conn, DialerContext, Error, OpCode};
use kim_protocol::pkt::InnerHandshakeReq;
use kim_tcp::{TcpConn, TcpDialer};
use prost::Message;

pub struct InnerTcpDialer {
    pub local_service_id: String,
}

#[async_trait]
impl TcpDialer for InnerTcpDialer {
    async fn dial_and_handshake(&self, ctx: DialerContext) -> Result<TcpConn, Error> {
        let stream = tokio::net::TcpStream::connect(&ctx.address).await?;
        let mut conn = TcpConn::new(stream);
        let req = InnerHandshakeReq {
            service_id: self.local_service_id.clone(),
        };
        conn.write_frame(OpCode::Binary, Bytes::from(req.encode_to_vec()))
            .await?;
        conn.flush().await?;
        Ok(conn)
    }
}
