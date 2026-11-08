use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::inference::InferenceRunner;
use goose_agent::machine::{EffectHandler, SessionLoader, StateMachine, Step};
use goose_agent::operation::{ConversationEffect, Emitter};
use goose_agent::tool::ToolProvider;
use goose_provider_types::base::Provider;
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::Conversation;
use goose_provider_types::model::ModelConfig;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, ErrorData, JsonObject, Tool,
};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::capability::interpolate_identity;
use crate::events::HostEffect;
use crate::ops::chat_guard::ChatGuardOp;
use crate::ops::max_turns::MaxTurnsOp;
use crate::ops::system_prompt::SystemPromptOp;
use crate::profile::builtin_templates;
use crate::HostSession;

/// Fallback when `profile_id` matches no builtin template.
pub(crate) const SUBAGENT_FALLBACK_PROMPT: &str = "\
You are an independent helper subagent. You run a focused, bounded task on behalf of a parent agent.

# Role
- Work independently on the assigned task only. Do not expand scope or start unrelated work.
- You are not the parent: do not send IM messages, do not talk to the user's contacts, and do not claim to be the parent agent.
- You have a limited turn budget (max 8 turns). Finish within that budget.

# How to work
- Prefer evidence from tools over guessing. Use only tools that appear in your tool list.
- Use tools efficiently: make the fewest calls that answer the task, and stop as soon as you have enough.
- If a tool call fails, change the approach. Do not retry the exact same call.
- Stay inside the task bounds. If something is blocked or missing, report that instead of improvising a larger plan.

# Report
- Lead with the conclusion.
- Follow with brief supporting detail (paths, quotes, or numbers) — no long narrative.
- When the task is done, say so explicitly. If you could not finish, say what is left and why.";

fn child_system_prompt(profile_id: &str) -> String {
    builtin_templates()
        .into_iter()
        .find(|p| p.id == profile_id)
        .map(|p| interpolate_identity(&p.system_prompt, &p))
        .unwrap_or_else(|| SUBAGENT_FALLBACK_PROMPT.to_string())
}

pub struct SubagentOp {
    pub provider: Arc<dyn Provider>,
    pub model: ModelConfig,
}

fn schema(value: Value) -> Arc<JsonObject> {
    match value {
        Value::Object(map) => Arc::new(map),
        _ => Arc::new(JsonObject::new()),
    }
}

fn error_result(msg: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(msg.into())])
}

struct ChildRuntime {
    store: Mutex<HashMap<String, Conversation>>,
}

#[async_trait]
impl SessionLoader<HostSession> for ChildRuntime {
    async fn load(&self, session_id: &str) -> Result<HostSession> {
        let store = self.store.lock().await;
        Ok(HostSession {
            id: session_id.to_string(),
            conversation: store
                .get(session_id)
                .cloned()
                .unwrap_or_else(Conversation::empty),
        })
    }
}

#[async_trait]
impl EffectHandler<HostSession, HostEffect> for ChildRuntime {
    async fn apply_effects(
        &self,
        session: &HostSession,
        effects: &mut [HostEffect],
        _emit: &Emitter,
    ) -> Result<()> {
        let mut store = self.store.lock().await;
        let conversation = store
            .entry(session.id.clone())
            .or_insert_with(Conversation::empty);
        for effect in effects.iter_mut() {
            match effect {
                HostEffect::Usage(_) => {}
                HostEffect::Conversation(ConversationEffect::AppendMessage(m)) => {
                    conversation.push(m.clone());
                }
                HostEffect::Conversation(ConversationEffect::ReplaceConversation(c)) => {
                    *conversation = c.clone();
                }
                HostEffect::Conversation(ConversationEffect::PatchToolRequestMeta { .. })
                | HostEffect::Conversation(ConversationEffect::SetMessageVisibility { .. }) => {}
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ToolProvider<HostSession> for SubagentOp {
    async fn tools(&self, _session: &HostSession) -> Result<Vec<Tool>> {
        Ok(vec![Tool::new(
            "delegate",
            "Run a smaller child agent (chat-only, no send_message) and return its reply.",
            schema(json!({
                "type": "object",
                "properties": {
                    "profile_id": {"type": "string"},
                    "task": {"type": "string"}
                },
                "required": ["task"],
                "additionalProperties": false
            })),
        )])
    }

    async fn call(
        &self,
        _session: &HostSession,
        _request_id: &str,
        call: CallToolRequestParams,
        _emit: &Emitter,
    ) -> Result<CallToolResult, ErrorData> {
        if call.name.as_ref() != "delegate" {
            return Ok(error_result("unknown tool"));
        }
        let args = call.arguments.unwrap_or_default();
        let task = args
            .get("task")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if task.is_empty() {
            return Ok(error_result("missing task"));
        }
        let profile_id = args
            .get("profile_id")
            .and_then(Value::as_str)
            .unwrap_or("translator");
        let child_prompt = child_system_prompt(profile_id);
        let runtime = ChildRuntime {
            store: Mutex::new(HashMap::new()),
        };
        {
            let mut store = runtime.store.lock().await;
            store.insert(
                "child".into(),
                Conversation::new_unvalidated([Message::user().with_text(task)]),
            );
        }
        let steps: Vec<Step<'static, HostSession, HostEffect>> = vec![
            Step::Operation(Arc::new(SystemPromptOp {
                prompt: child_prompt,
            })),
            Step::Operation(Arc::new(MaxTurnsOp { max: 8 })),
            Step::Operation(Arc::new(ChatGuardOp)),
            Step::Inference(Arc::new(InferenceRunner::new(
                Arc::clone(&self.provider),
                self.model.clone(),
            ))),
        ];
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let emit = Emitter::new(tx, CancellationToken::new());
        let machine = StateMachine::new(steps, CancellationToken::new());
        match machine.run(&runtime, "child", &emit).await {
            Ok(session) => {
                let text = session
                    .conversation
                    .messages()
                    .last()
                    .map(Message::as_concat_text)
                    .unwrap_or_default();
                Ok(CallToolResult::structured(
                    json!({"ok": true, "text": text}),
                ))
            }
            Err(err) => Ok(error_result(err.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_is_bounded_helper_without_send_message_tool() {
        let prompt = child_system_prompt("no-such-profile");
        assert_eq!(prompt, SUBAGENT_FALLBACK_PROMPT);
        assert!(prompt.contains("independent helper subagent"));
        assert!(prompt.contains("max 8 turns"));
        assert!(prompt.contains("Lead with the conclusion"));
        assert!(!prompt.contains("send_message"));
    }

    #[test]
    fn matching_template_uses_persona_prompt() {
        let prompt = child_system_prompt("translator");
        assert!(prompt.contains("译者"));
        assert_ne!(prompt, SUBAGENT_FALLBACK_PROMPT);
        assert!(!prompt.contains("send_message"));
    }

    #[test]
    fn goose_template_interpolates_identity_placeholders() {
        let prompt = child_system_prompt("goose");
        assert!(!prompt.contains("{display_name}"), "{prompt}");
        assert!(!prompt.contains("{model_name}"), "{prompt}");
        assert!(prompt.contains("助手"), "{prompt}");
        assert!(prompt.contains("# How you work"), "{prompt}");
        assert!(!prompt.contains("send_message"), "{prompt}");
    }
}
