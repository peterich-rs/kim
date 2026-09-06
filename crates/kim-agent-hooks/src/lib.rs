//! before_tool / after_tool hooks.

use async_trait::async_trait;
use kim_agent_types::{JsonValue, ToolResult};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HookError {
    #[error("hook failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ToolInvocation {
    pub operation_id: String,
    pub invocation_id: String,
    pub name: String,
    pub arguments: JsonValue,
}

#[derive(Debug, Clone)]
pub enum BeforeToolDecision {
    Allow(JsonValue),
    Block { message: String },
}

#[async_trait]
pub trait Hooks: Send + Sync {
    async fn before_tool(&self, inv: &ToolInvocation) -> Result<BeforeToolDecision, HookError>;
    async fn after_tool(
        &self,
        inv: &ToolInvocation,
        result: &ToolResult,
    ) -> Result<ToolResult, HookError>;
}

pub struct NoopHooks;

#[async_trait]
impl Hooks for NoopHooks {
    async fn before_tool(&self, inv: &ToolInvocation) -> Result<BeforeToolDecision, HookError> {
        Ok(BeforeToolDecision::Allow(inv.arguments.clone()))
    }
    async fn after_tool(
        &self,
        _inv: &ToolInvocation,
        result: &ToolResult,
    ) -> Result<ToolResult, HookError> {
        Ok(result.clone())
    }
}
