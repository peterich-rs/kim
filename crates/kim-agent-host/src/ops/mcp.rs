use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::Emitter;
use goose_agent::tool::ToolProvider;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock, ErrorData, Tool};
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt};
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::profile::ExtensionSpec;
use crate::{HostError, HostSession};

struct ConnectedExt {
    name: String,
    client: rmcp::service::RunningService<RoleClient, ()>,
}

pub struct McpHub {
    clients: Mutex<Vec<ConnectedExt>>,
}

impl McpHub {
    pub fn new() -> Self {
        Self {
            clients: Mutex::new(Vec::new()),
        }
    }

    pub async fn connect(&self, extensions: &[ExtensionSpec], cwd: &Path) -> Result<(), HostError> {
        let mut connected = Vec::new();
        for ext in extensions {
            if ext.command.is_empty() {
                return Err(HostError::Failed(format!(
                    "mcp extension {} has empty command",
                    ext.name
                )));
            }
            let transport = ext.transport.trim();
            if !transport.is_empty() && transport != "stdio" {
                return Err(HostError::Failed(format!(
                    "mcp transport {transport} is not supported"
                )));
            }
            let mut cmd = Command::new(&ext.command[0]);
            cmd.args(&ext.command[1..]).current_dir(cwd);
            let child = TokioChildProcess::new(cmd)
                .map_err(|e| HostError::Failed(format!("mcp {}: {e}", ext.name)))?;
            let client = ()
                .serve(child)
                .await
                .map_err(|e| HostError::Failed(format!("mcp {}: {e}", ext.name)))?;
            connected.push(ConnectedExt {
                name: ext.name.clone(),
                client,
            });
        }
        *self.clients.lock().await = connected;
        Ok(())
    }

    pub async fn disconnect(&self) {
        let mut clients = self.clients.lock().await;
        for mut ext in clients.drain(..) {
            let _ = ext.client.close().await;
        }
    }
}

pub struct McpToolProvider {
    pub hub: Arc<McpHub>,
}

fn prefixed(ext: &str, tool: &str) -> String {
    format!("{ext}__{tool}")
}

fn split_prefixed(name: &str) -> Option<(&str, &str)> {
    name.split_once("__")
}

#[async_trait]
impl ToolProvider<HostSession> for McpToolProvider {
    async fn tools(&self, _session: &HostSession) -> Result<Vec<Tool>> {
        let clients = self.hub.clients.lock().await;
        let mut out = Vec::new();
        for ext in clients.iter() {
            let listed = ext.client.list_all_tools().await.unwrap_or_default();
            for mut tool in listed {
                tool.name = Cow::Owned(prefixed(&ext.name, tool.name.as_ref()));
                out.push(tool);
            }
        }
        Ok(out)
    }

    async fn call(
        &self,
        _session: &HostSession,
        _request_id: &str,
        mut call: CallToolRequestParams,
        _emit: &Emitter,
    ) -> Result<CallToolResult, ErrorData> {
        let Some((ext_name, tool_name)) = split_prefixed(call.name.as_ref()) else {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "unknown mcp tool {}",
                call.name
            ))]));
        };
        let clients = self.hub.clients.lock().await;
        let Some(ext) = clients.iter().find(|c| c.name == ext_name) else {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "unknown mcp extension {ext_name}"
            ))]));
        };
        call.name = Cow::Owned(tool_name.to_string());
        match ext.client.call_tool(call).await {
            Ok(result) => Ok(result),
            Err(err) => Ok(CallToolResult::error(vec![ContentBlock::text(
                err.to_string(),
            )])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_and_splits() {
        assert_eq!(prefixed("github", "search"), "github__search");
        assert_eq!(split_prefixed("github__search"), Some(("github", "search")));
        assert_eq!(
            split_prefixed("github__search__more"),
            Some(("github", "search__more"))
        );
        assert_eq!(split_prefixed("noseparator"), None);
    }

    #[tokio::test]
    async fn missing_binary_is_failed() {
        let hub = McpHub::new();
        let spec = ExtensionSpec {
            name: "x".into(),
            transport: "stdio".into(),
            command: vec!["__kim_mcp_missing_bin__".into()],
            url: String::new(),
        };
        let err = hub
            .connect(&[spec], Path::new("/tmp"))
            .await
            .expect_err("missing bin");
        assert!(matches!(err, HostError::Failed(_)));
    }
}
