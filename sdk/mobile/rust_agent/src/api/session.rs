//! Thin FFI over `kim-agent-host` (Goose). Isolated from `kim_client_ffi`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kim_agent_host::{
    parse_permission, resolve_limits, AgentHost, AgentProfile, CancelReason, HostError, HostEvent,
    LegacyOpenOpts, ProviderSpec, ResolvedProfile, TimeoutKind, TurnOutcome, YieldKind,
};

use super::phase::{self, PhaseInput, SessionPhase};
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
    pub harness_json: String,
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
            harness_json: String::new(),
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
    pub recently_active: bool,
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
            recently_active: false,
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

struct Shared {
    host: RwLock<AgentHost>,
    session_id: String,
    events: broadcast::Sender<AgentUiEvent>,
    phase: Mutex<SessionPhase>,
    replay: Mutex<Vec<AgentUiEvent>>,
    complete_gate: tokio::sync::Mutex<()>,
    cancel: Mutex<Option<CancellationToken>>,
    generation: AtomicU64,
    limits: kim_agent_host::HarnessLimits,
    yield_cancel: Mutex<Option<CancellationToken>>,
    resolved: Mutex<Option<ResolvedProfile>>,
    recreate_attempts: AtomicU64,
}

pub struct AgentSession {
    inner: Arc<Shared>,
}

fn resolved_from_opts(
    opts: &SessionOpenOpts,
    project_root: String,
) -> Result<ResolvedProfile, String> {
    let mut profile = if opts.profile_json.trim().is_empty() {
        if !opts.profile_id.trim().is_empty() {
            return Err("profile_json required when profile_id is set".into());
        }
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
    let resolved = resolved_from_opts(&opts, project_root)?;
    let limits = resolve_limits(&opts.harness_json, resolved.profile.harness.as_ref())
        .map_err(map_host_err)?;
    let host = AgentHost::from_resolved(resolved.clone())
        .map_err(map_host_err)?
        .with_limits(limits);
    rt().block_on(async {
        host.configure_persist(disk, opts.resume_on_open).await;
        host.connect_extensions().await
    })
    .map_err(map_host_err)?;
    let shared = shared_new(host, session_id, Some(resolved));
    let _ = shared.events.send(AgentUiEvent::session_ready());
    Ok(AgentSession {
        inner: Arc::new(shared),
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
        if phase::transition(*phase, PhaseInput::Prompt).is_none() {
            return Err("agent busy".into());
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
            let _gate = inner.complete_gate.lock().await;
            if inner.generation.load(Ordering::SeqCst) != gen {
                let _ = op_tx.send(Err("aborted".into()));
                return;
            }
            let op = uuid::Uuid::new_v4().to_string();
            let _ = inner
                .events
                .send(AgentUiEvent::operation_started(op.clone()));
            let _ = op_tx.send(Ok(op.clone()));
            let (tx, rx) = mpsc::channel(64);
            let pump = spawn_host_pump(inner.events.clone(), op.clone(), rx);
            let host = {
                let guard = inner.host.read().await;
                guard.clone()
            };
            let result = host
                .prompt_with_context(&inner.session_id, &text, context.as_deref(), tx, cancel)
                .await;
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
            let host = {
                let guard = inner.host.read().await;
                guard.clone()
            };
            let pending = host.pending_yields(&inner.session_id).await;
            if pending.len() == 1 && pending[0].call_id == call_id {
                disarm_yield_watch(&inner);
                if let Ok(mut g) = inner.phase.lock() {
                    if phase::transition(*g, PhaseInput::Continue).is_some() {
                        *g = SessionPhase::Running;
                    }
                }
            }
            let result = host
                .complete_tool(&inner.session_id, &call_id, &output_json, tx, cancel)
                .await;
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
            let host = {
                let guard = inner.host.read().await;
                guard.clone()
            };
            let pending = host.pending_yields(&inner.session_id).await;
            if pending.len() == 1 && pending[0].call_id == call_id {
                disarm_yield_watch(&inner);
                if let Ok(mut g) = inner.phase.lock() {
                    if phase::transition(*g, PhaseInput::Continue).is_some() {
                        *g = SessionPhase::Running;
                    }
                }
            }
            let result = host
                .respond_permission(&inner.session_id, &call_id, parsed, tx, cancel)
                .await;
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
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        disarm_yield_watch(&self.inner);
        if let Ok(g) = self.inner.cancel.lock() {
            if let Some(token) = g.as_ref() {
                token.cancel();
            }
        }
        let inner = self.inner.clone();
        rt().block_on(async move {
            let grace = inner.limits.cancel_grace;
            match tokio::time::timeout(grace, inner.complete_gate.lock()).await {
                Ok(_gate) => {
                    let host = {
                        let guard = inner.host.read().await;
                        guard.clone()
                    };
                    host.cancel_pending_tools(&inner.session_id).await;
                    if let Ok(mut g) = inner.phase.lock() {
                        *g = SessionPhase::Idle;
                    }
                    if let Ok(mut g) = inner.replay.lock() {
                        g.clear();
                    }
                    let mut ev = AgentUiEvent::aborted(String::new());
                    ev.stop_reason = "cancelled".into();
                    let _ = inner.events.send(ev);
                    Ok(())
                }
                Err(_) => {
                    let _ = recreate_host(&inner).await;
                    if let Ok(mut g) = inner.phase.lock() {
                        *g = SessionPhase::Idle;
                    }
                    if let Ok(mut g) = inner.replay.lock() {
                        g.clear();
                    }
                    let mut ev = AgentUiEvent::failed(String::new(), "host poisoned".into());
                    ev.stop_reason = "poisoned".into();
                    let _ = inner.events.send(ev);
                    Ok(())
                }
            }
        })
    }

    pub fn steer(&self, text: String) -> Result<(), String> {
        let inner = self.inner.clone();
        rt().block_on(async move {
            let host = {
                let guard = inner.host.read().await;
                guard.clone()
            };
            host.steer(&inner.session_id, &text)
                .await
                .map_err(map_host_err)
        })
    }

    pub fn resume(&self) -> Result<ResumeReportDto, String> {
        let inner = self.inner.clone();
        rt().block_on(async move {
            let _gate = inner.complete_gate.lock().await;
            {
                let phase = inner.phase.lock().map_err(|_| "phase lock".to_string())?;
                if *phase != SessionPhase::Idle {
                    return Err("session is not idle".into());
                }
            }
            let host = {
                let guard = inner.host.read().await;
                guard.clone()
            };
            host.repair_in_process_pairing(&inner.session_id).await;
            let pending = host.pending_yields(&inner.session_id).await;
            if pending.is_empty() {
                return Ok(ResumeReportDto {
                    resumed_ops: Vec::new(),
                    statuses: Vec::new(),
                });
            }
            if let Ok(mut g) = inner.phase.lock() {
                *g = SessionPhase::Yielded;
            }
            let op = "resume".to_string();
            let mut resumed_ops = Vec::new();
            let mut statuses = Vec::new();
            let mut replayed = Vec::new();
            for p in pending {
                let status = match p.kind {
                    YieldKind::ActionRequired => "action_required",
                    YieldKind::ToolRequest => "tool_request",
                };
                let ev = match p.kind {
                    YieldKind::ActionRequired => AgentUiEvent::action_required(
                        op.clone(),
                        p.call_id.clone(),
                        p.name,
                        p.arguments_json,
                        p.prompt,
                    ),
                    YieldKind::ToolRequest => AgentUiEvent::tool_request(
                        op.clone(),
                        p.call_id.clone(),
                        p.name,
                        p.arguments_json,
                    ),
                };
                let _ = inner.events.send(ev.clone());
                replayed.push(ev);
                resumed_ops.push(p.call_id);
                statuses.push(status.to_string());
            }
            if let Ok(mut g) = inner.replay.lock() {
                *g = replayed;
            }
            let gen = inner.generation.load(Ordering::SeqCst);
            arm_yield_watch(&inner, gen);
            Ok(ResumeReportDto {
                resumed_ops,
                statuses,
            })
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
            let host = {
                let guard = self.inner.host.read().await;
                guard.clone()
            };
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
        let resolved = resolved_from_opts(&opts, String::new())?;
        let inner = self.inner.clone();
        rt().block_on(async move {
            if let Ok(mut slot) = inner.resolved.lock() {
                *slot = Some(resolved.clone());
            }
            {
                let slot = inner.host.read().await;
                let current = slot.profile_snapshot();
                if current.provider.kind == resolved.profile.provider.kind
                    && current.provider.base_url == resolved.profile.provider.base_url
                    && current.model.name == resolved.profile.model.name
                {
                    slot.replace_prompt_steer(
                        resolved.profile.system_prompt,
                        resolved.profile.steer,
                    );
                    return Ok(());
                }
            }
            let host = AgentHost::from_resolved(resolved.clone())
                .map_err(map_host_err)?
                .with_limits(
                    resolve_limits(&opts.harness_json, resolved.profile.harness.as_ref())
                        .unwrap_or_else(|_| kim_agent_host::HarnessLimits::disabled()),
                );
            host.connect_extensions().await.map_err(map_host_err)?;
            let mut slot = inner.host.write().await;
            let state = slot.runtime_state().await;
            host.restore_runtime_state(state).await;
            slot.disconnect_extensions().await;
            *slot = host;
            Ok(())
        })
    }

    pub fn close(&self) -> Result<(), String> {
        let _ = self.abort();
        let inner = self.inner.clone();
        rt().block_on(async move {
            let host = {
                let guard = inner.host.read().await;
                guard.clone()
            };
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
                HostEvent::Usage {
                    input_tokens,
                    output_tokens,
                } => {
                    let mut ev = AgentUiEvent::base("usage");
                    ev.operation_id = op.clone();
                    ev.input_tokens = input_tokens;
                    ev.output_tokens = output_tokens;
                    let _ = events.send(ev);
                }
                HostEvent::Keepalive => {
                    let mut ev = AgentUiEvent::base("keepalive");
                    ev.operation_id = op.clone();
                    let _ = events.send(ev);
                }
                HostEvent::ActionRequired { .. } => {}
            }
        }
    })
}

fn turn_is_current(inner: &Shared, gen: u64) -> bool {
    !phase::stale(inner.generation.load(Ordering::SeqCst), gen)
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

async fn finish_turn(
    inner: &Arc<Shared>,
    op: String,
    result: Result<TurnOutcome, HostError>,
    gen: u64,
) {
    if !turn_is_current(inner, gen) {
        return;
    }
    match result {
        Ok(TurnOutcome::Finished {
            text,
            replied,
            visible,
        }) => {
            disarm_yield_watch(inner);
            if !set_phase_if_current(inner, gen, SessionPhase::Idle) {
                return;
            }
            if let Ok(mut g) = inner.replay.lock() {
                g.clear();
            }
            let mut ev = AgentUiEvent::completed(op, text);
            ev.ok = replied;
            ev.stop_reason = if replied {
                "completed"
            } else if visible {
                "side_effect"
            } else {
                "empty"
            }
            .into();
            let _ = inner.events.send(ev);
        }
        Ok(TurnOutcome::Yielded { .. }) => {
            let host = {
                let guard = inner.host.read().await;
                guard.clone()
            };
            let pending = host.pending_yields(&inner.session_id).await;
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
            arm_yield_watch(inner, gen);
        }
        Ok(TurnOutcome::Cancelled { reason }) => {
            disarm_yield_watch(inner);
            if !set_phase_if_current(inner, gen, SessionPhase::Idle) {
                return;
            }
            if let Ok(mut g) = inner.replay.lock() {
                g.clear();
            }
            let mut ev = AgentUiEvent::aborted(op);
            ev.stop_reason = match reason {
                CancelReason::UserAbort => "cancelled",
                CancelReason::YieldAbandoned => "yield_abandoned",
            }
            .into();
            let _ = inner.events.send(ev);
        }
        Ok(TurnOutcome::TimedOut { kind }) => {
            disarm_yield_watch(inner);
            if !set_phase_if_current(inner, gen, SessionPhase::Idle) {
                return;
            }
            let mut ev = AgentUiEvent::failed(op, String::new());
            ev.stop_reason = match kind {
                TimeoutKind::Idle => "idle_timeout".into(),
                TimeoutKind::Hard { recently_active } => {
                    ev.recently_active = recently_active;
                    "hard_timeout".into()
                }
            };
            let _ = inner.events.send(ev);
        }
        Err(HostError::Poisoned {
            message,
            recently_active,
        }) => {
            disarm_yield_watch(inner);
            if !set_phase_if_current(inner, gen, SessionPhase::Idle) {
                return;
            }
            if let Ok(mut g) = inner.replay.lock() {
                g.clear();
            }
            let _ = recreate_host(inner).await;
            let mut ev = AgentUiEvent::failed(op, message);
            ev.stop_reason = "poisoned".into();
            ev.recently_active = recently_active;
            let _ = inner.events.send(ev);
        }
        Err(HostError::Provider(fail)) => {
            disarm_yield_watch(inner);
            if !set_phase_if_current(inner, gen, SessionPhase::Idle) {
                return;
            }
            let mut ev = AgentUiEvent::failed(op, fail.to_string());
            ev.stop_reason = "provider".into();
            let _ = inner.events.send(ev);
        }
        Err(err) => {
            disarm_yield_watch(inner);
            if !set_phase_if_current(inner, gen, SessionPhase::Idle) {
                return;
            }
            let mut ev = AgentUiEvent::failed(op, map_host_err(err));
            ev.stop_reason = "failed".into();
            let _ = inner.events.send(ev);
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

pub fn catalog_validate(
    vendor: String,
    model: String,
    choice_json: String,
) -> Result<String, String> {
    kim_agent_host::catalog_validate(&vendor, &model, &choice_json).map_err(map_host_err)
}

/// Portable skills under the user shelf and/or `<project>/.agents/skills`.
pub fn skill_portable_list(user_root: String, project_root: String) -> Result<String, String> {
    Ok(kim_agent_host::skill_portable_list_json(
        &user_root,
        &project_root,
    ))
}

/// Bundled (and optional cache) `kim-*` app skill summaries for assignment UI.
pub fn skill_app_catalog(cache_root: String) -> Result<String, String> {
    let root = cache_root.trim();
    let path = if root.is_empty() {
        None
    } else {
        Some(std::path::Path::new(root))
    };
    Ok(kim_agent_host::skill_app_catalog_json(path))
}

/// Preview assembled tools + layered prompts for a profile (no network).
pub fn preview_assembled(profile_json: String, project_root: String) -> Result<String, String> {
    let profile: AgentProfile =
        serde_json::from_str(&profile_json).map_err(|e| format!("profile_json: {e}"))?;
    let preview = kim_agent_host::preview_assembled(&profile, std::path::Path::new(&project_root))
        .map_err(map_host_err)?;
    serde_json::to_string(&preview).map_err(|e| e.to_string())
}

/// Registered capability kinds + risk + param_schema for UI cards.
pub fn capability_catalog_json() -> Result<String, String> {
    let entries = kim_agent_host::capability::catalog_entries();
    serde_json::to_string(&entries).map_err(|e| e.to_string())
}

fn map_host_err(err: HostError) -> String {
    err.to_string()
}

fn shared_new(host: AgentHost, session_id: String, resolved: Option<ResolvedProfile>) -> Shared {
    let limits = host.limits();
    let (tx, _) = broadcast::channel(256);
    Shared {
        host: RwLock::new(host),
        session_id,
        events: tx,
        phase: Mutex::new(SessionPhase::Idle),
        replay: Mutex::new(Vec::new()),
        complete_gate: tokio::sync::Mutex::new(()),
        cancel: Mutex::new(None),
        generation: AtomicU64::new(0),
        limits,
        yield_cancel: Mutex::new(None),
        resolved: Mutex::new(resolved),
        recreate_attempts: AtomicU64::new(0),
    }
}

fn disarm_yield_watch(inner: &Shared) {
    if let Ok(mut slot) = inner.yield_cancel.lock() {
        if let Some(token) = slot.take() {
            token.cancel();
        }
    }
}

fn arm_yield_watch(inner: &Arc<Shared>, gen: u64) {
    disarm_yield_watch(inner);
    let token = CancellationToken::new();
    if let Ok(mut slot) = inner.yield_cancel.lock() {
        *slot = Some(token.clone());
    }
    let wait = inner.limits.yield_wait;
    let inner = Arc::clone(inner);
    tokio::spawn(async move {
        tokio::select! {
            _ = token.cancelled() => {}
            _ = sleep_bounded(wait) => {
                if turn_is_current(&inner, gen) {
                    abandon_yield(inner).await;
                }
            }
        }
    });
}

async fn abandon_yield(inner: Arc<Shared>) {
    let _gate = inner.complete_gate.lock().await;
    {
        let Ok(phase) = inner.phase.lock() else {
            return;
        };
        if *phase != SessionPhase::Yielded {
            return;
        }
    }
    inner.generation.fetch_add(1, Ordering::SeqCst);
    if let Ok(slot) = inner.cancel.lock() {
        if let Some(token) = slot.as_ref() {
            token.cancel();
        }
    }
    let host = {
        let guard = inner.host.read().await;
        guard.clone()
    };
    host.repair_pairing(&inner.session_id).await;
    if let Ok(mut phase) = inner.phase.lock() {
        *phase = SessionPhase::Idle;
    }
    if let Ok(mut replay) = inner.replay.lock() {
        replay.clear();
    }
    let mut ev = AgentUiEvent::aborted(String::new());
    ev.stop_reason = "yield_abandoned".into();
    let _ = inner.events.send(ev);
}

async fn recreate_host(inner: &Shared) -> Result<(), String> {
    inner.recreate_attempts.fetch_add(1, Ordering::SeqCst);
    let resolved = inner
        .resolved
        .lock()
        .map_err(|_| "resolved lock".to_string())?
        .clone();
    let Some(resolved) = resolved else {
        return Ok(());
    };
    let host = AgentHost::from_resolved(resolved)
        .map_err(map_host_err)?
        .with_limits(inner.limits.clone());
    let old = {
        let guard = inner.host.read().await;
        guard.clone()
    };
    let state = old.runtime_state().await;
    host.restore_runtime_state(state).await;
    let _ = host.connect_extensions().await;
    host.repair_pairing(&inner.session_id).await;
    let mut slot = inner.host.write().await;
    slot.disconnect_extensions().await;
    *slot = host;
    Ok(())
}

async fn sleep_bounded(d: Duration) {
    const CAP: Duration = Duration::from_secs(24 * 60 * 60);
    if d >= CAP {
        std::future::pending::<()>().await;
    } else if !d.is_zero() {
        tokio::time::sleep(d).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_agent_host::{AgentProfile, LegacyOpenOpts, ScriptedProvider};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    fn session_from_host(session_id: String, host: AgentHost) -> AgentSession {
        AgentSession {
            inner: Arc::new(shared_new(host, session_id, None)),
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
        let mut profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            enable_approvals: true,
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        profile.permissions.tools.insert(
            "send_message".into(),
            kim_agent_host::PermissionDefault::AskBefore,
        );
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

    #[test]
    fn finish_turn_maps_stop_reason_without_string_scan() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        let host = AgentHost::from_provider_for_test(
            profile,
            Arc::new(ScriptedProvider::saying("hi")),
            PathBuf::from("/tmp"),
        )
        .unwrap();
        let session = session_from_host("s".into(), host);
        let mut rx = session.inner.events.subscribe();
        let cases = [
            (
                Ok(TurnOutcome::Finished {
                    text: "hi".into(),
                    replied: true,
                    visible: true,
                }),
                "assistant_finished",
                "completed",
                false,
            ),
            (
                Ok(TurnOutcome::Finished {
                    text: String::new(),
                    replied: false,
                    visible: true,
                }),
                "assistant_finished",
                "side_effect",
                false,
            ),
            (
                Ok(TurnOutcome::Finished {
                    text: String::new(),
                    replied: false,
                    visible: false,
                }),
                "assistant_finished",
                "empty",
                false,
            ),
            (
                Ok(TurnOutcome::TimedOut {
                    kind: TimeoutKind::Idle,
                }),
                "failed",
                "idle_timeout",
                false,
            ),
            (
                Ok(TurnOutcome::TimedOut {
                    kind: TimeoutKind::Hard {
                        recently_active: true,
                    },
                }),
                "failed",
                "hard_timeout",
                true,
            ),
            (
                Ok(TurnOutcome::Cancelled {
                    reason: CancelReason::UserAbort,
                }),
                "aborted",
                "cancelled",
                false,
            ),
            (
                Err(HostError::Poisoned {
                    message: "boom".into(),
                    recently_active: true,
                }),
                "failed",
                "poisoned",
                true,
            ),
            (
                Err(HostError::Failed("please cancel".into())),
                "failed",
                "failed",
                false,
            ),
        ];
        for (result, kind, stop, recent) in cases {
            rt().block_on(finish_turn(&session.inner, "op".into(), result, 0));
            let mut matched = false;
            while let Ok(ev) = rx.try_recv() {
                if ev.operation_id == "op" || ev.stop_reason == stop {
                    assert_eq!(ev.kind, kind, "{stop}");
                    assert_eq!(ev.stop_reason, stop);
                    assert_eq!(ev.recently_active, recent, "{stop}");
                    matched = true;
                }
            }
            assert!(matched, "missing {stop}");
        }
    }

    #[test]
    fn listen_emits_exactly_one_terminal_event_per_turn() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        let host = AgentHost::from_provider_for_test(
            profile,
            Arc::new(ScriptedProvider::saying("hello")),
            PathBuf::from("/tmp"),
        )
        .unwrap();
        let session = session_from_host("s".into(), host);
        let mut rx = session.inner.events.subscribe();
        session.prompt("hello".into()).unwrap();
        wait_phase(&session, "idle");
        thread::sleep(Duration::from_millis(40));
        let mut terminals = 0;
        while let Ok(ev) = rx.try_recv() {
            if matches!(
                ev.kind.as_str(),
                "assistant_finished" | "failed" | "aborted"
            ) {
                terminals += 1;
                assert_eq!(ev.kind, "assistant_finished");
                assert_eq!(ev.stop_reason, "completed");
            }
        }
        assert_eq!(terminals, 1);
    }

    #[test]
    fn yield_wait_abandon_cancels_permission() {
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
        .unwrap()
        .with_limits(kim_agent_host::HarnessLimits {
            yield_wait: Duration::from_millis(50),
            ..kim_agent_host::HarnessLimits::default()
        });
        let session = session_from_host("s".into(), host);
        let mut rx = session.inner.events.subscribe();
        session.prompt("ping bob".into()).unwrap();
        wait_phase(&session, "yielded");
        wait_phase(&session, "idle");
        let mut saw = false;
        while let Ok(ev) = rx.try_recv() {
            if ev.stop_reason == "yield_abandoned" {
                saw = true;
                assert_eq!(ev.kind, "aborted");
            }
        }
        assert!(saw);
        assert!(session
            .respond_permission("c1".into(), "allow_once".into())
            .is_err());
    }

    #[test]
    fn resume_reports_pending_yields() {
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
        session.prompt("find bob".into()).unwrap();
        wait_phase(&session, "yielded");
        if let Ok(mut phase) = session.inner.phase.lock() {
            *phase = SessionPhase::Idle;
        }
        let report = session.resume().unwrap();
        assert_eq!(report.resumed_ops, vec!["c1".to_string()]);
        assert_eq!(report.statuses, vec!["tool_request".to_string()]);
        assert_eq!(session.snapshot().unwrap().phase, "yielded");
    }

    #[test]
    fn resume_rejects_when_not_idle() {
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
        session.prompt("find bob".into()).unwrap();
        wait_phase(&session, "yielded");
        let err = match session.resume() {
            Ok(_) => panic!("expected resume to reject non-idle"),
            Err(e) => e,
        };
        assert!(err.contains("not idle"), "{err}");
        assert_eq!(session.snapshot().unwrap().phase, "yielded");
    }

    #[test]
    fn finish_turn_poisoned_attempts_recreate() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        });
        let host = AgentHost::from_provider_for_test(
            profile,
            Arc::new(ScriptedProvider::saying("hi")),
            PathBuf::from("/tmp"),
        )
        .unwrap();
        let session = session_from_host("s".into(), host);
        let mut rx = session.inner.events.subscribe();
        rt().block_on(finish_turn(
            &session.inner,
            "op".into(),
            Err(HostError::Poisoned {
                message: "boom".into(),
                recently_active: true,
            }),
            0,
        ));
        assert_eq!(session.inner.recreate_attempts.load(Ordering::SeqCst), 1);
        let mut saw = false;
        while let Ok(ev) = rx.try_recv() {
            if ev.stop_reason == "poisoned" {
                saw = true;
                assert_eq!(ev.kind, "failed");
                assert!(ev.recently_active);
            }
        }
        assert!(saw);
    }
}
