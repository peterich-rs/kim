use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::Operation;
use goose_provider_types::conversation::Conversation;

use crate::events::HostEffect;
use crate::HostSession;

pub struct SystemPromptOp {
    pub prompt: String,
}

#[async_trait]
impl Operation<HostSession, HostEffect> for SystemPromptOp {
    fn name(&self) -> &'static str {
        "system_prompt"
    }

    async fn prompt_parts(
        &self,
        _session: &HostSession,
        _conversation: &Conversation,
    ) -> Result<Vec<(String, String)>> {
        Ok(vec![("system".into(), self.prompt.clone())])
    }
}
