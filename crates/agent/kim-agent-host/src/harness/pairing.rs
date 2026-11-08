use goose_provider_types::conversation::message::{Message, MessageContent};
use goose_provider_types::conversation::Conversation;
use goose_provider_types::permission::Permission;

use crate::ops::permission::unanswered_confirmations;
use crate::ops::unanswered_tool_requests;

/// Pair every unanswered tool request and confirmation. Returns (tools, confirmations).
pub fn repair_conversation(conversation: &mut Conversation) -> (usize, usize) {
    let confirmations = unanswered_confirmations(conversation);
    let pending = unanswered_tool_requests(conversation);
    let n_confirmations = confirmations.len();
    let n_tools = pending.len();
    if n_confirmations == 0 && n_tools == 0 {
        return (0, 0);
    }
    let mut message = Message::user();
    for item in &confirmations {
        message = message.with_content(MessageContent::action_required_tool_confirmation_response(
            item.call_id.clone(),
            Permission::Cancel,
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for item in confirmations
        .into_iter()
        .chain(pending.into_iter().map(|req| crate::PendingYield {
            call_id: req.id,
            name: String::new(),
            arguments_json: String::new(),
            kind: crate::YieldKind::ToolRequest,
            prompt: String::new(),
        }))
    {
        if !seen.insert(item.call_id.clone()) {
            continue;
        }
        message.add_tool_response_with_metadata(
            item.call_id,
            Ok(rmcp::model::CallToolResult::error(vec![
                rmcp::model::ContentBlock::text("cancelled"),
            ])),
            None,
        );
    }
    conversation.push(message);
    tracing::info!(n_tools, n_confirmations, "harness.pairing_repair");
    (n_tools, n_confirmations)
}

/// Pair unanswered fs/bash/MCP (and broken) tool calls. Leave kim-world tools
/// and confirmation cards for `resume()` replay.
pub fn repair_in_process(conversation: &mut Conversation, yield_names: &[&str]) -> usize {
    let pending = unanswered_tool_requests(conversation);
    let mut ids = Vec::new();
    for req in pending {
        let in_process = match req.tool_call.as_ref() {
            Ok(call) => !yield_names.iter().any(|name| *name == call.name.as_ref()),
            Err(_) => true,
        };
        if in_process {
            ids.push(req.id);
        }
    }
    if ids.is_empty() {
        return 0;
    }
    let n = ids.len();
    let mut message = Message::user();
    for id in ids {
        message.add_tool_response_with_metadata(
            id,
            Ok(rmcp::model::CallToolResult::error(vec![
                rmcp::model::ContentBlock::text("cancelled"),
            ])),
            None,
        );
    }
    conversation.push(message);
    tracing::info!(n_tools = n, "harness.pairing_repair_in_process");
    n
}

#[cfg(test)]
pub fn history_valid(conversation: &Conversation) -> bool {
    unanswered_tool_requests(conversation).is_empty()
        && unanswered_confirmations(conversation).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::CallToolRequestParams;

    fn call(name: &'static str) -> CallToolRequestParams {
        let mut args = rmcp::model::JsonObject::new();
        args.insert("q".into(), serde_json::json!("x"));
        let mut call = CallToolRequestParams::new(name);
        call.arguments = Some(args);
        call
    }

    #[test]
    fn repair_in_process_pairs_bash_keeps_kim_yield() {
        let mut conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("go"),
            Message::assistant()
                .with_tool_request("b1".to_string(), Ok(call("bash")))
                .with_tool_request("k1".to_string(), Ok(call("search_contacts"))),
        ]);
        let n = repair_in_process(&mut conv, &["search_contacts"]);
        assert_eq!(n, 1);
        let pending = unanswered_tool_requests(&conv);
        assert_eq!(pending.len(), 1);
        let tool = pending[0].tool_call.as_ref().expect("call");
        assert_eq!(tool.name.as_ref(), "search_contacts");
        assert!(!history_valid(&conv));
    }

    #[test]
    fn repair_in_process_pairs_all_when_no_yield_names() {
        let mut conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("go"),
            Message::assistant().with_tool_request("b1".to_string(), Ok(call("bash"))),
        ]);
        let n = repair_in_process(&mut conv, &[]);
        assert_eq!(n, 1);
        assert!(history_valid(&conv));
    }
}
