use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::Emitter;
use goose_agent::tool::ToolProvider;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock, ErrorData, Tool};
use rmcp::{RoleClient, ServiceExt};
use tokio::sync::Mutex;

use crate::harness::{kill_child_process, prepare};
use crate::profile::ExtensionSpec;
use crate::{HostError, HostSession};

struct Bounds {
    tool_timeout: Duration,
    max_servers: usize,
    max_tools: usize,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            tool_timeout: Duration::from_secs(120),
            max_servers: 16,
            max_tools: 128,
        }
    }
}

enum SlotState {
    Live {
        client: rmcp::service::RunningService<RoleClient, ()>,
        child: Option<tokio::process::Child>,
        pgid: Option<u32>,
    },
    Dead {
        until: Instant,
    },
}

struct ServerSlot {
    name: String,
    spec: ExtensionSpec,
    cwd: PathBuf,
    state: SlotState,
}

pub struct McpHub {
    clients: Mutex<Vec<ServerSlot>>,
    bounds: std::sync::Mutex<Bounds>,
}

impl McpHub {
    pub fn new() -> Self {
        Self {
            clients: Mutex::new(Vec::new()),
            bounds: std::sync::Mutex::new(Bounds::default()),
        }
    }

    pub fn set_bounds(&self, tool_timeout: Duration, max_servers: usize, max_tools: usize) {
        if let Ok(mut bounds) = self.bounds.lock() {
            bounds.tool_timeout = tool_timeout;
            bounds.max_servers = max_servers;
            bounds.max_tools = max_tools;
        }
    }

    fn bounds(&self) -> Bounds {
        self.bounds
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone_bounds()
    }

    pub async fn connect(&self, extensions: &[ExtensionSpec], cwd: &Path) -> Result<(), HostError> {
        let bounds = self.bounds();
        if extensions.len() > bounds.max_servers {
            return Err(HostError::Failed(format!(
                "mcp server cap {} exceeded",
                bounds.max_servers
            )));
        }
        let mut connected = Vec::new();
        for ext in extensions {
            connected.push(spawn_slot(ext, cwd).await?);
        }
        *self.clients.lock().await = connected;
        Ok(())
    }

    pub async fn disconnect(&self) {
        let drained: Vec<ServerSlot> = {
            let mut clients = self.clients.lock().await;
            clients.drain(..).collect()
        };
        for mut slot in drained {
            kill_slot(&mut slot);
            if let SlotState::Live { mut client, .. } = slot.state {
                let _ = client.close().await;
            }
        }
    }
}

fn kill_slot(slot: &mut ServerSlot) {
    if let SlotState::Live { pgid, child, .. } = &mut slot.state {
        kill_child_process(*pgid, child.as_mut());
    }
}

fn poison_slot(slot: &mut ServerSlot, reason: &str) {
    tracing::info!(name = %slot.name, reason, "harness.mcp_poison");
    kill_slot(slot);
    slot.state = SlotState::Dead {
        until: Instant::now() + Duration::from_secs(5),
    };
}

async fn spawn_slot(ext: &ExtensionSpec, cwd: &Path) -> Result<ServerSlot, HostError> {
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
    let mut cmd = prepare(&ext.command[0]);
    cmd.args(&ext.command[1..])
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let mut child = cmd
        .spawn()
        .map_err(|err| HostError::Failed(format!("mcp {}: {err}", ext.name)))?;
    let pgid = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| HostError::Failed(format!("mcp {}: stdout was not piped", ext.name)))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| HostError::Failed(format!("mcp {}: stdin was not piped", ext.name)))?;
    let transport =
        rmcp::transport::async_rw::AsyncRwTransport::<RoleClient, _, _>::new(stdout, stdin);
    let client = ()
        .serve(transport)
        .await
        .map_err(|err| HostError::Failed(format!("mcp {}: {err}", ext.name)))?;
    Ok(ServerSlot {
        name: ext.name.clone(),
        spec: ext.clone(),
        cwd: cwd.to_path_buf(),
        state: SlotState::Live {
            client,
            child: Some(child),
            pgid,
        },
    })
}

impl Bounds {
    fn clone_bounds(&self) -> Self {
        Self {
            tool_timeout: self.tool_timeout,
            max_servers: self.max_servers,
            max_tools: self.max_tools,
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
        let max_tools = self.hub.bounds().max_tools;
        let peers: Vec<(String, rmcp::service::Peer<RoleClient>)> = {
            let clients = self.hub.clients.lock().await;
            clients
                .iter()
                .filter_map(|slot| match &slot.state {
                    SlotState::Live { client, .. } => {
                        Some((slot.name.clone(), client.peer().clone()))
                    }
                    SlotState::Dead { .. } => None,
                })
                .collect()
        };
        let mut out = Vec::new();
        for (name, peer) in peers {
            if out.len() >= max_tools {
                break;
            }
            let listed = peer.list_all_tools().await.unwrap_or_default();
            for mut tool in listed {
                if out.len() >= max_tools {
                    break;
                }
                tool.name = Cow::Owned(prefixed(&name, tool.name.as_ref()));
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
        emit: &Emitter,
    ) -> Result<CallToolResult, ErrorData> {
        let Some((ext_name, tool_name)) = split_prefixed(call.name.as_ref()) else {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "unknown mcp tool {}",
                call.name
            ))]));
        };
        let ext_name = ext_name.to_string();
        let tool_name = tool_name.to_string();
        self.maybe_restart(&ext_name).await;
        let timeout = self.hub.bounds().tool_timeout;
        let peer = {
            let clients = self.hub.clients.lock().await;
            clients.iter().find_map(|slot| {
                if slot.name != ext_name {
                    return None;
                }
                match &slot.state {
                    SlotState::Live { client, .. } => Some(client.peer().clone()),
                    SlotState::Dead { .. } => None,
                }
            })
        };
        let Some(peer) = peer else {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "mcp server {ext_name} is unavailable"
            ))]));
        };
        call.name = Cow::Owned(tool_name);
        let call_fut = peer.call_tool(call);
        tokio::pin!(call_fut);
        let result = tokio::select! {
            biased;
            _ = emit.cancelled() => {
                self.poison(&ext_name, "cancelled").await;
                return Ok(CallToolResult::error(vec![ContentBlock::text("cancelled")]));
            }
            _ = tokio::time::sleep(timeout) => {
                self.poison(&ext_name, "timeout").await;
                return Ok(CallToolResult::error(vec![ContentBlock::text("mcp tool timed out")]));
            }
            result = &mut call_fut => result,
        };
        match result {
            Ok(result) => Ok(result),
            Err(err) => Ok(CallToolResult::error(vec![ContentBlock::text(
                err.to_string(),
            )])),
        }
    }
}

impl McpToolProvider {
    async fn poison(&self, name: &str, reason: &str) {
        let mut clients = self.hub.clients.lock().await;
        if let Some(slot) = clients.iter_mut().find(|slot| slot.name == name) {
            poison_slot(slot, reason);
        }
    }

    async fn maybe_restart(&self, name: &str) {
        let spec = {
            let clients = self.hub.clients.lock().await;
            clients.iter().find_map(|slot| {
                if slot.name != name {
                    return None;
                }
                match slot.state {
                    SlotState::Dead { until } if Instant::now() >= until => {
                        Some((slot.spec.clone(), slot.cwd.clone()))
                    }
                    _ => None,
                }
            })
        };
        let Some((spec, cwd)) = spec else {
            return;
        };
        match spawn_slot(&spec, &cwd).await {
            Ok(fresh) => {
                let mut clients = self.hub.clients.lock().await;
                if let Some(slot) = clients.iter_mut().find(|slot| slot.name == name) {
                    *slot = fresh;
                }
            }
            Err(err) => {
                let mut clients = self.hub.clients.lock().await;
                if let Some(slot) = clients.iter_mut().find(|slot| slot.name == name) {
                    poison_slot(slot, &err.to_string());
                }
            }
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
            env: Default::default(),
        };
        let err = hub
            .connect(&[spec], Path::new("/tmp"))
            .await
            .expect_err("missing bin");
        assert!(matches!(err, HostError::Failed(_)));
    }

    #[tokio::test]
    async fn mcp_timeout_poisons_one_server() {
        let hub = McpHub::new();
        hub.set_bounds(Duration::from_millis(20), 16, 128);
        let mut live = ServerSlot {
            name: "other".into(),
            spec: ExtensionSpec {
                name: "other".into(),
                transport: "stdio".into(),
                command: vec!["true".into()],
                url: String::new(),
                env: Default::default(),
            },
            cwd: PathBuf::from("/tmp"),
            state: SlotState::Dead {
                until: Instant::now() + Duration::from_secs(60),
            },
        };
        let mut hung = ServerSlot {
            name: "hung".into(),
            spec: live.spec.clone(),
            cwd: PathBuf::from("/tmp"),
            state: SlotState::Dead {
                until: Instant::now() + Duration::from_secs(60),
            },
        };
        poison_slot(&mut hung, "timeout");
        assert!(matches!(hung.state, SlotState::Dead { .. }));
        assert!(matches!(live.state, SlotState::Dead { .. }));
        // The other slot is untouched by poisoning `hung`.
        let until_live = match live.state {
            SlotState::Dead { until } => until,
            SlotState::Live { .. } => Instant::now(),
        };
        poison_slot(&mut hung, "again");
        match live.state {
            SlotState::Dead { until } => assert_eq!(until, until_live),
            SlotState::Live { .. } => panic!("live mutated"),
        }
        let _ = hub;
    }
}
