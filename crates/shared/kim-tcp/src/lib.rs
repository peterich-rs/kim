//! KIM 通信层的 TCP 实现。

mod client;
mod codec;
mod conn;
mod opts;
mod server;

pub use client::{ClientOptions, IdentityDialer, TcpClient, TcpDialer};
pub use codec::{decode_frame, encode_frame, header_bytes, HEADER_LEN, MAX_PAYLOAD};
pub use conn::{
    PlainTcpConn, PlainTcpReadHalf, PlainTcpWriteHalf, TcpConn, TcpReadHalf, TcpWriteHalf,
};
pub use opts::{Keepalive, SocketOpts};
pub use server::{
    acquire_permit, apply_socket_opts, serve_conn, serve_tcp_conn, FrontendState, ServeConnCtx,
    TcpServer,
};
