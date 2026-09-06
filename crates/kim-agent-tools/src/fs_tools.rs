//! Local FS / bash tools scoped to a project root.
//!
//! Debug builds may need App Sandbox disabled on macOS; see docs/agent-macos-desktop.md.

use crate::{Tool, ToolError};
use async_trait::async_trait;
use kim_agent_types::{JsonValue, ToolResult};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;

/// Deny path escape outside `root`.
fn resolve_under(root: &Path, rel: &str) -> Result<PathBuf, ToolError> {
    let joined = if Path::new(rel).is_absolute() {
        PathBuf::from(rel)
    } else {
        root.join(rel)
    };
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let resolved = if joined.exists() {
        joined
            .canonicalize()
            .map_err(|e| ToolError::Failed(e.to_string()))?
    } else {
        let parent = joined.parent().unwrap_or(root);
        let file = joined
            .file_name()
            .ok_or_else(|| ToolError::Failed("bad path".into()))?;
        let parent_canon = parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
        parent_canon.join(file)
    };
    if !resolved.starts_with(&root_canon) {
        return Err(ToolError::Failed(format!(
            "path escapes project_root: {rel}"
        )));
    }
    Ok(resolved)
}

fn arg_str(args: &JsonValue, key: &str) -> Result<String, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ToolError::Failed(format!("missing string arg `{key}`")))
}

pub struct ReadTool {
    root: PathBuf,
}

pub struct WriteTool {
    root: PathBuf,
}

pub struct EditTool {
    root: PathBuf,
}

pub struct BashTool {
    root: PathBuf,
    enabled: bool,
}

impl ReadTool {
    pub fn new(root: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self { root: root.into() })
    }
}
impl WriteTool {
    pub fn new(root: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self { root: root.into() })
    }
}
impl EditTool {
    pub fn new(root: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self { root: root.into() })
    }
}
impl BashTool {
    pub fn new(root: impl Into<PathBuf>, enabled: bool) -> Arc<Self> {
        Arc::new(Self {
            root: root.into(),
            enabled,
        })
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }
    fn description(&self) -> &str {
        "Read a UTF-8 text file under project_root"
    }
    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type":"object",
            "properties":{"path":{"type":"string"}},
            "required":["path"]
        })
    }
    async fn call(&self, args: JsonValue) -> Result<ToolResult, ToolError> {
        let path = arg_str(&args, "path")?;
        let full = resolve_under(&self.root, &path)?;
        let text = tokio::fs::read_to_string(&full)
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        Ok(ToolResult {
            output: text,
            is_error: false,
        })
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }
    fn description(&self) -> &str {
        "Write a UTF-8 text file under project_root (creates parents)"
    }
    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type":"object",
            "properties":{
                "path":{"type":"string"},
                "content":{"type":"string"}
            },
            "required":["path","content"]
        })
    }
    async fn call(&self, args: JsonValue) -> Result<ToolResult, ToolError> {
        let path = arg_str(&args, "path")?;
        let content = arg_str(&args, "content")?;
        let full = resolve_under(&self.root, &path)?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ToolError::Failed(e.to_string()))?;
        }
        tokio::fs::write(&full, content.as_bytes())
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        Ok(ToolResult {
            output: format!("wrote {}", full.display()),
            is_error: false,
        })
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }
    fn description(&self) -> &str {
        "Replace an exact string occurrence in a file under project_root"
    }
    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type":"object",
            "properties":{
                "path":{"type":"string"},
                "old_str":{"type":"string"},
                "new_str":{"type":"string"}
            },
            "required":["path","old_str","new_str"]
        })
    }
    async fn call(&self, args: JsonValue) -> Result<ToolResult, ToolError> {
        let path = arg_str(&args, "path")?;
        let old = arg_str(&args, "old_str")?;
        let new = arg_str(&args, "new_str")?;
        let full = resolve_under(&self.root, &path)?;
        let text = tokio::fs::read_to_string(&full)
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        if !text.contains(&old) {
            return Ok(ToolResult {
                output: "old_str not found".into(),
                is_error: true,
            });
        }
        let updated = text.replacen(&old, &new, 1);
        tokio::fs::write(&full, updated.as_bytes())
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        Ok(ToolResult {
            output: format!("edited {}", full.display()),
            is_error: false,
        })
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }
    fn description(&self) -> &str {
        "Run a shell command with cwd=project_root (disabled when policy forbids)"
    }
    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type":"object",
            "properties":{"command":{"type":"string"}},
            "required":["command"]
        })
    }
    async fn call(&self, args: JsonValue) -> Result<ToolResult, ToolError> {
        if !self.enabled {
            return Ok(ToolResult {
                output: "bash disabled by permission policy / sandbox".into(),
                is_error: true,
            });
        }
        let command = arg_str(&args, "command")?;
        let output = Command::new("bash")
            .arg("-lc")
            .arg(&command)
            .current_dir(&self.root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        let mut text = String::from_utf8_lossy(&output.stdout).to_string();
        let err = String::from_utf8_lossy(&output.stderr);
        if !err.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&err);
        }
        if text.len() > 32_000 {
            text.truncate(32_000);
            text.push_str("\n…truncated");
        }
        Ok(ToolResult {
            output: text,
            is_error: !output.status.success(),
        })
    }
}

/// Register read/write/edit/bash under `root`. `bash_enabled` gates shell.
pub fn register_fs_tools(
    registry: &mut crate::ToolRegistry,
    root: impl Into<PathBuf>,
    bash_enabled: bool,
) {
    let root = root.into();
    registry.register(ReadTool::new(root.clone()));
    registry.register(WriteTool::new(root.clone()));
    registry.register(EditTool::new(root.clone()));
    registry.register(BashTool::new(root, bash_enabled));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolRegistry;

    #[tokio::test]
    async fn write_read_edit_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut reg = ToolRegistry::new();
        register_fs_tools(&mut reg, dir.path(), false);
        let write = reg.get("write").unwrap();
        write
            .call(serde_json::json!({"path":"a.txt","content":"hello"}))
            .await
            .unwrap();
        let read = reg.get("read").unwrap();
        let out = read
            .call(serde_json::json!({"path":"a.txt"}))
            .await
            .unwrap();
        assert_eq!(out.output, "hello");
        let edit = reg.get("edit").unwrap();
        edit.call(serde_json::json!({"path":"a.txt","old_str":"hello","new_str":"hi"}))
            .await
            .unwrap();
        let out = read
            .call(serde_json::json!({"path":"a.txt"}))
            .await
            .unwrap();
        assert_eq!(out.output, "hi");
        let bash = reg.get("bash").unwrap();
        let blocked = bash
            .call(serde_json::json!({"command":"echo x"}))
            .await
            .unwrap();
        assert!(blocked.is_error);
    }
}
