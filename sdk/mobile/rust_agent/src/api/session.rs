//! Thin FFI over kim-agent-harness. Hard isolation from kim_client_ffi / IM.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures::stream::{self, BoxStream, StreamExt};
use kim_agent_harness::{
    DriveStatus, Effects, Harness, HarnessEffects, ResumeReport as HarnessResumeReport,
};
use kim_agent_hooks::NoopHooks;
use kim_agent_llm::{
    LlmClient, LlmError, LlmResult, OpenAiResponsesClient, ResponseRequest, ScriptedLlm,
};
use kim_agent_storage::{SqliteStorage, Storage};
use kim_agent_tools::{register_fs_tools, Tool, ToolError, ToolRegistry};
use kim_agent_types::{Context, JsonValue, OperationResult, StreamEvent, ToolResult};
use tokio::sync::{broadcast, RwLock};

use super::rt;
use crate::frb_generated::StreamSink;

/// `llm_backend`: "scripted" | "responses_http"
pub struct SessionOpenOpts {
    pub model: String,
    pub llm_backend: String,
    pub resume_on_open: bool,
    pub base_url: String,
    pub api_key: String,
    pub enable_fs_tools: bool,
    pub bash_enabled: bool,
}

impl Default for SessionOpenOpts {
    fn default() -> Self {
        Self {
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            resume_on_open: true,
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            enable_fs_tools: false,
            bash_enabled: false,
        }
    }
}

/// Flat UI event (same shape as IM KimSessionEvent — avoids freezed enums).
#[derive(Clone)]
pub struct AgentUiEvent {
    pub kind: String,
    pub operation_id: String,
    pub call_id: String,
    pub name: String,
    pub delta: String,
    pub arguments_json: String,
    pub output_preview: String,
    pub ok: bool,
    pub stop_reason: String,
    pub message: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub resumed_ops: Vec<String>,
}

impl AgentUiEvent {
    fn base(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            operation_id: String::new(),
            call_id: String::new(),
            name: String::new(),
            delta: String::new(),
            arguments_json: String::new(),
            output_preview: String::new(),
            ok: false,
            stop_reason: String::new(),
            message: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            resumed_ops: Vec::new(),
        }
    }

    fn session_ready(resumed_ops: Vec<String>) -> Self {
        let mut e = Self::base("session_ready");
        e.resumed_ops = resumed_ops;
        e
    }
    fn operation_started(operation_id: String) -> Self {
        let mut e = Self::base("operation_started");
        e.operation_id = operation_id;
        e
    }
    fn text_delta(operation_id: String, delta: String) -> Self {
        let mut e = Self::base("assistant_text_delta");
        e.operation_id = operation_id;
        e.delta = delta;
        e
    }
    fn tool_started(operation_id: String, call_id: String, name: String) -> Self {
        let mut e = Self::base("tool_call_started");
        e.operation_id = operation_id;
        e.call_id = call_id;
        e.name = name;
        e
    }
    fn tool_args(operation_id: String, call_id: String, delta: String) -> Self {
        let mut e = Self::base("tool_call_args_delta");
        e.operation_id = operation_id;
        e.call_id = call_id;
        e.delta = delta;
        e
    }
    fn tool_finished(
        operation_id: String,
        call_id: String,
        name: String,
        arguments_json: String,
    ) -> Self {
        let mut e = Self::base("tool_call_finished");
        e.operation_id = operation_id;
        e.call_id = call_id;
        e.name = name;
        e.arguments_json = arguments_json;
        e
    }
    fn tool_result(
        operation_id: String,
        call_id: String,
        ok: bool,
        output_preview: String,
    ) -> Self {
        let mut e = Self::base("tool_result");
        e.operation_id = operation_id;
        e.call_id = call_id;
        e.ok = ok;
        e.output_preview = output_preview;
        e
    }
    fn completed(operation_id: String, stop_reason: String) -> Self {
        let mut e = Self::base("operation_completed");
        e.operation_id = operation_id;
        e.stop_reason = stop_reason;
        e
    }
    fn aborted(operation_id: String) -> Self {
        let mut e = Self::base("operation_aborted");
        e.operation_id = operation_id;
        e
    }
    fn failed(operation_id: String, message: String) -> Self {
        let mut e = Self::base("operation_failed");
        e.operation_id = operation_id;
        e.message = message;
        e
    }
    fn usage(operation_id: String, input_tokens: u64, output_tokens: u64) -> Self {
        let mut e = Self::base("usage");
        e.operation_id = operation_id;
        e.input_tokens = input_tokens;
        e.output_tokens = output_tokens;
        e
    }
}

pub struct ResumeReportDto {
    pub resumed_ops: Vec<String>,
    pub statuses: Vec<String>,
}

pub struct SessionSnapshotDto {
    pub busy: bool,
    pub last_operation_id: String,
}

struct Shared {
    harness: RwLock<Option<Arc<Harness>>>,
    events: broadcast::Sender<AgentUiEvent>,
    current_cx: Mutex<Option<Context>>,
    current_op: Arc<Mutex<Option<String>>>,
    last_tool_call_id: Arc<Mutex<Option<String>>>,
    busy: AtomicBool,
    project_root: PathBuf,
    sqlite_path: String,
    opts: Mutex<SessionOpenOpts>,
}

pub struct AgentSession {
    inner: Arc<Shared>,
}

fn sqlite_url(path: &str) -> String {
    if path.starts_with("sqlite:") {
        return path.to_string();
    }
    let p = path.replace('\\', "/");
    if p.starts_with('/') {
        format!("sqlite://{p}")
    } else {
        format!("sqlite://{p}")
    }
}

fn demo_script_text() -> Vec<StreamEvent> {
    vec![
        StreamEvent::ResponseStarted {
            provider_response_id: "demo-text".into(),
        },
        StreamEvent::TextDelta {
            output_index: 0,
            delta: "Hello from ".into(),
        },
        StreamEvent::TextDelta {
            output_index: 0,
            delta: "ScriptedLlm".into(),
        },
        StreamEvent::TextDelta {
            output_index: 0,
            delta: " — streaming works. Try again for a tool demo.".into(),
        },
        StreamEvent::Completed,
    ]
}

fn demo_script_tool() -> Vec<StreamEvent> {
    vec![
        StreamEvent::ResponseStarted {
            provider_response_id: "demo-tool".into(),
        },
        StreamEvent::ToolCallStarted {
            output_index: 0,
            call_id: "call_demo".into(),
            name: "echo".into(),
        },
        StreamEvent::ToolCallArgsDelta {
            output_index: 0,
            call_id: "call_demo".into(),
            delta: "{\"msg\":\"kim\"}".into(),
        },
        StreamEvent::ToolCallFinished {
            output_index: 0,
            call_id: "call_demo".into(),
            name: "echo".into(),
            arguments: "{\"msg\":\"kim\"}".into(),
        },
        StreamEvent::Completed,
    ]
}

fn demo_script_after_tool() -> Vec<StreamEvent> {
    vec![
        StreamEvent::ResponseStarted {
            provider_response_id: "demo-after".into(),
        },
        StreamEvent::TextDelta {
            output_index: 0,
            delta: "Tool finished — Session SQLite kept; provider changes won't wipe it.".into(),
        },
        StreamEvent::Completed,
    ]
}

/// Alternates text / tool / after-tool scripts; never runs dry.
struct RefillingScripted {
    delay_ms: u64,
    turn: Mutex<u32>,
}

#[async_trait]
impl LlmClient for RefillingScripted {
    async fn stream(
        &self,
        _req: ResponseRequest,
        cx: &Context,
    ) -> LlmResult<BoxStream<'static, LlmResult<StreamEvent>>> {
        let n = {
            let mut g = self
                .turn
                .lock()
                .map_err(|e| LlmError::Failed(e.to_string()))?;
            let v = *g;
            *g = g.wrapping_add(1);
            v
        };
        let events = match n % 3 {
            0 => demo_script_text(),
            1 => demo_script_tool(),
            _ => demo_script_after_tool(),
        };
        let delay = self.delay_ms;
        let cancelled = cx.abort.clone();
        let s = stream::unfold(
            (events.into_iter(), cancelled, false),
            move |(mut iter, cancelled, aborted)| async move {
                if aborted {
                    return None;
                }
                if cancelled.is_cancelled() {
                    return Some((Err(LlmError::Aborted), (iter, cancelled, true)));
                }
                match iter.next() {
                    Some(ev) => {
                        if delay > 0 {
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                        }
                        if cancelled.is_cancelled() {
                            return Some((Err(LlmError::Aborted), (iter, cancelled, true)));
                        }
                        Some((Ok(ev), (iter, cancelled, false)))
                    }
                    None => None,
                }
            },
        );
        Ok(Box::pin(s))
    }
}

struct EchoTool {}

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }
    fn description(&self) -> &str {
        "Echo arguments as JSON"
    }
    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({"type":"object","properties":{"msg":{"type":"string"}}})
    }
    async fn call(&self, args: JsonValue) -> Result<ToolResult, ToolError> {
        Ok(ToolResult {
            output: args.to_string(),
            is_error: false,
        })
    }
}

struct TeeingEffects {
    inner: HarnessEffects,
    events: broadcast::Sender<AgentUiEvent>,
    current_op: Arc<Mutex<Option<String>>>,
    last_tool_call_id: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl Effects for TeeingEffects {
    async fn complete_llm(
        &self,
        req: ResponseRequest,
        cx: &Context,
    ) -> LlmResult<BoxStream<'static, LlmResult<StreamEvent>>> {
        let stream = self.inner.complete_llm(req, cx).await?;
        let events = self.events.clone();
        let current_op = self.current_op.clone();
        let last_tool = self.last_tool_call_id.clone();
        let mapped = stream.map(move |item| {
            if let Ok(ref ev) = item {
                let op = current_op
                    .lock()
                    .ok()
                    .and_then(|g| g.clone())
                    .unwrap_or_default();
                if let StreamEvent::ToolCallStarted { call_id, .. }
                | StreamEvent::ToolCallFinished { call_id, .. } = ev
                {
                    if let Ok(mut g) = last_tool.lock() {
                        *g = Some(call_id.clone());
                    }
                }
                if let Some(ui) = map_stream_event(&op, ev) {
                    let _ = events.send(ui);
                }
            }
            item
        });
        Ok(Box::pin(mapped))
    }

    async fn call_tool(&self, name: &str, args: JsonValue) -> Result<ToolResult, ToolError> {
        let op = self
            .current_op
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_default();
        let call_id = self
            .last_tool_call_id
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_else(|| name.to_string());
        let result = self.inner.call_tool(name, args).await;
        let (ok, preview) = match &result {
            Ok(r) => (!r.is_error, preview(&r.output)),
            Err(e) => (false, e.to_string()),
        };
        let _ = self
            .events
            .send(AgentUiEvent::tool_result(op, call_id, ok, preview));
        result
    }
}

fn preview(s: &str) -> String {
    const MAX: usize = 800;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(MAX).collect::<String>())
    }
}

fn map_stream_event(op: &str, ev: &StreamEvent) -> Option<AgentUiEvent> {
    match ev {
        StreamEvent::TextDelta { delta, .. } => {
            Some(AgentUiEvent::text_delta(op.to_string(), delta.clone()))
        }
        StreamEvent::ToolCallStarted { call_id, name, .. } => Some(AgentUiEvent::tool_started(
            op.to_string(),
            call_id.clone(),
            name.clone(),
        )),
        StreamEvent::ToolCallArgsDelta { call_id, delta, .. } => Some(AgentUiEvent::tool_args(
            op.to_string(),
            call_id.clone(),
            delta.clone(),
        )),
        StreamEvent::ToolCallFinished {
            call_id,
            name,
            arguments,
            ..
        } => Some(AgentUiEvent::tool_finished(
            op.to_string(),
            call_id.clone(),
            name.clone(),
            arguments.clone(),
        )),
        StreamEvent::UsageHint(u) => Some(AgentUiEvent::usage(
            op.to_string(),
            u.input_tokens,
            u.output_tokens,
        )),
        StreamEvent::Failed { message } => {
            Some(AgentUiEvent::failed(op.to_string(), message.clone()))
        }
        StreamEvent::ResponseStarted { .. } | StreamEvent::Completed => None,
    }
}

fn build_llm(opts: &SessionOpenOpts) -> Arc<dyn LlmClient> {
    match opts.llm_backend.as_str() {
        "responses_http" | "live" | "responses" => {
            Arc::new(OpenAiResponsesClient::new(&opts.base_url, &opts.api_key))
        }
        _ => Arc::new(RefillingScripted {
            delay_ms: 35,
            turn: Mutex::new(0),
        }),
    }
}

fn build_tools(opts: &SessionOpenOpts, project_root: &PathBuf) -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(EchoTool {}));
    if opts.enable_fs_tools {
        register_fs_tools(&mut tools, project_root, opts.bash_enabled);
    }
    tools
}

async fn build_harness(
    sqlite_path: &str,
    project_root: PathBuf,
    opts: &SessionOpenOpts,
    events: broadcast::Sender<AgentUiEvent>,
    current_op: Arc<Mutex<Option<String>>>,
    last_tool_call_id: Arc<Mutex<Option<String>>>,
) -> Result<Arc<Harness>, String> {
    let url = sqlite_url(sqlite_path);
    let storage: Arc<dyn Storage> =
        Arc::new(SqliteStorage::open(&url).await.map_err(|e| e.to_string())?);
    let llm = build_llm(opts);
    let tools = build_tools(opts, &project_root);
    let effects: Arc<dyn Effects> = Arc::new(TeeingEffects {
        inner: HarnessEffects {
            llm,
            tools: tools.clone(),
        },
        events,
        current_op,
        last_tool_call_id,
    });
    let model = if opts.model.is_empty() {
        "scripted".to_string()
    } else {
        opts.model.clone()
    };
    let harness = Harness::builder(storage)
        .effects(effects)
        .hooks(Arc::new(NoopHooks))
        .tools(tools)
        .project_root(project_root)
        .model(model)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(Arc::new(harness))
}

pub fn session_open(
    sqlite_path: String,
    project_root: String,
    opts: SessionOpenOpts,
) -> Result<AgentSession, String> {
    let _guard = rt().enter();
    let project_root = PathBuf::from(project_root);
    std::fs::create_dir_all(&project_root).map_err(|e| e.to_string())?;
    if let Some(parent) = std::path::Path::new(&sqlite_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let (tx, _) = broadcast::channel(256);
    let current_op = Arc::new(Mutex::new(None));
    let last_tool_call_id = Arc::new(Mutex::new(None));
    let resume_on_open = opts.resume_on_open;

    let harness = rt().block_on(build_harness(
        &sqlite_path,
        project_root.clone(),
        &opts,
        tx.clone(),
        current_op.clone(),
        last_tool_call_id.clone(),
    ))?;

    let shared = Arc::new(Shared {
        harness: RwLock::new(Some(harness.clone())),
        events: tx.clone(),
        current_cx: Mutex::new(None),
        current_op,
        last_tool_call_id,
        busy: AtomicBool::new(false),
        project_root,
        sqlite_path,
        opts: Mutex::new(opts),
    });

    if resume_on_open {
        match rt().block_on(async {
            let cx = Context::new();
            harness.resume(&cx).await
        }) {
            Ok(r) => {
                let resumed: Vec<String> =
                    r.resumed_operations.iter().map(|o| o.to_string()).collect();
                let _ = tx.send(AgentUiEvent::session_ready(resumed));
            }
            Err(e) => {
                let _ = tx.send(AgentUiEvent::failed(String::new(), format!("resume: {e}")));
            }
        }
    } else {
        let _ = tx.send(AgentUiEvent::session_ready(Vec::new()));
    }

    Ok(AgentSession { inner: shared })
}

impl AgentSession {
    pub fn prompt(&self, text: String) -> Result<String, String> {
        if self.inner.busy.swap(true, Ordering::SeqCst) {
            return Err("agent busy".into());
        }
        let text = text.trim().to_string();
        if text.is_empty() {
            self.inner.busy.store(false, Ordering::SeqCst);
            return Err("empty prompt".into());
        }

        let inner = self.inner.clone();
        let (op_id_tx, op_id_rx) = std::sync::mpsc::channel();

        rt().spawn(async move {
            let harness = { inner.harness.read().await.clone() };
            let Some(harness) = harness else {
                let _ = inner
                    .events
                    .send(AgentUiEvent::failed(String::new(), "session closed".into()));
                inner.busy.store(false, Ordering::SeqCst);
                let _ = op_id_tx.send(Err("session closed".to_string()));
                return;
            };

            let cx = Context::new();
            if let Ok(mut g) = inner.current_cx.lock() {
                *g = Some(cx.clone());
            }

            let adm = match harness
                .accept(kim_agent_harness::AcceptRequest::prompt(text), &cx)
                .await
            {
                Ok(a) => a,
                Err(e) => {
                    let _ = inner
                        .events
                        .send(AgentUiEvent::failed(String::new(), e.to_string()));
                    inner.busy.store(false, Ordering::SeqCst);
                    let _ = op_id_tx.send(Err(e.to_string()));
                    return;
                }
            };
            let op = adm.operation_id.to_string();
            if let Ok(mut g) = inner.current_op.lock() {
                *g = Some(op.clone());
            }
            let _ = inner
                .events
                .send(AgentUiEvent::operation_started(op.clone()));
            let _ = op_id_tx.send(Ok(op.clone()));

            let status = harness.drive(adm.operation_id, &cx).await;
            if let Ok(mut g) = inner.current_cx.lock() {
                *g = None;
            }
            if let Ok(mut g) = inner.current_op.lock() {
                *g = None;
            }
            match status {
                Ok(DriveStatus::Completed { result }) => match result {
                    OperationResult::Aborted { .. } => {
                        let _ = inner.events.send(AgentUiEvent::aborted(op));
                    }
                    OperationResult::Failed { message, .. } => {
                        let _ = inner.events.send(AgentUiEvent::failed(op, message));
                    }
                    OperationResult::Completed { .. } => {
                        let _ = inner
                            .events
                            .send(AgentUiEvent::completed(op, "completed".into()));
                    }
                },
                Ok(DriveStatus::Aborted { .. }) => {
                    let _ = inner.events.send(AgentUiEvent::aborted(op));
                }
                Ok(DriveStatus::Parked { reason }) => {
                    let _ = inner
                        .events
                        .send(AgentUiEvent::failed(op, format!("parked: {reason}")));
                }
                Ok(DriveStatus::Fault { error }) => {
                    let _ = inner.events.send(AgentUiEvent::failed(op, error));
                }
                Err(e) => {
                    let _ = inner.events.send(AgentUiEvent::failed(op, e.to_string()));
                }
            }
            inner.busy.store(false, Ordering::SeqCst);
        });

        op_id_rx
            .recv_timeout(Duration::from_secs(30))
            .map_err(|_| "prompt start timeout".to_string())?
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn listen(&self, sink: StreamSink<AgentUiEvent>) -> Result<(), String> {
        let _guard = rt().enter();
        let mut rx = self.inner.events.subscribe();
        rt().spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(ev) => {
                        if sink.add(ev).is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        Ok(())
    }

    pub fn abort(&self) -> Result<(), String> {
        let _guard = rt().enter();
        if let Ok(g) = self.inner.current_cx.lock() {
            if let Some(cx) = g.as_ref() {
                cx.cancel();
            }
        }
        let inner = self.inner.clone();
        rt().spawn(async move {
            let harness = { inner.harness.read().await.clone() };
            if let Some(h) = harness {
                let cx = Context::new();
                let _ = h.request_abort("main", &cx).await;
            }
        });
        Ok(())
    }

    pub fn resume(&self) -> Result<ResumeReportDto, String> {
        let _guard = rt().enter();
        let inner = self.inner.clone();
        rt().block_on(async move {
            let harness = { inner.harness.read().await.clone() };
            let Some(h) = harness else {
                return Err("session closed".into());
            };
            let cx = Context::new();
            if let Ok(mut g) = inner.current_cx.lock() {
                *g = Some(cx.clone());
            }
            inner.busy.store(true, Ordering::SeqCst);
            let report = h.resume(&cx).await.map_err(|e| e.to_string());
            if let Ok(mut g) = inner.current_cx.lock() {
                *g = None;
            }
            inner.busy.store(false, Ordering::SeqCst);
            Ok(map_resume(report?))
        })
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn snapshot(&self) -> Result<SessionSnapshotDto, String> {
        let last = self
            .inner
            .current_op
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_default();
        Ok(SessionSnapshotDto {
            busy: self.inner.busy.load(Ordering::SeqCst),
            last_operation_id: last,
        })
    }

    /// Rebuild harness/client; SQLite session tree is preserved.
    pub fn reconfigure(&self, opts: SessionOpenOpts) -> Result<(), String> {
        let _guard = rt().enter();
        if self.inner.busy.load(Ordering::SeqCst) {
            return Err("cannot reconfigure while busy".into());
        }
        let inner = self.inner.clone();
        rt().block_on(async move {
            let harness = build_harness(
                &inner.sqlite_path,
                inner.project_root.clone(),
                &opts,
                inner.events.clone(),
                inner.current_op.clone(),
                inner.last_tool_call_id.clone(),
            )
            .await?;
            *inner.harness.write().await = Some(harness);
            *inner.opts.lock().map_err(|e| e.to_string())? = opts;
            Ok(())
        })
    }

    pub fn close(&self) {
        let inner = self.inner.clone();
        let _ = rt().block_on(async move {
            *inner.harness.write().await = None;
        });
    }
}

fn map_resume(r: HarnessResumeReport) -> ResumeReportDto {
    ResumeReportDto {
        resumed_ops: r.resumed_operations.iter().map(|o| o.to_string()).collect(),
        statuses: r.drive_statuses.into_iter().map(|(_, s)| s).collect(),
    }
}

#[allow(dead_code)]
fn _keep_scripted() -> ScriptedLlm {
    ScriptedLlm::single(vec![StreamEvent::Completed])
}
