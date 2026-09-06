use async_trait::async_trait;
use futures::stream::BoxStream;
use kim_agent_llm::{LlmClient, LlmResult, ResponseRequest};
use kim_agent_tools::{ToolError, ToolRegistry};
use kim_agent_types::{Context, JsonValue, StreamEvent, ToolResult};
use std::sync::Arc;

#[async_trait]
pub trait Effects: Send + Sync {
    async fn complete_llm(
        &self,
        req: ResponseRequest,
        cx: &Context,
    ) -> LlmResult<BoxStream<'static, LlmResult<StreamEvent>>>;

    async fn call_tool(&self, name: &str, args: JsonValue) -> Result<ToolResult, ToolError>;
}

pub struct HarnessEffects {
    pub llm: Arc<dyn LlmClient>,
    pub tools: ToolRegistry,
}

#[async_trait]
impl Effects for HarnessEffects {
    async fn complete_llm(
        &self,
        req: ResponseRequest,
        cx: &Context,
    ) -> LlmResult<BoxStream<'static, LlmResult<StreamEvent>>> {
        self.llm.stream(req, cx).await
    }

    async fn call_tool(&self, name: &str, args: JsonValue) -> Result<ToolResult, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::Unknown(name.to_string()))?;
        tool.call(args).await
    }
}
