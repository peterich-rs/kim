use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::{
    assistant_turn_count, messages_since_kickoff, not_applicable, yielded_with, Operation,
    OperationResult,
};
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::Conversation;

use crate::events::HostEffect;
use crate::HostSession;

pub struct MaxTurnsOp {
    pub max: u32,
}

#[async_trait]
impl Operation<HostSession, HostEffect> for MaxTurnsOp {
    fn name(&self) -> &'static str {
        "max_turns"
    }

    async fn run(
        &self,
        _session: &HostSession,
        conversation: &Conversation,
        _emit: &goose_agent::operation::Emitter,
    ) -> Result<OperationResult<HostEffect>> {
        let Ok(turn) = messages_since_kickoff(conversation) else {
            return not_applicable();
        };
        if assistant_turn_count(turn) < self.max {
            return not_applicable();
        }
        let message =
            Message::assistant().with_text(format!("Stopped after {} assistant turns.", self.max));
        yielded_with([HostEffect::from(message)])
    }
}
