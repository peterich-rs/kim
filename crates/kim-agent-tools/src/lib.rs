//! Tool trait, registry, and scripted tools for Phase 0.

mod fs_tools;

pub use fs_tools::{register_fs_tools, BashTool, EditTool, ReadTool, WriteTool};

use async_trait::async_trait;
use kim_agent_types::{JsonValue, ReplayPolicy, ToolResult, ToolSchema};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("unknown tool: {0}")]
    Unknown(String),
    #[error("tool failed: {0}")]
    Failed(String),
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> JsonValue;
    fn replay_policy(&self) -> ReplayPolicy {
        ReplayPolicy::Never
    }
    async fn call(&self, args: JsonValue) -> Result<ToolResult, ToolError>;

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
        }
    }
}

#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn schemas(&self) -> Vec<ToolSchema> {
        let mut names: Vec<_> = self.tools.keys().cloned().collect();
        names.sort();
        names
            .into_iter()
            .filter_map(|n| self.tools.get(&n).map(|t| t.schema()))
            .collect()
    }
}

/// Scripted tool that returns canned outputs (and counts calls for crash tests).
pub struct ScriptedTool {
    name: String,
    description: String,
    policy: ReplayPolicy,
    outputs: std::sync::Mutex<Vec<ToolResult>>,
    pub call_count: std::sync::atomic::AtomicUsize,
}

impl ScriptedTool {
    pub fn new(
        name: impl Into<String>,
        outputs: Vec<ToolResult>,
        policy: ReplayPolicy,
    ) -> Arc<Self> {
        Arc::new(Self {
            name: name.into(),
            description: "scripted tool".into(),
            policy,
            outputs: std::sync::Mutex::new(outputs),
            call_count: std::sync::atomic::AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl Tool for ScriptedTool {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({"type":"object","properties":{}})
    }
    fn replay_policy(&self) -> ReplayPolicy {
        self.policy
    }
    async fn call(&self, _args: JsonValue) -> Result<ToolResult, ToolError> {
        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut g = self
            .outputs
            .lock()
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        if g.is_empty() {
            Ok(ToolResult {
                output: "scripted: no more outputs".into(),
                is_error: true,
            })
        } else {
            Ok(g.remove(0))
        }
    }
}
