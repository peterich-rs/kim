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

use crate::events::HostEffect;
use crate::ops::chat_guard::ChatGuardOp;
use crate::ops::max_turns::MaxTurnsOp;
use crate::ops::system_prompt::SystemPromptOp;
use crate::profile::builtin_templates;
use crate::HostSession;

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
        let child_prompt = builtin_templates()
            .into_iter()
            .find(|p| p.id == profile_id)
            .map(|p| p.system_prompt)
            .unwrap_or_else(|| {
                "You are a helper subagent. Do not send messages. Answer the task.".into()
            });
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
