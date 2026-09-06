//! Agent harness: accept / drive / abort / resume (Phase 0).

mod effects;
mod harness;

pub use effects::{Effects, HarnessEffects};
pub use harness::*;
pub use kim_agent_llm::LlmError;
