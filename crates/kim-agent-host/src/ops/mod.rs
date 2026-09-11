use std::collections::HashSet;

use goose_agent::operation::messages_since_kickoff;
use goose_provider_types::conversation::message::{Message, MessageContent, ToolRequest};
use goose_provider_types::conversation::Conversation;

pub mod bash;
pub mod chat_guard;
pub mod compaction;
pub mod deferred_kim;
pub mod fs;
pub mod max_turns;
pub mod mcp;
pub mod permission;
pub mod system_prompt;
pub mod unknown_tool;

pub fn unanswered_tool_requests(conversation: &Conversation) -> Vec<ToolRequest> {
    let Ok(turn) = messages_since_kickoff(conversation) else {
        return Vec::new();
    };
    let answered: HashSet<&str> = turn
        .iter()
        .flat_map(Message::get_tool_response_ids)
        .collect();
    turn.iter()
        .flat_map(|message| message.content.iter())
        .filter_map(MessageContent::as_tool_request)
        .filter(|request| !answered.contains(request.id.as_str()))
        .cloned()
        .collect()
}
