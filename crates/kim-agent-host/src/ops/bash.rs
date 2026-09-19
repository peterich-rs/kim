use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::Emitter;
use goose_agent::tool::ToolProvider;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, ErrorData, JsonObject, Tool,
};
use serde_json::{json, Value};

use crate::harness::{kill_group, prepare};
use crate::HostSession;

const PREVIEW_BYTES: usize = 50 * 1024;

pub struct BashToolProvider {
    pub root: PathBuf,
    pub timeout: Duration,
}

impl BashToolProvider {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            timeout: Duration::from_secs(30),
        }
    }
}

fn schema(value: Value) -> Arc<JsonObject> {
    match value {
        Value::Object(map) => Arc::new(map),
        _ => Arc::new(JsonObject::new()),
    }
}

fn error_result(msg: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(msg.into())])
}

fn truncate(s: &str) -> String {
    if s.len() <= PREVIEW_BYTES {
        return s.to_string();
    }
    let marker = " … ";
    let keep = PREVIEW_BYTES.saturating_sub(marker.len()) / 2;
    let mut head = keep;
    while head > 0 && !s.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = s.len().saturating_sub(keep);
    while tail < s.len() && !s.is_char_boundary(tail) {
        tail += 1;
    }
    format!("{}{marker}{}", &s[..head], &s[tail..])
}

fn parse_argv(call: &CallToolRequestParams) -> Result<Vec<String>, ErrorData> {
    let args = call
        .arguments
        .as_ref()
        .ok_or_else(|| ErrorData::invalid_params("missing argv", None))?;
    let raw = args
        .get("argv")
        .ok_or_else(|| ErrorData::invalid_params("missing argv", None))?;
    let arr = raw
        .as_array()
        .ok_or_else(|| ErrorData::invalid_params("argv must be an array", None))?;
    if arr.is_empty() {
        return Err(ErrorData::invalid_params("argv must be non-empty", None));
    }
    let mut argv = Vec::with_capacity(arr.len());
    for item in arr {
        let s = item
            .as_str()
            .ok_or_else(|| ErrorData::invalid_params("argv items must be strings", None))?;
        argv.push(s.to_string());
    }
    Ok(argv)
}

#[async_trait]
impl ToolProvider<HostSession> for BashToolProvider {
    async fn tools(&self, _session: &HostSession) -> Result<Vec<Tool>> {
        Ok(vec![Tool::new(
            "bash",
            "Run a process in the agent workspace. Arguments are argv, not a shell string.",
            schema(json!({
                "type": "object",
                "properties": {
                    "argv": {
                        "type": "array",
                        "items": {"type": "string"},
                        "minItems": 1
                    }
                },
                "required": ["argv"],
                "additionalProperties": false
            })),
        )])
    }

    async fn call(
        &self,
        _session: &HostSession,
        _request_id: &str,
        call: CallToolRequestParams,
        _emit: &Emitter,
    ) -> Result<CallToolResult, ErrorData> {
        if call.name.as_ref() != "bash" {
            return Ok(error_result(format!("unknown tool {}", call.name)));
        }
        let argv = parse_argv(&call)?;
        Ok(self.run_argv(argv, _emit).await)
    }
}

impl BashToolProvider {
    async fn run_argv(&self, argv: Vec<String>, emit: &Emitter) -> CallToolResult {
        let prog = &argv[0];
        if prog.contains("..") {
            return error_result("argv[0] must not contain ..");
        }
        let mut cmd = prepare(prog);
        cmd.args(&argv[1..])
            .current_dir(&self.root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => return error_result(err.to_string()),
        };
        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let read_out = tokio::spawn(read_capped(stdout));
        let read_err = tokio::spawn(read_capped(stderr));
        let status = tokio::select! {
            biased;
            _ = emit.cancelled() => {
                if let Some(pid) = pid {
                    kill_group(pid);
                }
                let _ = child.start_kill();
                let _ = child.wait().await;
                let _ = read_out.await;
                let _ = read_err.await;
                return error_result("cancelled");
            }
            _ = tokio::time::sleep(self.timeout) => {
                if let Some(pid) = pid {
                    kill_group(pid);
                }
                let _ = child.start_kill();
                let _ = child.wait().await;
                let _ = read_out.await;
                let _ = read_err.await;
                return error_result("bash timed out");
            }
            status = child.wait() => status,
        };
        let stdout = read_out.await.unwrap_or_default();
        let stderr = read_err.await.unwrap_or_default();
        let output = match status {
            Ok(status) => status,
            Err(err) => return error_result(err.to_string()),
        };
        let mut combined = String::from_utf8_lossy(&stdout).into_owned();
        if !stderr.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(&String::from_utf8_lossy(&stderr));
        }
        let preview = truncate(&combined);
        if output.success() {
            CallToolResult::structured(json!({"ok": true, "output": preview}))
        } else {
            error_result(preview)
        }
    }
}

async fn read_capped(pipe: Option<impl tokio::io::AsyncRead + Unpin>) -> Vec<u8> {
    let Some(mut pipe) = pipe else {
        return Vec::new();
    };
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        match pipe.read(&mut tmp).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf.len() < PREVIEW_BYTES {
                    let room = PREVIEW_BYTES - buf.len();
                    buf.extend_from_slice(&tmp[..n.min(room)]);
                }
            }
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HostSession;
    use goose_agent::operation::Emitter;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn emit() -> Emitter {
        let (tx, _rx) = mpsc::channel(8);
        Emitter::new(tx, CancellationToken::new())
    }

    fn session() -> HostSession {
        HostSession {
            id: "s".into(),
            conversation: goose_provider_types::conversation::Conversation::empty(),
        }
    }

    #[tokio::test]
    async fn rejects_dotdot_argv0() {
        let dir = tempfile::tempdir().unwrap();
        let provider = BashToolProvider::new(dir.path().to_path_buf());
        let mut call = CallToolRequestParams::new("bash");
        let mut args = JsonObject::new();
        args.insert("argv".into(), json!(["../bin/echo", "hi"]));
        call.arguments = Some(args);
        let result = provider.call(&session(), "1", call, &emit()).await.unwrap();
        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn does_not_expand_env_in_argv() {
        let dir = tempfile::tempdir().unwrap();
        let provider = BashToolProvider::new(dir.path().to_path_buf());
        let mut call = CallToolRequestParams::new("bash");
        let mut args = JsonObject::new();
        args.insert("argv".into(), json!(["/bin/echo", "$HOME"]));
        call.arguments = Some(args);
        let result = provider.call(&session(), "1", call, &emit()).await.unwrap();
        assert_ne!(result.is_error, Some(true));
        let text = format!("{result:?}");
        assert!(text.contains("$HOME"), "{text}");
    }
}
