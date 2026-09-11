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
use tokio::process::Command;

use crate::HostSession;

const TIMEOUT: Duration = Duration::from_secs(30);
const PREVIEW_BYTES: usize = 8 * 1024;

pub struct BashToolProvider {
    pub root: PathBuf,
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
    let mut end = PREVIEW_BYTES;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
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
        Ok(self.run_argv(argv).await)
    }
}

impl BashToolProvider {
    async fn run_argv(&self, argv: Vec<String>) -> CallToolResult {
        let prog = &argv[0];
        if prog.contains("..") {
            return error_result("argv[0] must not contain ..");
        }
        // Do not spawn through a shell; argv values are literals, not interpolated.
        let mut cmd = Command::new(prog);
        cmd.args(&argv[1..])
            .current_dir(&self.root)
            .kill_on_drop(true)
            .env_remove("BASH_ENV")
            .env_remove("ENV");
        let output = match tokio::time::timeout(TIMEOUT, cmd.output()).await {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return error_result(e.to_string()),
            Err(_) => return error_result("bash timed out after 30s"),
        };
        let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.stderr.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        let preview = truncate(&combined);
        let ok = output.status.success();
        if ok {
            CallToolResult::structured(json!({"ok": true, "output": preview}))
        } else {
            error_result(preview)
        }
    }
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
        let provider = BashToolProvider {
            root: dir.path().to_path_buf(),
        };
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
        let provider = BashToolProvider {
            root: dir.path().to_path_buf(),
        };
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
