//! Thin FFI over `kim-agent-host` (Goose). Isolated from `kim_client_ffi`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kim_agent_host::{
    parse_permission, AgentHost, AgentProfile, HostError, HostEvent, LegacyOpenOpts, ProviderSpec,
    ResolvedProfile, TurnOutcome, YieldKind,
};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_util::sync::CancellationToken;

use super::rt;
use crate::frb_generated::StreamSink;

pub struct SessionOpenOpts {
    pub model: String,
    pub llm_backend: String,
    pub resume_on_open: bool,
    pub base_url: String,
    pub api_key: String,
    pub enable_fs_tools: bool,
    pub bash_enabled: bool,
    pub profile_id: String,
    pub profile_json: String,
    pub thinking_effort: String,
    pub goose_mode: String,
    pub enable_kim_tools: bool,
    pub enable_approvals: bool,
    pub session_id: String,
}

impl Default for SessionOpenOpts {
    fn default() -> Self {
        Self {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            resume_on_open: true,
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            enable_fs_tools: false,
            bash_enabled: false,
            profile_id: String::new(),
            profile_json: String::new(),
            thinking_effort: String::new(),
            goose_mode: String::new(),
            enable_kim_tools: false,
            enable_approvals: false,
            session_id: String::new(),
        }
    }
}

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

    fn session_ready() -> Self {
        Self::base("session_ready")
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

    fn completed(operation_id: String, text: String) -> Self {
        let mut e = Self::base("assistant_finished");
        e.operation_id = operation_id;
        e.message = text;
        e.stop_reason = "completed".into();
        e.ok = true;
        e
    }

    fn failed(operation_id: String, message: String) -> Self {
        let mut e = Self::base("failed");
        e.operation_id = operation_id;
        e.message = message;
        e
    }

    fn aborted(operation_id: String) -> Self {
        let mut e = Self::base("aborted");
        e.operation_id = operation_id;
        e.stop_reason = "aborted".into();
        e
    }

    fn tool_request(
        operation_id: String,
        call_id: String,
        name: String,
        arguments_json: String,
    ) -> Self {
        let mut e = Self::base("tool_request");
        e.operation_id = operation_id;
        e.call_id = call_id;
        e.name = name;
        e.arguments_json = arguments_json;
        e
    }

    fn tool_started(operation_id: String, call_id: String, name: String) -> Self {
        let mut e = Self::base("tool_started");
        e.operation_id = operation_id;
        e.call_id = call_id;
        e.name = name;
        e
    }

    fn tool_finished(
        operation_id: String,
        call_id: String,
        name: String,
        output_preview: String,
        ok: bool,
    ) -> Self {
        let mut e = Self::base("tool_finished");
        e.operation_id = operation_id;
        e.call_id = call_id;
        e.name = name;
        e.output_preview = output_preview;
        e.ok = ok;
        e
    }

    fn action_required(
        operation_id: String,
        call_id: String,
        name: String,
        arguments_json: String,
        prompt: String,
    ) -> Self {
        let mut e = Self::base("action_required");
        e.operation_id = operation_id;
        e.call_id = call_id;
        e.name = name;
        e.arguments_json = arguments_json;
        e.message = prompt;
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
    pub phase: String,
    pub pending_call_ids: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SessionPhase {
    Idle,
    Running,
    Yielded,
}

impl SessionPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Yielded => "yielded",
        }
    }
}

struct Shared {
    host: RwLock<AgentHost>,
    session_id: String,
    events: broadcast::Sender<AgentUiEvent>,
    phase: Mutex<SessionPhase>,
    replay: Mutex<Vec<AgentUiEvent>>,
    complete_gate: tokio::sync::Mutex<()>,
    cancel: Mutex<Option<CancellationToken>>,
    generation: AtomicU64,
}

pub struct AgentSession {
    inner: Arc<Shared>,
}

fn resolved_from_opts(
    opts: &SessionOpenOpts,
    project_root: String,
) -> Result<ResolvedProfile, String> {
    let mut profile = if opts.profile_json.trim().is_empty() {
        AgentProfile::from_legacy(&LegacyOpenOpts {
            model: opts.model.clone(),
            llm_backend: opts.llm_backend.clone(),
            base_url: opts.base_url.clone(),
            enable_fs_tools: opts.enable_fs_tools,
            bash_enabled: opts.bash_enabled,
            enable_kim_tools: opts.enable_kim_tools,
            enable_approvals: opts.enable_approvals,
            thinking_effort: opts.thinking_effort.clone(),
            goose_mode: opts.goose_mode.clone(),
            profile_id: if opts.profile_id.trim().is_empty() {
                "goose".into()
            } else {
                opts.profile_id.clone()
            },
        })
    } else {
        serde_json::from_str(&opts.profile_json).map_err(|e| e.to_string())?
    };
    profile.fill_provider_from_legacy(&LegacyOpenOpts {
        llm_backend: opts.llm_backend.clone(),
        base_url: opts.base_url.clone(),
        ..LegacyOpenOpts::default()
    });
    profile.normalize_mode();
    profile.apply_reasoning().map_err(map_host_err)?;
    tracing::info!(
        profile_id = %profile.id,
        provider = %profile.provider.kind,
        model = %profile.model.name,
        "session_open"
    );
    Ok(ResolvedProfile {
        profile,
        api_key: opts.api_key.clone(),
        project_root: PathBuf::from(project_root),
    })
}

pub fn session_open(
    sqlite_path: String,
    project_root: String,
    opts: SessionOpenOpts,
) -> Result<AgentSession, String> {
    let _guard = rt().enter();
    let disk = kim_agent_host::session_file_from_sqlite_path(&sqlite_path);
    let session_id = if !opts.session_id.trim().is_empty() {
        opts.session_id.clone()
    } else if sqlite_path.trim().is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        sqlite_path
    };
    let host =
        AgentHost::from_resolved(resolved_from_opts(&opts, project_root)?).map_err(map_host_err)?;
    rt().block_on(async {
        host.configure_persist(disk, opts.resume_on_open).await;
        host.connect_extensions().await
    })
    .map_err(map_host_err)?;
    let (tx, _) = broadcast::channel(256);
    let _ = tx.send(AgentUiEvent::session_ready());
    Ok(AgentSession {
        inner: Arc::new(Shared {
            host: RwLock::new(host),
            session_id,
            events: tx,
            phase: Mutex::new(SessionPhase::Idle),
            replay: Mutex::new(Vec::new()),
            complete_gate: tokio::sync::Mutex::new(()),
            cancel: Mutex::new(None),
            generation: AtomicU64::new(0),
        }),
    })
}

impl AgentSession {
    fn begin_run(&self) -> Result<(CancellationToken, u64), String> {
        let mut phase = self
            .inner
            .phase
            .lock()
            .map_err(|_| "phase lock".to_string())?;
        match *phase {
            SessionPhase::Running => return Err("agent busy".into()),
            SessionPhase::Yielded => return Err("agent waiting for tool".into()),
            SessionPhase::Idle => {}
        }
        *phase = SessionPhase::Running;
        let gen = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let cancel = CancellationToken::new();
        if let Ok(mut g) = self.inner.cancel.lock() {
            *g = Some(cancel.clone());
        }
        Ok((cancel, gen))
    }

    pub fn prompt(&self, text: String) -> Result<String, String> {
        self.start_prompt(text, None)
    }

    pub fn prompt_with_context(
        &self,
        text: String,
        context_json: String,
    ) -> Result<String, String> {
        self.start_prompt(text, Some(context_json))
    }

    fn start_prompt(&self, text: String, context: Option<String>) -> Result<String, String> {
        let text = text.trim().to_string();
        if text.is_empty() {
            return Err("empty prompt".into());
        }
        let (cancel, gen) = self.begin_run()?;
        let inner = self.inner.clone();
        let (op_tx, op_rx) = std::sync::mpsc::channel();
        rt().spawn(async move {
            let op = uuid::Uuid::new_v4().to_string();
            let _ = inner
                .events
                .send(AgentUiEvent::operation_started(op.clone()));
            let _ = op_tx.send(Ok(op.clone()));
            let (tx, rx) = mpsc::channel(64);
            let pump = spawn_host_pump(inner.events.clone(), op.clone(), rx);
            let host = inner.host.read().await;
            let result = host
                .prompt_with_context(&inner.session_id, &text, context.as_deref(), tx, cancel)
                .await;
            drop(host);
            let _ = pump.await;
            finish_turn(&inner, op, result, gen).await;
        });
        op_rx
            .recv_timeout(Duration::from_secs(30))
            .map_err(|_| "prompt start timeout".to_string())?
    }

    pub fn complete_tool(&self, call_id: String, output_json: String) -> Result<String, String> {
        {
            let phase = self
                .inner
                .phase
                .lock()
                .map_err(|_| "phase lock".to_string())?;
            if *phase != SessionPhase::Yielded {
                return Err("unknown tool call".into());
            }
        }
        let inner = self.inner.clone();
        let gen = inner.generation.load(Ordering::SeqCst);
        let (op_tx, op_rx) = std::sync::mpsc::channel();
        let cancel = CancellationToken::new();
        if let Ok(mut g) = inner.cancel.lock() {
            *g = Some(cancel.clone());
        }
        rt().spawn(async move {
            let _gate = inner.complete_gate.lock().await;
            if inner.generation.load(Ordering::SeqCst) != gen {
                let _ = op_tx.send(Err("aborted".into()));
                return;
            }
            {
                let Ok(phase) = inner.phase.lock() else {
                    let _ = op_tx.send(Err("phase lock".into()));
                    return;
                };
                if *phase != SessionPhase::Yielded {
                    let _ = op_tx.send(Err("unknown tool call".into()));
                    return;
                }
            }
            let op = uuid::Uuid::new_v4().to_string();
            let _ = op_tx.send(Ok(op.clone()));
            let (tx, rx) = mpsc::channel(64);
            let pump = spawn_host_pump(inner.events.clone(), op.clone(), rx);
            let host = inner.host.read().await;
            let pending = host.pending_yields(&inner.session_id).await;
            if pending.len() == 1 && pending[0].call_id == call_id {
                if let Ok(mut g) = inner.phase.lock() {
                    if *g == SessionPhase::Yielded {
                        *g = SessionPhase::Running;
                    }
                }
            }
            let result = host
                .complete_tool(&inner.session_id, &call_id, &output_json, tx, cancel)
                .await;
            drop(host);
            let _ = pump.await;
            finish_turn(&inner, op, result, gen).await;
        });
        op_rx
            .recv_timeout(Duration::from_secs(30))
            .map_err(|_| "complete_tool start timeout".to_string())?
    }

    pub fn respond_permission(
        &self,
        call_id: String,
        permission: String,
    ) -> Result<String, String> {
        let parsed = parse_permission(&permission)?;
        {
            let phase = self
                .inner
                .phase
                .lock()
                .map_err(|_| "phase lock".to_string())?;
            if *phase != SessionPhase::Yielded {
                return Err("unknown tool call".into());
            }
        }
        let inner = self.inner.clone();
        let gen = inner.generation.load(Ordering::SeqCst);
        let (op_tx, op_rx) = std::sync::mpsc::channel();
        let cancel = CancellationToken::new();
        if let Ok(mut g) = inner.cancel.lock() {
            *g = Some(cancel.clone());
        }
        rt().spawn(async move {
            let _gate = inner.complete_gate.lock().await;
            if inner.generation.load(Ordering::SeqCst) != gen {
                let _ = op_tx.send(Err("aborted".into()));
                return;
            }
            {
                let Ok(phase) = inner.phase.lock() else {
                    let _ = op_tx.send(Err("phase lock".into()));
                    return;
                };
                if *phase != SessionPhase::Yielded {
                    let _ = op_tx.send(Err("unknown tool call".into()));
                    return;
                }
            }
            let op = uuid::Uuid::new_v4().to_string();
            let _ = op_tx.send(Ok(op.clone()));
            let (tx, rx) = mpsc::channel(64);
            let pump = spawn_host_pump(inner.events.clone(), op.clone(), rx);
            let host = inner.host.read().await;
            let pending = host.pending_yields(&inner.session_id).await;
            if pending.len() == 1 && pending[0].call_id == call_id {
                if let Ok(mut g) = inner.phase.lock() {
                    if *g == SessionPhase::Yielded {
                        *g = SessionPhase::Running;
                    }
                }
            }
            let result = host
                .respond_permission(&inner.session_id, &call_id, parsed, tx, cancel)
                .await;
            drop(host);
            let _ = pump.await;
            finish_turn(&inner, op, result, gen).await;
        });
        op_rx
            .recv_timeout(Duration::from_secs(30))
            .map_err(|_| "respond_permission start timeout".to_string())?
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn listen(&self, sink: StreamSink<AgentUiEvent>) -> Result<(), String> {
        let _guard = rt().enter();
        let replay = self
            .inner
            .replay
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let mut rx = self.inner.events.subscribe();
        rt().spawn(async move {
            for ev in replay {
                if sink.add(ev).is_err() {
                    return;
                }
            }
            loop {
                match rx.recv().await {
                    Ok(ev) => {
                        if sink.add(ev).is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Pending yield events live in `replay`; skip dropped live ticks.
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        Ok(())
    }

    pub fn abort(&self) -> Result<(), String> {
        if let Ok(g) = self.inner.cancel.lock() {
            if let Some(token) = g.as_ref() {
                token.cancel();
            }
        }
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        let inner = self.inner.clone();
        rt().block_on(async move {
            let _gate = inner.complete_gate.lock().await;
            let host = inner.host.read().await;
            host.cancel_pending_tools(&inner.session_id).await;
            drop(host);
            if let Ok(mut g) = inner.phase.lock() {
                *g = SessionPhase::Idle;
            }
            if let Ok(mut g) = inner.replay.lock() {
                g.clear();
            }
            let _ = inner.events.send(AgentUiEvent::aborted(String::new()));
        });
        Ok(())
    }

    pub fn resume(&self) -> Result<ResumeReportDto, String> {
        Ok(ResumeReportDto {
            resumed_ops: Vec::new(),
            statuses: Vec::new(),
        })
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn snapshot(&self) -> Result<SessionSnapshotDto, String> {
        let _guard = rt().enter();
        let phase = self
            .inner
            .phase
            .lock()
            .map(|g| *g)
            .unwrap_or(SessionPhase::Idle);
        let pending = rt().block_on(async {
            let host = self.inner.host.read().await;
            host.pending_yields(&self.inner.session_id).await
        });
        Ok(SessionSnapshotDto {
            busy: phase == SessionPhase::Running,
            last_operation_id: String::new(),
            phase: phase.as_str().to_string(),
            pending_call_ids: pending.into_iter().map(|p| p.call_id).collect(),
        })
    }

    pub fn reconfigure(&self, opts: SessionOpenOpts) -> Result<(), String> {
        let _guard = rt().enter();
        let host = AgentHost::from_resolved(resolved_from_opts(&opts, String::new())?)
            .map_err(map_host_err)?;
        rt().block_on(host.connect_extensions())
            .map_err(map_host_err)?;
        let inner = self.inner.clone();
        rt().block_on(async move {
            let mut slot = inner.host.write().await;
            slot.disconnect_extensions().await;
            *slot = host;
        });
        Ok(())
    }

    pub fn close(&self) -> Result<(), String> {
        let _ = self.abort();
        let inner = self.inner.clone();
        rt().block_on(async move {
            let host = inner.host.read().await;
            host.disconnect_extensions().await;
        });
        Ok(())
    }
}

fn spawn_host_pump(
    events: broadcast::Sender<AgentUiEvent>,
    op: String,
    mut rx: mpsc::Receiver<HostEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                HostEvent::TextDelta { delta } => {
                    let _ = events.send(AgentUiEvent::text_delta(op.clone(), delta));
                }
                HostEvent::Finished { text } => {
                    let _ = events.send(AgentUiEvent::completed(op.clone(), text));
                }
                HostEvent::Failed { message } => {
                    let _ = events.send(AgentUiEvent::failed(op.clone(), message));
                }
                HostEvent::ToolRequest { call_id, name, .. } => {
                    let _ = events.send(AgentUiEvent::tool_started(op.clone(), call_id, name));
                }
                HostEvent::ToolResult {
                    call_id,
                    name,
                    output_preview,
                    ok,
                } => {
                    let _ = events.send(AgentUiEvent::tool_finished(
                        op.clone(),
                        call_id,
                        name,
                        output_preview,
                        ok,
                    ));
                }
                HostEvent::ActionRequired { .. } | HostEvent::Usage { .. } => {}
            }
        }
    })
}

fn turn_is_current(inner: &Shared, gen: u64) -> bool {
    inner.generation.load(Ordering::SeqCst) == gen
}

fn set_phase_if_current(inner: &Shared, gen: u64, next: SessionPhase) -> bool {
    if !turn_is_current(inner, gen) {
        return false;
    }
    let Ok(mut g) = inner.phase.lock() else {
        return false;
    };
    if !turn_is_current(inner, gen) {
        return false;
    }
    *g = next;
    true
}

async fn finish_turn(inner: &Shared, op: String, result: Result<TurnOutcome, HostError>, gen: u64) {
    if !turn_is_current(inner, gen) {
        return;
    }
    match result {
        Ok(TurnOutcome::Finished { .. }) => {
            if !set_phase_if_current(inner, gen, SessionPhase::Idle) {
                return;
            }
            if let Ok(mut g) = inner.replay.lock() {
                g.clear();
            }
        }
        Ok(TurnOutcome::Yielded { .. }) => {
            let host = inner.host.read().await;
            let pending = host.pending_yields(&inner.session_id).await;
            drop(host);
            if !set_phase_if_current(inner, gen, SessionPhase::Yielded) {
                return;
            }
            let mut replayed = Vec::new();
            for p in pending {
                if !turn_is_current(inner, gen) {
                    return;
                }
                let ev = match p.kind {
                    YieldKind::ActionRequired => AgentUiEvent::action_required(
                        op.clone(),
                        p.call_id,
                        p.name,
                        p.arguments_json,
                        p.prompt,
                    ),
                    YieldKind::ToolRequest => {
                        AgentUiEvent::tool_request(op.clone(), p.call_id, p.name, p.arguments_json)
                    }
                };
                let _ = inner.events.send(ev.clone());
                replayed.push(ev);
            }
            if !turn_is_current(inner, gen) {
                return;
            }
            if let Ok(mut g) = inner.replay.lock() {
                *g = replayed;
            }
        }
        Err(HostError::Failed(msg)) if msg.contains("cancel") => {
            if !set_phase_if_current(inner, gen, SessionPhase::Idle) {
                return;
            }
            let _ = inner.events.send(AgentUiEvent::aborted(op));
        }
        Err(err) => {
            if !set_phase_if_current(inner, gen, SessionPhase::Idle) {
                return;
            }
            let _ = inner
                .events
                .send(AgentUiEvent::failed(op, map_host_err(err)));
        }
    }
}

pub fn fetch_supported_models(opts: SessionOpenOpts) -> Result<Vec<String>, String> {
    let _guard = rt().enter();
    let spec = ProviderSpec {
        kind: opts.llm_backend.clone(),
        base_url: opts.base_url.clone(),
        key_ref: String::new(),
    };
    rt().block_on(async {
        kim_agent_host::fetch_models(&spec, &opts.api_key)
            .await
            .map_err(|e| e.to_string())
    })
}

pub fn list_builtin_profiles() -> Result<Vec<String>, String> {
    kim_agent_host::builtin_templates()
        .into_iter()
        .map(|p| serde_json::to_string(&p).map_err(|e| e.to_string()))
        .collect()
}

pub fn list_bundled_providers() -> Result<Vec<String>, String> {
    Ok(kim_agent_host::bundled_provider_summaries()
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "display_name": s.display_name,
                "mobile": s.mobile,
            })
            .to_string()
        })
        .collect())
}

pub fn catalog_vendors() -> Result<String, String> {
    kim_agent_host::catalog_vendors_json().map_err(map_host_err)
}

pub fn catalog_surface(vendor: String, model: String) -> Result<String, String> {
    kim_agent_host::catalog_surface_json(&vendor, &model).map_err(map_host_err)
}

pub fn catalog_validate(vendor: String, model: String, choice_json: String) -> Result<(), String> {
    kim_agent_host::catalog_validate(&vendor, &model, &choice_json).map_err(map_host_err)
}

fn map_host_err(err: HostError) -> String {
    err.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_agent_host::{AgentProfile, LegacyOpenOpts, ScriptedProvider};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    fn session_from_host(session_id: String, host: AgentHost) -> AgentSession {
        let (tx, _) = broadcast::channel(256);
        AgentSession {
            inner: Arc::new(Shared {
                host: RwLock::new(host),
                session_id,
                events: tx,
                phase: Mutex::new(SessionPhase::Idle),
                replay: Mutex::new(Vec::new()),
                complete_gate: tokio::sync::Mutex::new(()),
                cancel: Mutex::new(None),
                generation: AtomicU64::new(0),
            }),
        }
    }

    fn wait_phase(session: &AgentSession, want: &str) {
        for _ in 0..100 {
            let snap = session.snapshot().unwrap();
            if snap.phase == want {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "timed out waiting for phase {want}, got {}",
            session.snapshot().unwrap().phase
        );
    }

    fn wait_pending(session: &AgentSession, want: &[&str]) {
        let want: Vec<String> = want.iter().map(|s| (*s).to_string()).collect();
        for _ in 0..100 {
            let snap = session.snapshot().unwrap();
            if snap.pending_call_ids == want {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "timed out waiting for pending {want:?}, got {:?}",
            session.snapshot().unwrap().pending_call_ids
        );
    }

    #[test]
    fn search_contacts_yields_before_tool_request() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        let host = AgentHost::from_provider_for_test(
            profile,
            Arc::new(ScriptedProvider::kim_search_contacts(&[("c1", "bob")])),
            PathBuf::from("/tmp"),
        )
        .unwrap();
        let session = session_from_host("s".into(), host);
        let mut rx = session.inner.events.subscribe();
        session.prompt("find bob".into()).unwrap();
        wait_phase(&session, "yielded");
        let snap = session.snapshot().unwrap();
        assert!(!snap.busy);
        assert_eq!(snap.pending_call_ids, vec!["c1".to_string()]);
        let mut saw_request = false;
        while let Ok(ev) = rx.try_recv() {
            if ev.kind == "tool_request" {
                saw_request = true;
                assert_eq!(ev.call_id, "c1");
            }
        }
        assert!(saw_request);
        session
            .complete_tool("c1".into(), r#"{"people":[]}"#.into())
            .unwrap();
        wait_phase(&session, "idle");
        session.prompt("thanks".into()).unwrap();
        wait_phase(&session, "idle");
        assert_eq!(session.snapshot().unwrap().phase, "idle");
        assert!(!session.snapshot().unwrap().busy);
    }

    #[test]
    fn two_tool_requests_serial_complete() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        let host = AgentHost::from_provider_for_test(
            profile,
            Arc::new(ScriptedProvider::kim_search_contacts(&[
                ("c1", "a"),
                ("c2", "b"),
            ])),
            PathBuf::from("/tmp"),
        )
        .unwrap();
        let session = session_from_host("s".into(), host);
        session.prompt("find".into()).unwrap();
        wait_phase(&session, "yielded");
        assert_eq!(session.snapshot().unwrap().pending_call_ids.len(), 2);
        session
            .complete_tool("c1".into(), r#"{"people":[]}"#.into())
            .unwrap();
        wait_phase(&session, "yielded");
        wait_pending(&session, &["c2"]);
        session
            .complete_tool("c2".into(), r#"{"people":[]}"#.into())
            .unwrap();
        wait_phase(&session, "idle");
    }

    #[test]
    fn send_message_action_required_after_yielded() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            enable_approvals: true,
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        let host = AgentHost::from_provider_for_test(
            profile,
            Arc::new(ScriptedProvider::kim_send_message("c1", "bob", "hi")),
            PathBuf::from("/tmp"),
        )
        .unwrap();
        let session = session_from_host("s".into(), host);
        let mut rx = session.inner.events.subscribe();
        session.prompt("ping bob".into()).unwrap();
        wait_phase(&session, "yielded");
        let snap = session.snapshot().unwrap();
        assert!(!snap.busy);
        assert_eq!(snap.pending_call_ids, vec!["c1".to_string()]);
        let mut saw = false;
        while let Ok(ev) = rx.try_recv() {
            if ev.kind == "action_required" {
                saw = true;
                assert_eq!(ev.call_id, "c1");
                assert_eq!(ev.name, "send_message");
            }
            assert_ne!(ev.kind, "tool_request");
        }
        assert!(saw);
        session
            .respond_permission("c1".into(), "allow_once".into())
            .unwrap();
        let mut saw_tool = false;
        for _ in 0..100 {
            while let Ok(ev) = rx.try_recv() {
                if ev.kind == "tool_request" {
                    saw_tool = true;
                    assert_eq!(ev.call_id, "c1");
                }
            }
            if saw_tool {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(saw_tool);
    }

    #[test]
    fn abort_keeps_idle_and_ignores_finish_turn() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        let host = AgentHost::from_provider_for_test(
            profile,
            Arc::new(ScriptedProvider::kim_search_contacts(&[("c1", "bob")])),
            PathBuf::from("/tmp"),
        )
        .unwrap();
        let session = session_from_host("s".into(), host);
        let mut rx = session.inner.events.subscribe();
        session.prompt("find bob".into()).unwrap();
        wait_phase(&session, "yielded");
        while rx.try_recv().is_ok() {}
        session.abort().unwrap();
        assert_eq!(session.snapshot().unwrap().phase, "idle");
        thread::sleep(Duration::from_millis(80));
        assert_eq!(session.snapshot().unwrap().phase, "idle");
        assert!(session
            .complete_tool("c1".into(), r#"{"people":[]}"#.into())
            .is_err());
        while let Ok(ev) = rx.try_recv() {
            assert_ne!(ev.kind, "tool_request");
            assert_ne!(ev.kind, "assistant_finished");
        }
        session.prompt("again".into()).unwrap();
        wait_phase(&session, "idle");
    }
}
