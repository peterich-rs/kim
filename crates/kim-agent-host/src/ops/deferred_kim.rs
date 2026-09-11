use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::{not_applicable, yielded, Operation, OperationResult};
use goose_provider_types::conversation::Conversation;
use rmcp::model::{JsonObject, Tool};
use serde_json::{json, Value};

use crate::events::HostEffect;
use crate::ops::unanswered_tool_requests;
use crate::HostSession;

pub struct DeferredKimToolOp {
    pub names: Vec<String>,
}

fn schema(value: Value) -> Arc<JsonObject> {
    match value {
        Value::Object(map) => Arc::new(map),
        _ => Arc::new(JsonObject::new()),
    }
}

pub fn kim_tool_schema(name: &str) -> Option<Tool> {
    match name {
        "search_contacts" => Some(Tool::new(
            "search_contacts",
            "Search local friends and optionally the server user directory.",
            schema(json!({
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"],
                "additionalProperties": false
            })),
        )),
        "search_messages" => Some(Tool::new(
            "search_messages",
            "Search cached messages in a thread (current dest if omitted).",
            schema(json!({
                "type": "object",
                "properties": {
                    "dest": {"type": "string"},
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 50}
                },
                "required": ["query"],
                "additionalProperties": false
            })),
        )),
        "get_conversation_context" => Some(Tool::new(
            "get_conversation_context",
            "Return the latest N text messages of a thread for the model to read.",
            schema(json!({
                "type": "object",
                "properties": {
                    "dest": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 50}
                },
                "additionalProperties": false
            })),
        )),
        "list_profiles" => Some(Tool::new(
            "list_profiles",
            "List enabled local agent profiles (id, display_name). Cannot switch the current session.",
            schema(json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            })),
        )),
        "send_message" => Some(Tool::new(
            "send_message",
            "Send a text IM message to a KIM user who is already a friend. Never send to local agents.",
            schema(json!({
                "type": "object",
                "properties": {
                    "dest": {"type": "string"},
                    "text": {"type": "string"}
                },
                "required": ["dest", "text"],
                "additionalProperties": false
            })),
        )),
        "read_clipboard" => Some(Tool::new(
            "read_clipboard",
            "Read current text clipboard on this device.",
            schema(json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            })),
        )),
        _ => None,
    }
}

#[async_trait]
impl Operation<HostSession, HostEffect> for DeferredKimToolOp {
    fn name(&self) -> &'static str {
        "kim_tools"
    }

    async fn inference_tools(&self, _session: &HostSession) -> Result<Vec<Tool>> {
        Ok(self
            .names
            .iter()
            .filter_map(|n| kim_tool_schema(n))
            .collect())
    }

    async fn run(
        &self,
        _session: &HostSession,
        conversation: &Conversation,
        _emit: &goose_agent::operation::Emitter,
    ) -> Result<OperationResult<HostEffect>> {
        let pending = unanswered_tool_requests(conversation)
            .into_iter()
            .filter(|req| {
                req.tool_call
                    .as_ref()
                    .ok()
                    .is_some_and(|call| self.names.iter().any(|n| n == call.name.as_ref()))
            })
            .count();
        if pending == 0 {
            return not_applicable();
        }
        yielded()
    }
}
