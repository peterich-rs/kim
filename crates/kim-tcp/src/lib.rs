//! KIM 通信层的 TCP 实现。

mod client;
mod codec;
mod conn;
mod server;

pub use client::{ClientOptions, IdentityDialer, TcpClient, TcpDialer};
pub use codec::{decode_frame, encode_frame, HEADER_LEN, MAX_PAYLOAD};
pub use conn::{TcpConn, TcpReadHalf, TcpWriteHalf};
pub use server::TcpServer;
