use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;

use crate::{Error, Frame, OpCode};

/// 一条已经建立的连接。握手阶段 Server/Client 都拿这个读写。
///
/// 实现者负责「从字节流里切出一帧 / 把一帧写回去」。
/// TCP 和 WebSocket 各写一份，上层 Accept/Receive 不用分协议。
#[async_trait]
pub trait Conn: Send {
    async fn read_frame(&mut self) -> Result<Frame, Error>;
    async fn write_frame(&mut self, opcode: OpCode, payload: Bytes) -> Result<(), Error>;
    async fn flush(&mut self) -> Result<(), Error>;
    async fn shutdown(&mut self) -> Result<(), Error>;

    /// 对端地址。仅未 split 的连接有值；读/写半边默认 None。
    fn peer_addr(&self) -> Option<String> {
        None
    }
}

/// 新连接进来后，由业务决定「这是谁」。
///
/// 返回的字符串就是 channel_id（这条连接的临时身份证）。
/// 返回 Err 则通信层会关掉这条连接。
///
/// 第一期 echo 里：客户端先发自己的名字，这里读出来当 id。
/// 以后登录：这里读 JWT、校验，再把用户和连接绑在一起。
#[async_trait]
pub trait Acceptor: Send + Sync {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error>;

    /// ChannelMap.add 成功、且 MessageListener 已设置之后、read_loop 之前。默认 Ok。
    async fn on_channel_ready(&self, _channel_id: &str) -> Result<(), Error> {
        Ok(())
    }

    /// Accept 已 Ok，但这条连接不会进入 read_loop（id 重复 / 未设 MessageListener）。
    async fn on_accept_abandoned(&self, _channel_id: &str) {}
}

/// 收到一帧业务数据（Ping/Pong/Close 已经被 Channel 吃掉）。
///
/// 不保证与读循环同任务；同一 `header.channel_id`（或连接 id）上仍 FIFO。
/// 第一个参数是 [`ChannelHandle`]：只能回消息、能知道 id，不能直接把连接关掉。
#[async_trait]
pub trait MessageListener: Send + Sync {
    async fn receive(&self, handle: &dyn ChannelHandle, payload: Bytes);
}

/// 连接断开时通知业务。以后这里会清会话、改在线状态。
#[async_trait]
pub trait StateListener: Send + Sync {
    async fn disconnect(&self, channel_id: &str) -> Result<(), Error>;
}

/// 业务层能对一条连接做的最小操作：我是谁、推一串字节。
///
/// 不能关连接；关连接是通信层的事（[`crate::Server::close_channel`]）。
#[async_trait]
pub trait ChannelHandle: Send + Sync {
    fn id(&self) -> &str;
    async fn push(&self, payload: Bytes) -> Result<(), Error>;
}

/// 客户端去连别人时，怎么拨号、怎么握手，丢给业务。
#[async_trait]
pub trait Dialer: Send + Sync {
    async fn dial_and_handshake(&self, ctx: DialerContext) -> Result<Box<dyn Conn>, Error>;
}

#[derive(Clone, Debug)]
pub struct DialerContext {
    pub id: String,
    pub name: String,
    pub address: String,
    pub timeout: Duration,
}
