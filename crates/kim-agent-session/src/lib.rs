//! Conversation tree, context projection, AGENTS.md / `.agents` discovery.

mod agents;
mod tree;

pub use agents::{discover_agents_context, AgentsDiscovery, DEFAULT_AGENTS_CHAR_CAP};
pub use tree::{SessionError, SessionFacade};
