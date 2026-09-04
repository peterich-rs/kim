//! TDLib-shaped KIM client: session, login, talk, ping, ack, inbox/history/offline, supervisor.
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
mod supervisor;
mod sync;
mod token;
mod wire;

pub use auth::{AuthClient, AuthSession};
pub use client::KimClient;
pub use config::{
    http_origin_from_ws, ClientConfig, DEFAULT_CLIENT_USER_AGENT, DEFAULT_DEVICE,
    DEFAULT_LOCAL_HTTP_ORIGIN, DEFAULT_LOCAL_URL, DEFAULT_PROD_HTTP_ORIGIN, DEFAULT_PROD_URL,
};
pub use error::ClientError;
pub use events::{
    Event, HistoryItem, InboxItem, IncomingTalk, Message, MessageIndex, OutgoingContent, Profile,
    TalkResult,
};
pub use login::{login_on_conn, send_ping, wait_pong};
pub use session::MemorySession;
pub use supervisor::{LinkState, SessionEvent, SessionSupervisor};
pub use token::{account_from_token, token_unusable, unverified_claims, UnverifiedClaims};
pub use wire::{
    encode_ack, encode_ack_batch, encode_dest_cmd, encode_empty_cmd, encode_history,
    encode_inbox_list, encode_login, encode_offline_content, encode_offline_index, encode_outgoing,
    encode_ping, encode_user_image, encode_user_search, encode_user_talk, encode_user_talk_typed,
    encode_user_update, is_kickout,
};

pub use kim_protocol::{INBOX_KIND_GROUP, INBOX_KIND_USER};

#[cfg(test)]
mod tests;
