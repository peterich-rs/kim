use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::{
    applied, messages_since_kickoff, not_applicable, ConversationEffect, Operation, OperationResult,
};
use goose_provider_types::base::Provider;
use goose_provider_types::conversation::message::{Message, MessageContent};
use goose_provider_types::conversation::Conversation;
use goose_provider_types::model::ModelConfig;

use crate::events::{HostEffect, HostError, ProviderFail};
use crate::HostSession;

pub const SUMMARIZE_SYSTEM: &str = "\
You summarize earlier conversation turns for KIM compaction. \
Preserve user intent, decisions, and tool call ids. Reply with plain text only.";

const MARK: &str = "kim.compaction.v1\n";

pub struct CompactionOp {
    pub provider: Arc<dyn Provider>,
    pub model: ModelConfig,
    pub tool_result_bytes: usize,
    pub input_tokens: Arc<AtomicU64>,
}

fn char_count(conversation: &Conversation) -> usize {
    conversation
        .messages()
        .iter()
        .map(Message::as_concat_text)
        .map(|text| text.len())
        .sum()
}

fn estimate_tokens(conversation: &Conversation, input_tokens: &AtomicU64) -> usize {
    let usage = input_tokens.load(Ordering::Relaxed);
    if usage > 0 {
        return usage as usize;
    }
    char_count(conversation) / 4
}

fn summarize_config(model_name: &str) -> ModelConfig {
    let mut cfg = ModelConfig::new(model_name);
    cfg.max_tokens = Some(1024);
    cfg.temperature = Some(0.2);
    cfg
}

fn fallback_summary(older: &[Message]) -> String {
    let mut buf = String::from(MARK);
    for message in older {
        let text = message.as_concat_text();
        if text.is_empty() {
            continue;
        }
        let snippet: String = text.chars().take(512).collect();
        buf.push_str(&snippet);
        buf.push('\n');
        if buf.len() > 4 * 1024 {
            break;
        }
    }
    buf
}

fn split_older<'a>(conversation: &'a Conversation) -> (&'a [Message], &'a [Message]) {
    let messages = conversation.messages();
    let kickoff = messages_since_kickoff(conversation)
        .map(|turn| turn.len())
        .unwrap_or(0);
    let split = messages.len().saturating_sub(kickoff);
    (&messages[..split], &messages[split..])
}

fn already_compacted(older: &[Message]) -> bool {
    !older.is_empty()
        && older
            .iter()
            .all(|message| !message.is_user_visible() && message.as_concat_text().starts_with(MARK))
}

impl CompactionOp {
    async fn summarize(&self, older: &[Message]) -> String {
        let cfg = summarize_config(&self.model.model_name);
        let user = Message::user().with_text(fallback_summary(older));
        match self
            .provider
            .complete(&cfg, SUMMARIZE_SYSTEM, std::slice::from_ref(&user), &[])
            .await
        {
            Ok((message, _)) => {
                let text = message.as_concat_text();
                if text.trim().is_empty() {
                    fallback_summary(older)
                } else if text.starts_with(MARK) {
                    text
                } else {
                    format!("{MARK}{text}")
                }
            }
            Err(_) => fallback_summary(older),
        }
    }

    fn replacement(summary: String, recent: &[Message]) -> Conversation {
        let mut next = vec![Message::user()
            .with_text(summary)
            .with_visibility(false, true)];
        next.extend(recent.iter().cloned());
        Conversation::new_unvalidated(next)
    }

    /// Ignore the 70% threshold. Older empty → shrink this turn's tool results.
    pub async fn force(&self, conversation: &Conversation) -> Result<Conversation, HostError> {
        let (older, recent) = split_older(conversation);
        if !older.is_empty() {
            let summary = self.summarize(older).await;
            tracing::info!(
                tokens_before = estimate_tokens(conversation, &self.input_tokens),
                source = "context_400",
                "harness.compaction"
            );
            return Ok(Self::replacement(summary, recent));
        }
        match shrink_current_turn(conversation, self.tool_result_bytes) {
            Some(next) => {
                tracing::info!(source = "context_400", "harness.compaction");
                Ok(next)
            }
            None => Err(HostError::Provider(ProviderFail::ContextExceeded)),
        }
    }
}

fn shrink_current_turn(conversation: &Conversation, cap: usize) -> Option<Conversation> {
    let mut messages = conversation.messages().clone();
    let mut changed = false;
    for message in &mut messages {
        for block in &mut message.content {
            if let MessageContent::Image(_) = block {
                changed = true;
            }
        }
        message
            .content
            .retain(|block| !matches!(block, MessageContent::Image(_)));
        for block in &mut message.content {
            let MessageContent::ToolResponse(res) = block else {
                continue;
            };
            let Ok(result) = res.tool_result.as_mut() else {
                continue;
            };
            if let Some(value) = result.structured_content.as_mut() {
                let rendered = value.to_string();
                if rendered.len() > cap {
                    *value = serde_json::json!({
                        "ok": false,
                        "output": ellipsize(&rendered, cap),
                    });
                    changed = true;
                }
            }
        }
    }
    if !changed {
        return None;
    }
    Some(Conversation::new_unvalidated(messages))
}

fn ellipsize(text: &str, cap: usize) -> String {
    if text.len() <= cap {
        return text.to_string();
    }
    let marker = " … ";
    let keep = cap.saturating_sub(marker.len()) / 2;
    let mut head = keep;
    while head > 0 && !text.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = text.len().saturating_sub(keep);
    while tail < text.len() && !text.is_char_boundary(tail) {
        tail += 1;
    }
    format!("{}{marker}{}", &text[..head], &text[tail..])
}

#[async_trait]
impl Operation<HostSession, HostEffect> for CompactionOp {
    fn name(&self) -> &'static str {
        "compaction"
    }

    async fn run(
        &self,
        _session: &HostSession,
        conversation: &Conversation,
        _emit: &goose_agent::operation::Emitter,
    ) -> Result<OperationResult<HostEffect>> {
        let limit = self
            .provider
            .get_context_limit(&self.model.model_name, self.model.context_limit)
            .await;
        let threshold = limit.saturating_mul(70) / 100;
        let tokens = estimate_tokens(conversation, &self.input_tokens);
        if threshold == 0 || tokens <= threshold {
            return not_applicable();
        }
        let (older, recent) = split_older(conversation);
        if older.is_empty() || already_compacted(older) {
            return not_applicable();
        }
        let summary = self.summarize(older).await;
        tracing::info!(
            tokens_before = tokens,
            source = "proactive",
            "harness.compaction"
        );
        let replacement = Self::replacement(summary, recent);
        applied([HostEffect::Conversation(
            ConversationEffect::ReplaceConversation(replacement),
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goose_provider_types::conversation::message::Message;

    #[test]
    fn force_compact_shrinks_current_turn_tool_results_when_older_empty() {
        let fat = "x".repeat(8_000);
        let conv = Conversation::new_unvalidated(vec![
            Message::user().with_text("go"),
            Message::user().with_tool_response(
                "c1".to_string(),
                Ok(rmcp::model::CallToolResult::structured(serde_json::json!({
                    "ok": true,
                    "output": fat,
                }))),
            ),
        ]);
        let shrunk = shrink_current_turn(&conv, 200).expect("shrunk");
        let text = shrunk
            .messages()
            .iter()
            .map(Message::as_concat_text)
            .collect::<String>();
        assert!(text.len() < fat.len());
    }

    #[test]
    fn force_noop_when_nothing_to_shrink() {
        let conv = Conversation::new_unvalidated(vec![Message::user().with_text("only")]);
        assert!(shrink_current_turn(&conv, 200).is_none());
    }
}
