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
}

impl ScriptedProvider {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            queue: Mutex::new(VecDeque::from(messages)),
        }
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
}
