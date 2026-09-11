use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::{
    applied, messages_since_kickoff, not_applicable, ConversationEffect, Operation, OperationResult,
};
use goose_provider_types::base::Provider;
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::Conversation;

use crate::events::HostEffect;
use crate::HostSession;

pub struct CompactionOp {
    pub provider: Arc<dyn Provider>,
    pub model: String,
}

fn char_count(conversation: &Conversation) -> usize {
    conversation
        .messages()
        .iter()
        .map(Message::as_concat_text)
        .map(|t| t.len())
        .sum()
}

fn summarize(older: &[Message]) -> String {
    let mut buf = String::from("(conversation summary)\n");
    for message in older {
        let text = message.as_concat_text();
        if text.is_empty() {
            continue;
        }
        let snippet: String = text.chars().take(80).collect();
        buf.push_str(&snippet);
        buf.push('\n');
        if buf.len() > 512 {
            break;
        }
    }
    buf
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
        let limit = self.provider.get_context_limit(&self.model, None).await;
        let threshold = limit.saturating_mul(70) / 100;
        let tokens = char_count(conversation) / 4;
        if threshold == 0 || tokens <= threshold {
            return not_applicable();
        }
        let messages = conversation.messages();
        let kickoff = messages_since_kickoff(conversation)
            .map(|m| m.len())
            .unwrap_or(0);
        let split = messages.len().saturating_sub(kickoff);
        let older = &messages[..split];
        let recent = &messages[split..];
        if older.is_empty() {
            return not_applicable();
        }
        // Already compacted this history; applying again would loop.
        if older.iter().all(|m| {
            !m.is_user_visible() && m.as_concat_text().starts_with("(conversation summary)")
        }) {
            return not_applicable();
        }
        let mut next = vec![Message::user()
            .with_text(summarize(older))
            .with_visibility(false, true)];
        next.extend(recent.iter().cloned());
        let replacement = Conversation::new_unvalidated(next);
        applied([HostEffect::Conversation(
            ConversationEffect::ReplaceConversation(replacement),
        )])
    }
}
