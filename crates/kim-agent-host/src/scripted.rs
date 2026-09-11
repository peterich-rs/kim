use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use goose_provider_types::base::{MessageStream, Provider};
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::token_usage::{ProviderUsage, Usage};
use goose_provider_types::errors::ProviderError;
use goose_provider_types::model::ModelConfig;
use goose_providers::base::stream_from_single_message;
use rmcp::model::Tool;

pub struct ScriptedProvider {
    queue: Mutex<VecDeque<Message>>,
    context_limit: Option<usize>,
}

impl ScriptedProvider {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            queue: Mutex::new(VecDeque::from(messages)),
            context_limit: None,
        }
    }

    pub fn with_context_limit(mut self, limit: usize) -> Self {
        self.context_limit = Some(limit);
        self
    }

    pub fn kim_search_contacts(calls: &[(&str, &str)]) -> Self {
        let mut message = Message::assistant();
        for (id, query) in calls {
            let mut args = rmcp::model::JsonObject::new();
            args.insert("query".into(), serde_json::json!(query));
            let mut call = rmcp::model::CallToolRequestParams::new("search_contacts");
            call.arguments = Some(args);
            message = message.with_tool_request((*id).to_string(), Ok(call));
        }
        Self::new(vec![message])
    }

    pub fn kim_send_message(id: &str, dest: &str, text: &str) -> Self {
        let mut args = rmcp::model::JsonObject::new();
        args.insert("dest".into(), serde_json::json!(dest));
        args.insert("text".into(), serde_json::json!(text));
        let mut call = rmcp::model::CallToolRequestParams::new("send_message");
        call.arguments = Some(args);
        Self::new(vec![
            Message::assistant().with_tool_request(id.to_string(), Ok(call))
        ])
    }
}

#[async_trait]
impl Provider for ScriptedProvider {
    fn get_name(&self) -> &str {
        "scripted"
    }

    async fn stream(
        &self,
        model_config: &ModelConfig,
        _system: &str,
        _messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        let next = match self.queue.lock() {
            Ok(mut q) => q.pop_front(),
            Err(poisoned) => poisoned.into_inner().pop_front(),
        }
        .unwrap_or_else(|| Message::assistant().with_text("scripted: empty queue"));
        let usage = ProviderUsage::new(model_config.model_name.clone(), Usage::default());
        Ok(stream_from_single_message(next, usage))
    }

    async fn get_context_limit(&self, _model: &str, override_limit: Option<usize>) -> usize {
        if let Some(n) = override_limit {
            return n;
        }
        if let Some(n) = self.context_limit {
            return n;
        }
        128_000
    }
}
