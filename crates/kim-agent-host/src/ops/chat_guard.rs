use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::{applied, not_applicable, Operation, OperationResult};
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::Conversation;
use rmcp::model::{CallToolResult, ContentBlock};

use crate::events::HostEffect;
use crate::ops::unanswered_tool_requests;
use crate::HostSession;

pub struct ChatGuardOp;

#[async_trait]
impl Operation<HostSession, HostEffect> for ChatGuardOp {
    fn name(&self) -> &'static str {
        "chat_guard"
    }

    async fn run(
        &self,
        _session: &HostSession,
        conversation: &Conversation,
        _emit: &goose_agent::operation::Emitter,
    ) -> Result<OperationResult<HostEffect>> {
        let pending = unanswered_tool_requests(conversation);
        if pending.is_empty() {
            return not_applicable();
        }
        let mut message = Message::user();
        for request in pending {
            message.add_tool_response_with_metadata(
                request.id,
                Ok(CallToolResult::error(vec![ContentBlock::text(
                    "tools are not enabled in this chat",
                )])),
                None,
            );
        }
        applied([HostEffect::from(message)])
    }
}
