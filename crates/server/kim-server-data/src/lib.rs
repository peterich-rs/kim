//! IM data ports and their Memory / Postgres / Redis adapters.
//!
//! Account secrets and bot identity writes stay in Royal. This crate keeps the
//! public directory, messages, groups, social graph, id generation, replay
//! nonces, and agent spec records.

pub mod agent_spec;
pub mod directory;
pub mod hmac_nonce;
pub mod idgen;
pub mod social;
pub mod social_cache;
pub mod store;
pub mod users;
