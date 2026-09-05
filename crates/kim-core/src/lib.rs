//! KIM 通信层的「说明书」。
//!
//! TCP / WebSocket 各自实现 [`Conn`]，业务只依赖这里的 trait。

mod channel;
mod channel_map;
mod conn;
mod error;
mod frame;
mod opcode;
mod server;
mod signal;
mod socket;

pub use channel::{
    Channel, ChannelOpts, ChannelReadLoop, LaneKeyFn, MailboxFullHook, WriteFullPolicy,
};
pub use channel_map::ChannelMap;
pub use conn::{
    Acceptor, ChannelHandle, Conn, Dialer, DialerContext, MessageListener, StateListener,
};
pub use error::Error;
pub use frame::Frame;
pub use opcode::OpCode;
pub use server::{Client, Server};
pub use signal::wait_shutdown_signal;
pub use socket::{apply_socket_opts, Keepalive, SocketOpts};

use std::time::Duration;

/// 握手最多等多久。超时就断开，避免半开连接占着资源。
pub const DEFAULT_LOGIN_WAIT: Duration = Duration::from_secs(10);
/// 多久没读到数据就当对方掉线。应大于客户端心跳间隔。
pub const DEFAULT_READ_WAIT: Duration = Duration::from_secs(60);
/// 单次写出最多等多久。
pub const DEFAULT_WRITE_WAIT: Duration = Duration::from_secs(10);
/// 客户端默认多久发一次 Ping。
pub const DEFAULT_HEARTBEAT: Duration = Duration::from_secs(30);
/// Shutdown waits this long for in-flight connection tasks, then aborts them.
pub const DEFAULT_DRAIN_WAIT: Duration = Duration::from_secs(15);
/// Default write mailbox depth. Full + Disconnect fails Push instead of blocking.
pub const DEFAULT_WRITE_QUEUE: usize = 64;
/// Per-lane queue and process-wide in-flight handler budget.
pub const DEFAULT_MAX_IN_FLIGHT: usize = 64;
