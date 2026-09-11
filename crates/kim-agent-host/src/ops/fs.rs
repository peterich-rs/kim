use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::Emitter;
use goose_agent::tool::ToolProvider;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, ErrorData, JsonObject, Tool,
};
use serde_json::{json, Value};

use crate::HostSession;

pub struct FsToolProvider {
    pub root: PathBuf,
    pub writable: bool,
}

fn schema(value: Value) -> Arc<JsonObject> {
    match value {
        Value::Object(map) => Arc::new(map),
        _ => Arc::new(JsonObject::new()),
    }
}

fn arg_string(call: &CallToolRequestParams, key: &str) -> Result<String, ErrorData> {
    let args = call
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params(format!("missing {key}"), None))?;
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ErrorData::invalid_params(format!("missing {key}"), None))
}

fn candidate_in_root(root: &Path, raw: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

fn resolve_in_root(root: &Path, raw: &str) -> Result<PathBuf, String> {
    let root_canon = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let candidate = candidate_in_root(root, raw);
    let canon = std::fs::canonicalize(&candidate).map_err(|e| e.to_string())?;
    if !canon.starts_with(&root_canon) {
        return Err("path escapes workspace".into());
    }
    Ok(canon)
}

/// Canonicalize the existing parent, then join the final component so a missing
/// target (new file) still resolves. Reject `..` / `.` as the leaf and symlink escape.
fn resolve_in_root_for_write(root: &Path, raw: &str) -> Result<PathBuf, String> {
    let root_canon = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let candidate = candidate_in_root(root, raw);
    let file_name = candidate
        .file_name()
        .ok_or_else(|| "invalid path".to_string())?;
    if file_name == "." || file_name == ".." {
        return Err("path escapes workspace".into());
    }
    // WHY: Path::exists follows links, so a dangling workspace symlink looks
    // like a new file and write() would create the outside target.
    let canon = match std::fs::symlink_metadata(&candidate) {
        Ok(_) => std::fs::canonicalize(&candidate).map_err(|e| e.to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let parent = candidate
                .parent()
                .ok_or_else(|| "invalid path".to_string())?;
            let parent_canon = std::fs::canonicalize(parent).map_err(|e| e.to_string())?;
            parent_canon.join(file_name)
        }
        Err(e) => return Err(e.to_string()),
    };
    if !canon.starts_with(&root_canon) {
        return Err("path escapes workspace".into());
    }
    Ok(canon)
}

fn error_result(msg: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(msg.into())])
}

#[async_trait]
impl ToolProvider<HostSession> for FsToolProvider {
    async fn tools(&self, _session: &HostSession) -> Result<Vec<Tool>> {
        let mut tools = vec![
            Tool::new(
                "read_file",
                "Read a UTF-8 file under the agent workspace.",
                schema(json!({
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                    "required": ["path"],
                    "additionalProperties": false
                })),
            ),
            Tool::new(
                "list_dir",
                "List a directory under the agent workspace.",
                schema(json!({
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                    "required": ["path"],
                    "additionalProperties": false
                })),
            ),
        ];
        if self.writable {
            tools.push(Tool::new(
                "write_file",
                "Write a UTF-8 file under the agent workspace.",
                schema(json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "contents": {"type": "string"}
                    },
                    "required": ["path", "contents"],
                    "additionalProperties": false
                })),
            ));
        }
        Ok(tools)
    }

    async fn call(
        &self,
        _session: &HostSession,
        _request_id: &str,
        call: CallToolRequestParams,
        _emit: &Emitter,
    ) -> Result<CallToolResult, ErrorData> {
        match call.name.as_ref() {
            "read_file" => {
                let path = arg_string(&call, "path")?;
                Ok(self.read_file(&path))
            }
            "list_dir" => {
                let path = arg_string(&call, "path")?;
                Ok(self.list_dir(&path))
            }
            "write_file" if self.writable => {
                let path = arg_string(&call, "path")?;
                let contents = arg_string(&call, "contents")?;
                Ok(self.write_file(&path, &contents))
            }
            other => Ok(error_result(format!("unknown fs tool {other}"))),
        }
    }
}

impl FsToolProvider {
    fn read_file(&self, raw: &str) -> CallToolResult {
        let path = match resolve_in_root(&self.root, raw) {
            Ok(p) => p,
            Err(e) => return error_result(e),
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => CallToolResult::structured(json!({"ok": true, "text": text})),
            Err(e) => error_result(e.to_string()),
        }
    }

    fn list_dir(&self, raw: &str) -> CallToolResult {
        let path = match resolve_in_root(&self.root, raw) {
            Ok(p) => p,
            Err(e) => return error_result(e),
        };
        let rd = match std::fs::read_dir(&path) {
            Ok(rd) => rd,
            Err(e) => return error_result(e.to_string()),
        };
        let mut names = Vec::new();
        for ent in rd {
            match ent {
                Ok(e) => names.push(e.file_name().to_string_lossy().into_owned()),
                Err(e) => return error_result(e.to_string()),
            }
        }
        CallToolResult::structured(json!({"ok": true, "entries": names}))
    }

    fn write_file(&self, raw: &str, contents: &str) -> CallToolResult {
        let path = match resolve_in_root_for_write(&self.root, raw) {
            Ok(p) => p,
            Err(e) => return error_result(e),
        };
        match std::fs::write(&path, contents) {
            Ok(()) => CallToolResult::structured(json!({"ok": true})),
            Err(e) => error_result(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goose_agent::operation::Emitter;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn emit() -> Emitter {
        let (tx, _rx) = mpsc::channel(8);
        Emitter::new(tx, CancellationToken::new())
    }

    #[tokio::test]
    async fn path_escape_fails() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(dir.path().join("secret.txt"), "nope").unwrap();
        let provider = FsToolProvider {
            root: root.clone(),
            writable: false,
        };
        let session = crate::HostSession {
            id: "s".into(),
            conversation: goose_provider_types::conversation::Conversation::empty(),
        };
        let mut call = CallToolRequestParams::new("read_file");
        let mut args = JsonObject::new();
        args.insert("path".into(), json!("../ok.txt"));
        call.arguments = Some(args);
        let result = provider.call(&session, "1", call, &emit()).await.unwrap();
        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn read_file_inside_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("ok.txt"), "hello").unwrap();
        let provider = FsToolProvider {
            root: root.clone(),
            writable: false,
        };
        let session = crate::HostSession {
            id: "s".into(),
            conversation: goose_provider_types::conversation::Conversation::empty(),
        };
        let mut call = CallToolRequestParams::new("read_file");
        let mut args = JsonObject::new();
        args.insert("path".into(), json!("ok.txt"));
        call.arguments = Some(args);
        let result = provider.call(&session, "1", call, &emit()).await.unwrap();
        assert_ne!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn write_file_creates_new_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        let provider = FsToolProvider {
            root: root.clone(),
            writable: true,
        };
        let session = crate::HostSession {
            id: "s".into(),
            conversation: goose_provider_types::conversation::Conversation::empty(),
        };
        let mut call = CallToolRequestParams::new("write_file");
        let mut args = JsonObject::new();
        args.insert("path".into(), json!("fresh.txt"));
        args.insert("contents".into(), json!("hello"));
        call.arguments = Some(args);
        let result = provider.call(&session, "1", call, &emit()).await.unwrap();
        assert_ne!(result.is_error, Some(true));
        assert_eq!(
            std::fs::read_to_string(root.join("fresh.txt")).unwrap(),
            "hello"
        );
    }

    #[tokio::test]
    async fn write_file_escape_fails() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        let provider = FsToolProvider {
            root: root.clone(),
            writable: true,
        };
        let session = crate::HostSession {
            id: "s".into(),
            conversation: goose_provider_types::conversation::Conversation::empty(),
        };
        let mut call = CallToolRequestParams::new("write_file");
        let mut args = JsonObject::new();
        args.insert("path".into(), json!("../secret.txt"));
        args.insert("contents".into(), json!("nope"));
        call.arguments = Some(args);
        let result = provider.call(&session, "1", call, &emit()).await.unwrap();
        assert_eq!(result.is_error, Some(true));
        assert!(!dir.path().join("secret.txt").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn write_file_dangling_symlink_does_not_create_outside() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        let outside = dir.path().join("outside");
        std::os::unix::fs::symlink(&outside, root.join("evil.txt")).unwrap();
        let provider = FsToolProvider {
            root: root.clone(),
            writable: true,
        };
        let session = crate::HostSession {
            id: "s".into(),
            conversation: goose_provider_types::conversation::Conversation::empty(),
        };
        let mut call = CallToolRequestParams::new("write_file");
        let mut args = JsonObject::new();
        args.insert("path".into(), json!("evil.txt"));
        args.insert("contents".into(), json!("pwned"));
        call.arguments = Some(args);
        let result = provider.call(&session, "1", call, &emit()).await.unwrap();
        assert_eq!(result.is_error, Some(true));
        assert!(!outside.exists());
    }
}
