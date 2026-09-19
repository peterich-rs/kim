use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use futures::stream;
use goose_provider_types::base::{MessageStream, Provider};
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::token_usage::{ProviderUsage, Usage};
use goose_provider_types::errors::ProviderError;
use goose_provider_types::model::ModelConfig;
use goose_providers::base::stream_from_single_message;
use rmcp::model::Tool;

use crate::ops::compaction::SUMMARIZE_SYSTEM;

pub struct ScriptedProvider {
    queue: Mutex<VecDeque<Message>>,
    errors: Mutex<VecDeque<ProviderError>>,
    summarize: Mutex<VecDeque<Message>>,
    context_limit: Option<usize>,
    hang: bool,
    hangs_left: AtomicU32,
    error_delay: AtomicU32,
}

impl ScriptedProvider {
    pub fn saying(text: &str) -> Self {
        Self::new(vec![Message::assistant().with_text(text)])
    }

    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            queue: Mutex::new(VecDeque::from(messages)),
            errors: Mutex::new(VecDeque::new()),
            summarize: Mutex::new(VecDeque::new()),
            context_limit: None,
            hang: false,
            hangs_left: AtomicU32::new(0),
            error_delay: AtomicU32::new(0),
        }
    }

    pub fn hanging() -> Self {
        let mut provider = Self::new(vec![]);
        provider.hang = true;
        provider
    }

    /// First `stream` call hangs; later calls use the queue (and [`Self::with_followup`]).
    pub fn hang_once() -> Self {
        let provider = Self::new(vec![]);
        provider.hangs_left.store(1, Ordering::Relaxed);
        provider
    }

    pub fn with_followup(self, message: Message) -> Self {
        self.queue
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push_back(message);
        self
    }

    pub fn with_context_limit(mut self, limit: usize) -> Self {
        self.context_limit = Some(limit);
        self
    }

    pub fn with_errors(self, errors: Vec<ProviderError>) -> Self {
        *self.errors.lock().unwrap_or_else(|err| err.into_inner()) = VecDeque::from(errors);
        self
    }

    /// Skip the error queue for the first `n` non-summarize `stream` calls.
    pub fn delay_errors(self, n: u32) -> Self {
        self.error_delay.store(n, Ordering::Relaxed);
        self
    }

    pub fn with_summarizer(self, messages: Vec<Message>) -> Self {
        *self.summarize.lock().unwrap_or_else(|err| err.into_inner()) = VecDeque::from(messages);
        self
    }

    pub fn fail(err: ProviderError) -> Self {
        Self::new(vec![]).with_errors(vec![err])
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

    fn pop_message(slot: &Mutex<VecDeque<Message>>) -> Option<Message> {
        slot.lock()
            .unwrap_or_else(|err| err.into_inner())
            .pop_front()
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
        system: &str,
        _messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        if self.hang || self.hangs_left.load(Ordering::Relaxed) > 0 {
            if self.hang || self.hangs_left.fetch_sub(1, Ordering::Relaxed) > 0 {
                let pending = stream::pending::<
                    Result<(Option<Message>, Option<ProviderUsage>), ProviderError>,
                >();
                return Ok(Box::pin(pending));
            }
        }
        if system == SUMMARIZE_SYSTEM {
            let next = Self::pop_message(&self.summarize)
                .unwrap_or_else(|| Message::assistant().with_text("kim.compaction.v1\nsummary"));
            let usage = ProviderUsage::new(model_config.model_name.clone(), Usage::default());
            return Ok(stream_from_single_message(next, usage));
        }
        if self.error_delay.load(Ordering::Relaxed) > 0 {
            self.error_delay.fetch_sub(1, Ordering::Relaxed);
        } else if let Some(err) = self
            .errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
        {
            return Err(err);
        }
        let next = Self::pop_message(&self.queue)
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
