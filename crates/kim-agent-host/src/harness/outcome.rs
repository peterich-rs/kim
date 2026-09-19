use goose_provider_types::conversation::message::{Message, MessageContent, MessageErrorKind};
use goose_provider_types::conversation::token_usage::ProviderUsage;
use goose_provider_types::conversation::Conversation;
use goose_provider_types::errors::ProviderError;
use rmcp::model::CallToolResult;

use crate::events::{ProviderFail, TurnOutcome, YieldKind};
use crate::ops::unanswered_tool_requests;
use crate::profile::ToolSet;
use crate::{session_pending, PendingYield};

pub const EMPTY_RESPONSE_MESSAGE: &str =
    "The model returned an empty response. Please resend your message to continue.";

#[derive(Debug, Clone)]
pub struct TurnVisibility {
    pub text: String,
    pub replied: bool,
    pub send_ok: bool,
    pub visible: bool,
}

#[derive(Debug)]
pub enum Classify {
    Yield(TurnOutcome),
    Finished(TurnVisibility),
    Recover(RecoverKind),
    Provider(ProviderFail),
    Failed(String),
}

#[derive(Debug)]
pub enum RecoverKind {
    Context,
    Truncated,
    RateLimit { wait: std::time::Duration },
    StripImages,
}

pub fn classify_from_conversation(
    conversation: &Conversation,
    tools: &ToolSet,
    probe: Option<ProviderError>,
    last_usage: Option<&ProviderUsage>,
) -> Classify {
    if let Some(first) = session_pending(conversation, tools).into_iter().next() {
        return Classify::Yield(yielded(first));
    }
    match goose_agent::operation::trailing_error(conversation) {
        Some(MessageErrorKind::ContextLengthExceeded) => {
            return Classify::Recover(RecoverKind::Context);
        }
        Some(_) if matches!(probe, Some(ProviderError::RateLimitExceeded { .. })) => {
            let wait = match probe {
                Some(ProviderError::RateLimitExceeded { retry_delay, .. }) => {
                    retry_delay.unwrap_or(std::time::Duration::from_secs(1))
                }
                _ => std::time::Duration::from_secs(1),
            };
            return Classify::Recover(RecoverKind::RateLimit { wait });
        }
        Some(MessageErrorKind::Other) if conversation_has_image(conversation) => {
            return Classify::Recover(RecoverKind::StripImages);
        }
        Some(kind) => {
            return Classify::Provider(match kind {
                MessageErrorKind::ContextLengthExceeded => ProviderFail::ContextExceeded,
                MessageErrorKind::Authentication => ProviderFail::Other("authentication".into()),
                MessageErrorKind::CreditsExhausted => {
                    ProviderFail::Other("credits exhausted".into())
                }
                MessageErrorKind::Other => ProviderFail::from_provider(
                    &probe.unwrap_or(ProviderError::ExecutionError("provider error".into())),
                ),
            });
        }
        None => {}
    }
    if output_limit_reached(conversation, last_usage) {
        return Classify::Recover(RecoverKind::Truncated);
    }
    if let Some(ProviderError::RateLimitExceeded { retry_delay, .. }) = probe {
        return Classify::Recover(RecoverKind::RateLimit {
            wait: retry_delay.unwrap_or(std::time::Duration::from_secs(1)),
        });
    }
    Classify::Finished(turn_visibility(conversation))
}

fn yielded(first: PendingYield) -> TurnOutcome {
    TurnOutcome::Yielded {
        kind: first.kind,
        call_id: first.call_id,
        name: first.name,
    }
}

pub fn turn_visibility(conversation: &Conversation) -> TurnVisibility {
    let text = visible_assistant_text(conversation);
    let replied = !text.trim().is_empty();
    let send_ok = successful_send_message(conversation);
    TurnVisibility {
        text,
        replied,
        send_ok,
        visible: replied || send_ok,
    }
}

fn visible_assistant_text(conversation: &Conversation) -> String {
    let Some(last) = conversation.messages().last() else {
        return String::new();
    };
    if last.role != rmcp::model::Role::Assistant || last.error_kind().is_some() {
        return String::new();
    }
    if last.content.iter().any(|block| {
        matches!(
            block,
            MessageContent::ToolRequest(_) | MessageContent::ActionRequired(_)
        )
    }) {
        return String::new();
    }
    let text = message_text(last);
    if text.trim() == EMPTY_RESPONSE_MESSAGE {
        return String::new();
    }
    if !last.is_user_visible() {
        return String::new();
    }
    text
}

pub fn successful_send_message(conversation: &Conversation) -> bool {
    let Ok(turn) = goose_agent::operation::messages_since_kickoff(conversation) else {
        return false;
    };
    let mut wanted = Vec::new();
    for message in turn {
        for block in &message.content {
            let MessageContent::ToolRequest(req) = block else {
                continue;
            };
            let Ok(call) = req.tool_call.as_ref() else {
                continue;
            };
            if call.name.as_ref() == "send_message" {
                wanted.push(req.id.as_str());
            }
        }
    }
    if wanted.is_empty() {
        return false;
    }
    for message in turn {
        for block in &message.content {
            let MessageContent::ToolResponse(res) = block else {
                continue;
            };
            if !wanted.iter().any(|id| *id == res.id.as_str()) {
                continue;
            }
            if response_ok(res) {
                return true;
            }
        }
    }
    false
}

fn response_ok(res: &goose_provider_types::conversation::message::ToolResponse) -> bool {
    let Ok(result) = res.tool_result.as_ref() else {
        return false;
    };
    structured_ok(result)
}

fn structured_ok(result: &CallToolResult) -> bool {
    result
        .structured_content
        .as_ref()
        .and_then(|value| value.get("ok"))
        .and_then(|v| v.as_bool())
        == Some(true)
}

pub fn conversation_has_image(conversation: &Conversation) -> bool {
    conversation.messages().iter().any(|message| {
        message
            .content
            .iter()
            .any(|block| matches!(block, MessageContent::Image(_)))
    })
}

pub fn output_limit_reached(
    conversation: &Conversation,
    last_usage: Option<&ProviderUsage>,
) -> bool {
    if conversation
        .messages()
        .iter()
        .any(|message| message.metadata.output_token_limit_reached)
    {
        return true;
    }
    last_usage
        .and_then(|usage| usage.finish_reasons.as_deref())
        .is_some_and(finish_reasons_truncated)
}

pub fn finish_reasons_truncated(reasons: &[String]) -> bool {
    reasons.iter().any(|reason| {
        let r = reason.to_ascii_lowercase();
        r.contains("length") || r.contains("max_tokens")
    })
}

fn message_text(message: &Message) -> String {
    let mut out = String::new();
    for block in &message.content {
        if let MessageContent::Text(text) = block {
            out.push_str(&text.text);
        }
    }
    out
}

pub fn strip_trailing_error(conversation: &Conversation) -> Conversation {
    let mut messages = conversation.messages().clone();
    if messages.last().and_then(Message::error_kind).is_some() {
        messages.pop();
    }
    Conversation::new_unvalidated(messages)
}

pub fn strip_empty_placeholder(conversation: &Conversation) -> Conversation {
    let mut messages = conversation.messages().clone();
    if let Some(last) = messages.last() {
        if message_text(last).trim() == EMPTY_RESPONSE_MESSAGE {
            messages.pop();
        }
    }
    Conversation::new_unvalidated(messages)
}

pub fn strip_images_keep_pairing(conversation: &Conversation) -> Conversation {
    let mut messages = conversation.messages().clone();
    for message in &mut messages {
        message
            .content
            .retain(|block| !matches!(block, MessageContent::Image(_)));
    }
    Conversation::new_unvalidated(messages)
}

pub fn clear_output_limit_flags(conversation: &Conversation) -> Conversation {
    let mut messages = conversation.messages().clone();
    for message in &mut messages {
        message.metadata.output_token_limit_reached = false;
    }
    Conversation::new_unvalidated(messages)
}

pub fn append_continue_hidden(conversation: &Conversation) -> Conversation {
    let mut messages = conversation.messages().clone();
    messages.push(
        Message::user()
            .with_text("Continue. The previous response was cut off by the output token limit.")
            .with_visibility(false, true),
    );
    Conversation::new_unvalidated(messages)
}

#[allow(dead_code)]
fn _yield_kind_used(kind: YieldKind) -> YieldKind {
    kind
}

#[allow(dead_code)]
fn _unanswered_used(conversation: &Conversation) -> usize {
    unanswered_tool_requests(conversation).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::CallToolRequestParams;

    fn send_call(id: &str) -> Message {
        let mut args = rmcp::model::JsonObject::new();
        args.insert("dest".into(), serde_json::json!("bob"));
        args.insert("text".into(), serde_json::json!("hi"));
        let mut call = CallToolRequestParams::new("send_message");
        call.arguments = Some(args);
        Message::assistant().with_tool_request(id.to_string(), Ok(call))
    }

    #[test]
    fn assistant_text_marks_replied() {
        let conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("hi"),
            Message::assistant().with_text("done"),
        ]);
        let vis = turn_visibility(&conv);
        assert!(vis.replied);
        assert!(vis.visible);
        assert!(!vis.send_ok);
    }

    #[test]
    fn empty_finish_is_not_replied_nor_visible() {
        let conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("hi"),
            Message::assistant(),
        ]);
        let vis = turn_visibility(&conv);
        assert!(!vis.replied);
        assert!(!vis.visible);
    }

    #[test]
    fn send_message_ok_json_is_visible_not_replied() {
        let conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("ping"),
            send_call("c1"),
            Message::user().with_tool_response(
                "c1".to_string(),
                Ok(CallToolResult::structured(serde_json::json!({"ok": true}))),
            ),
        ]);
        let vis = turn_visibility(&conv);
        assert!(!vis.replied);
        assert!(vis.send_ok);
        assert!(vis.visible);
    }

    #[test]
    fn send_message_ok_false_is_not_send_ok() {
        let conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("ping"),
            send_call("c1"),
            Message::user().with_tool_response(
                "c1".to_string(),
                Ok(CallToolResult::structured(serde_json::json!({"ok": false}))),
            ),
        ]);
        let vis = turn_visibility(&conv);
        assert!(!vis.send_ok);
        assert!(!vis.visible);
    }

    #[test]
    fn classify_uses_output_token_limit_reached() {
        let mut message = Message::assistant().with_text("partial");
        message.metadata.output_token_limit_reached = true;
        let conv = Conversation::new_unvalidated(vec![Message::user().with_text("go"), message]);
        let class = classify_from_conversation(&conv, &ToolSet::default(), None, None);
        assert!(matches!(class, Classify::Recover(RecoverKind::Truncated)));
    }

    #[test]
    fn classify_uses_finish_reasons_from_last_usage() {
        let conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("go"),
            Message::assistant().with_text("partial"),
        ]);
        let usage = ProviderUsage::new(
            "m".into(),
            goose_provider_types::conversation::token_usage::Usage::default(),
        )
        .with_finish_reasons(vec!["length".into()]);
        let class = classify_from_conversation(&conv, &ToolSet::default(), None, Some(&usage));
        assert!(matches!(class, Classify::Recover(RecoverKind::Truncated)));
    }

    #[test]
    fn classify_ignores_non_truncated_finish_reasons() {
        let conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("go"),
            Message::assistant().with_text("done"),
        ]);
        let usage = ProviderUsage::new(
            "m".into(),
            goose_provider_types::conversation::token_usage::Usage::default(),
        )
        .with_finish_reasons(vec!["stop".into()]);
        let class = classify_from_conversation(&conv, &ToolSet::default(), None, Some(&usage));
        assert!(matches!(class, Classify::Finished(_)));
    }

    #[test]
    fn empty_placeholder_is_not_a_reply() {
        let conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("hi"),
            Message::assistant().with_text(EMPTY_RESPONSE_MESSAGE),
        ]);
        let vis = turn_visibility(&conv);
        assert!(!vis.replied);
        assert!(!vis.visible);
    }
}
