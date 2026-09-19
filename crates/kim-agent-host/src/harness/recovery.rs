use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use goose_provider_types::base::{MessageStream, Provider};
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::token_usage::ProviderUsage;
use goose_provider_types::errors::ProviderError;
use goose_provider_types::model::ModelConfig;
use rmcp::model::Tool;

/// Records `stream`/`complete` errors so a 429 `retry_delay` survives goose swallowing the error.
pub struct ProviderProbe {
    inner: Arc<dyn Provider>,
    last: Mutex<Option<ProviderError>>,
}

impl ProviderProbe {
    pub fn wrap(inner: Arc<dyn Provider>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            last: Mutex::new(None),
        })
    }

    pub fn take_fail(&self) -> Option<ProviderError> {
        self.last.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

#[async_trait]
impl Provider for ProviderProbe {
    fn get_name(&self) -> &str {
        self.inner.get_name()
    }

    async fn stream(
        &self,
        model_config: &ModelConfig,
        system: &str,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        match self
            .inner
            .stream(model_config, system, messages, tools)
            .await
        {
            Ok(stream) => Ok(stream),
            Err(err) => {
                *self.last.lock().unwrap_or_else(|e| e.into_inner()) = Some(err.clone());
                Err(err)
            }
        }
    }

    async fn complete(
        &self,
        model_config: &ModelConfig,
        system: &str,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<(Message, ProviderUsage), ProviderError> {
        match self
            .inner
            .complete(model_config, system, messages, tools)
            .await
        {
            Ok(pair) => Ok(pair),
            Err(err) => {
                *self.last.lock().unwrap_or_else(|e| e.into_inner()) = Some(err.clone());
                Err(err)
            }
        }
    }

    async fn get_context_limit(&self, model: &str, override_limit: Option<usize>) -> usize {
        self.inner.get_context_limit(model, override_limit).await
    }
}
