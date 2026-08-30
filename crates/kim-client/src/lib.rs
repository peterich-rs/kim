//! TDLib-shaped KIM client: session, login, talk, ping, ack.
//!
//! Business talks to [`kim_core::Conn`]. This crate uses [`kim_ws::connect_ws`]
//! as the Conn impl (WGateway `ws://` / `wss://`). TCP / QUIC later is a new
//! Conn impl — login/talk/ack stay the same.

mod auth;
mod client;
mod config;
mod error;
mod events;
mod login;
mod pump;
mod session;
mod token;
mod wire;

pub use auth::{AuthClient, AuthSession};
pub use client::KimClient;
pub use config::{
    http_origin_from_ws, ClientConfig, DEFAULT_CLIENT_USER_AGENT, DEFAULT_DEVICE,
    DEFAULT_LOCAL_HTTP_ORIGIN, DEFAULT_LOCAL_URL, DEFAULT_PROD_HTTP_ORIGIN, DEFAULT_PROD_URL,
};
pub use error::ClientError;
pub use events::{Event, IncomingTalk, Profile, TalkResult};
pub use login::{login_on_conn, send_ping, wait_pong};
pub use session::MemorySession;
pub use token::account_from_token;
pub use wire::{
    decode_event, encode_ack, encode_dest_cmd, encode_empty_cmd, encode_login, encode_ping,
    encode_user_search, encode_user_talk, is_kickout,
};

#[cfg(test)]
mod tests;
