use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::{applied, not_applicable, Operation, OperationResult};
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::Conversation;
use rmcp::model::{CallToolResult, ContentBlock};

use crate::events::HostEffect;
use crate::ops::unanswered_tool_requests;
use crate::HostSession;

pub struct UnknownToolOp;

#[async_trait]
impl Operation<HostSession, HostEffect> for UnknownToolOp {
    fn name(&self) -> &'static str {
        "unknown_tool"
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
            let name = request
                .tool_call
                .as_ref()
                .ok()
                .map(|call| call.name.to_string())
                .unwrap_or_else(|| "unknown".into());
            message.add_tool_response_with_metadata(
                request.id,
                Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                    "unknown tool {name}"
                ))])),
                None,
            );
        }
        applied([HostEffect::from(message)])
    }
}
