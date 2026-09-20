//! Codex session held by `AgentHost` when `runtime = codex`.
//!
//! One host, one thread. Goose `run_loop` is not on this path. Rollouts are
//! written (`ephemeral = false`), but a new process starts a new thread:
//! `resume_thread_*` rebuilds `StartThreadOptions` without `dynamic_tools`,
//! which would drop the KIM tools.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_core_api::CodexThread;
use codex_core_api::EventMsg;
use codex_core_api::Op;
use codex_core_api::StartIfIdleSubmission;
use codex_core_api::ThreadId;
use codex_core_api::ThreadManager;
use codex_core_api::TurnInputRequest;
use codex_core_api::UserInput;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::protocol::ReviewDecision;
use goose_provider_types::permission::Permission;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::codex_inject::apply_profile;
use crate::codex_inject::confirms_in_app;
use crate::codex_inject::kim_dynamic_tools;
use crate::codex_rt::embed_config;
use crate::codex_rt::start_thread;
use crate::codex_rt::CodexEmbedOpts;
use crate::events::CancelReason;
use crate::events::HostError;
use crate::events::HostEvent;
use crate::events::PendingYield;
use crate::events::TimeoutKind;
use crate::events::TurnOutcome;
use crate::events::YieldKind;
use crate::truncate_chars;
use crate::AgentHost;

pub struct CodexLaunch {
    pub codex_home: PathBuf,
    pub helper: PathBuf,
    pub linux_sandbox: Option<PathBuf>,
    pub api_key: String,
    /// Append-only chat memory. Each `session_open` is a new thread; this file
    /// is what the next message sees. Not `auth.json`.
    pub transcript: Option<PathBuf>,
}

pub(crate) struct CodexSlot {
    launch: Option<CodexLaunch>,
    live: Option<LiveThread>,
    pending: Option<PendingCodex>,
    turns: u32,
    hard_deadline: Option<Instant>,
    send_ok: bool,
}

struct LiveThread {
    #[allow(dead_code)]
    manager: ThreadManager,
    thread: Arc<CodexThread>,
    #[allow(dead_code)]
    thread_id: ThreadId,
}

enum PendingCodex {
    Confirm {
        call_id: String,
        name: String,
        arguments_json: String,
        prompt: String,
    },
    Execute {
        call_id: String,
        name: String,
        arguments_json: String,
    },
    Exec {
        id: String,
        turn_id: Option<String>,
        call_id: String,
        arguments_json: String,
        prompt: String,
    },
    Patch {
        id: String,
        call_id: String,
        arguments_json: String,
        prompt: String,
    },
}

impl CodexSlot {
    pub(crate) fn new() -> Self {
        Self {
            launch: None,
            live: None,
            pending: None,
            turns: 0,
            hard_deadline: None,
            send_ok: false,
        }
    }
}

impl PendingCodex {
    fn yield_view(&self) -> PendingYield {
        match self {
            Self::Confirm {
                call_id,
                name,
                arguments_json,
                prompt,
            } => PendingYield {
                call_id: call_id.clone(),
                name: name.clone(),
                arguments_json: arguments_json.clone(),
                kind: YieldKind::ActionRequired,
                prompt: prompt.clone(),
            },
            Self::Exec {
                call_id,
                arguments_json,
                prompt,
                ..
            } => PendingYield {
                call_id: call_id.clone(),
                name: "shell".to_string(),
                arguments_json: arguments_json.clone(),
                kind: YieldKind::ActionRequired,
                prompt: prompt.clone(),
            },
            Self::Patch {
                call_id,
                arguments_json,
                prompt,
                ..
            } => PendingYield {
                call_id: call_id.clone(),
                name: "apply_patch".to_string(),
                arguments_json: arguments_json.clone(),
                kind: YieldKind::ActionRequired,
                prompt: prompt.clone(),
            },
            Self::Execute {
                call_id,
                name,
                arguments_json,
            } => PendingYield {
                call_id: call_id.clone(),
                name: name.clone(),
                arguments_json: arguments_json.clone(),
                kind: YieldKind::ToolRequest,
                prompt: String::new(),
            },
        }
    }

    fn outcome(&self) -> TurnOutcome {
        let view = self.yield_view();
        TurnOutcome::Yielded {
            kind: view.kind,
            call_id: view.call_id,
            name: view.name,
        }
    }
}

impl AgentHost {
    pub async fn use_codex(&self, launch: CodexLaunch) -> Result<(), HostError> {
        if launch.api_key.trim().is_empty() {
            return Err(HostError::MissingApiKey);
        }
        if !launch.helper.is_absolute() || !launch.helper.is_file() {
            return Err(HostError::Failed(format!(
                "codex helper executable is not configured ({})",
                launch.helper.display()
            )));
        }
        if let Some(path) = &launch.linux_sandbox {
            if !path.is_absolute() {
                return Err(HostError::Failed(
                    "linux sandbox helper path must be absolute".into(),
                ));
            }
        }
        std::fs::create_dir_all(&launch.codex_home)
            .map_err(|err| HostError::Failed(format!("codex_home: {err}")))?;
        let mut slot = self.inner.codex.lock().await;
        slot.launch = Some(launch);
        slot.live = None;
        slot.pending = None;
        slot.turns = 0;
        slot.send_ok = false;
        self.inner
            .codex_runtime
            .store(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    pub(crate) async fn codex_prompt(
        &self,
        text: &str,
        context_json: Option<&str>,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError> {
        let mut slot = self.inner.codex.lock().await;
        if slot.pending.is_some() {
            return Err(HostError::Failed("agent waiting for tool".into()));
        }
        let profile = self.profile_snapshot();
        if let Some(max) = profile.max_turns {
            if slot.turns >= max {
                return Err(HostError::Failed("max turns".into()));
            }
        }
        self.ensure_live(&mut slot).await?;
        let mut body = String::new();
        if let Some(path) = slot
            .launch
            .as_ref()
            .and_then(|launch| launch.transcript.clone())
        {
            let earlier = read_transcript(&path);
            if !earlier.is_empty() {
                body.push_str("Earlier turns in this chat:\n");
                body.push_str(&earlier);
                body.push_str("\n\n");
            }
        }
        if let Some(ctx) = context_json.map(str::trim).filter(|s| !s.is_empty()) {
            body.push_str(&truncate_chars(ctx, 8 * 1024));
            body.push_str("\n\n");
        }
        body.push_str(text);
        let thread = slot
            .live
            .as_ref()
            .map(|live| Arc::clone(&live.thread))
            .ok_or_else(|| HostError::Failed("codex thread is not open".into()))?;
        let submission = thread
            .start_turn_if_idle(TurnInputRequest::user_input(vec![UserInput::Text {
                text: body,
                text_elements: Vec::new(),
            }]))
            .await
            .map_err(|err| HostError::Failed(err.to_string()))?;
        if let StartIfIdleSubmission::NotSubmitted { reason } = submission {
            return Err(HostError::Failed(format!(
                "turn input was not submitted: {reason:?}"
            )));
        }
        slot.turns = slot.turns.saturating_add(1);
        slot.send_ok = false;
        let transcript = slot
            .launch
            .as_ref()
            .and_then(|launch| launch.transcript.clone());
        let (idle, hard) = self.arm_deadlines(true);
        slot.hard_deadline = hard;
        let outcome = self.pump(&mut slot, thread, events, cancel, idle).await?;
        if let TurnOutcome::Finished {
            text: assistant, ..
        } = &outcome
        {
            if let Some(path) = transcript {
                if !assistant.trim().is_empty() {
                    append_transcript(&path, text, assistant);
                }
            }
        }
        Ok(outcome)
    }

    pub(crate) async fn codex_complete_tool(
        &self,
        call_id: &str,
        output_json: &str,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError> {
        let mut slot = self.inner.codex.lock().await;
        let Some(pending) = slot.pending.take() else {
            return Err(HostError::UnknownToolCall(call_id.to_string()));
        };
        let PendingCodex::Execute { name, .. } = &pending else {
            slot.pending = Some(pending);
            return Err(HostError::UnknownToolCall(call_id.to_string()));
        };
        if pending.yield_view().call_id != call_id {
            slot.pending = Some(pending);
            return Err(HostError::UnknownToolCall(call_id.to_string()));
        }
        let name = name.clone();
        let thread = live_thread(&slot)?;
        let ok = output_ok(output_json);
        if name == "send_message" && ok {
            slot.send_ok = true;
        }
        submit_dynamic(&thread, call_id, output_json, ok).await?;
        let (idle, _) = self.arm_deadlines(false);
        self.pump(&mut slot, thread, events, cancel, idle).await
    }

    pub(crate) async fn codex_respond_permission(
        &self,
        call_id: &str,
        permission: Permission,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError> {
        let mut slot = self.inner.codex.lock().await;
        let pending = slot
            .pending
            .take()
            .ok_or_else(|| HostError::UnknownToolCall(call_id.to_string()))?;
        if pending.yield_view().call_id != call_id {
            slot.pending = Some(pending);
            return Err(HostError::UnknownToolCall(call_id.to_string()));
        }
        let allowed = matches!(permission, Permission::AllowOnce | Permission::AlwaysAllow);
        match pending {
            PendingCodex::Confirm {
                call_id,
                name,
                arguments_json,
                ..
            } if allowed => {
                let next = PendingCodex::Execute {
                    call_id,
                    name,
                    arguments_json,
                };
                let outcome = next.outcome();
                slot.pending = Some(next);
                return Ok(outcome);
            }
            PendingCodex::Confirm { call_id, .. } => {
                let thread = live_thread(&slot)?;
                submit_dynamic(&thread, &call_id, "denied by user", false).await?;
            }
            PendingCodex::Exec { id, turn_id, .. } => {
                let thread = live_thread(&slot)?;
                thread
                    .submit(Op::ExecApproval {
                        id,
                        turn_id,
                        decision: review(permission),
                    })
                    .await
                    .map_err(|err| HostError::Failed(err.to_string()))?;
            }
            PendingCodex::Patch { id, .. } => {
                let thread = live_thread(&slot)?;
                thread
                    .submit(Op::PatchApproval {
                        id,
                        decision: review(permission),
                    })
                    .await
                    .map_err(|err| HostError::Failed(err.to_string()))?;
            }
            PendingCodex::Execute {
                call_id: pending_id,
                name,
                arguments_json,
            } => {
                slot.pending = Some(PendingCodex::Execute {
                    call_id: pending_id,
                    name,
                    arguments_json,
                });
                return Err(HostError::UnknownToolCall(call_id.to_string()));
            }
        }
        let thread = live_thread(&slot)?;
        let (idle, _) = self.arm_deadlines(false);
        self.pump(&mut slot, thread, events, cancel, idle).await
    }

    pub(crate) async fn codex_pending(&self) -> Vec<PendingYield> {
        let slot = self.inner.codex.lock().await;
        slot.pending.iter().map(PendingCodex::yield_view).collect()
    }

    pub(crate) async fn codex_cancel_pending(&self) {
        let mut slot = self.inner.codex.lock().await;
        let Some(pending) = slot.pending.take() else {
            return;
        };
        let Ok(thread) = live_thread(&slot) else {
            return;
        };
        match pending {
            PendingCodex::Confirm { call_id, .. } | PendingCodex::Execute { call_id, .. } => {
                let _ = submit_dynamic(&thread, &call_id, "cancelled", false).await;
            }
            PendingCodex::Exec { id, turn_id, .. } => {
                let _ = thread
                    .submit(Op::ExecApproval {
                        id,
                        turn_id,
                        decision: ReviewDecision::Abort,
                    })
                    .await;
            }
            PendingCodex::Patch { id, .. } => {
                let _ = thread
                    .submit(Op::PatchApproval {
                        id,
                        decision: ReviewDecision::Abort,
                    })
                    .await;
            }
        }
        let _ = thread.submit(Op::Interrupt).await;
    }

    async fn ensure_live(&self, slot: &mut CodexSlot) -> Result<(), HostError> {
        if slot.live.is_some() {
            return Ok(());
        }
        let launch = slot
            .launch
            .as_ref()
            .ok_or_else(|| HostError::Failed("codex runtime is not configured".into()))?;
        let profile = self.profile_snapshot();
        let mut opts = CodexEmbedOpts {
            codex_home: launch.codex_home.clone(),
            codex_self_exe: Some(launch.helper.clone()),
            codex_linux_sandbox_exe: launch.linux_sandbox.clone(),
            cwd: self.inner.project_root.clone(),
            model: profile.model.name.clone(),
            api_key: launch.api_key.clone(),
            prompt: "unused".into(),
        };
        if !opts.cwd.is_absolute() {
            opts.cwd = std::env::current_dir().unwrap_or(opts.cwd);
        }
        let api_key = launch.api_key.clone();
        let mut config = embed_config(&opts)?;
        apply_profile(&mut config, &profile, &api_key)?;
        let tools = kim_dynamic_tools(&profile);
        let started = start_thread(config, &api_key, tools).await?;
        slot.live = Some(LiveThread {
            manager: started.manager,
            thread: started.thread,
            thread_id: started.thread_id,
        });
        Ok(())
    }

    fn arm_deadlines(&self, reset_hard: bool) -> (Option<Instant>, Option<Instant>) {
        let limits = self.limits();
        let idle = finite(limits.idle).map(|d| Instant::now() + d);
        let hard = if reset_hard {
            finite(limits.hard).map(|d| Instant::now() + d)
        } else {
            None
        };
        (idle, hard)
    }

    async fn pump(
        &self,
        slot: &mut CodexSlot,
        thread: Arc<CodexThread>,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
        mut idle_deadline: Option<Instant>,
    ) -> Result<TurnOutcome, HostError> {
        let mut assistant = String::new();
        let mut saw_delta = false;
        let profile = self.profile_snapshot();
        let mut last_activity = Instant::now();
        loop {
            if cancel.is_cancelled() {
                return self.interrupted(&thread).await;
            }
            let hard_deadline = slot.hard_deadline;
            let event = tokio::select! {
                _ = cancel.cancelled() => return self.interrupted(&thread).await,
                _ = sleep_until(idle_deadline) => {
                    return Ok(TurnOutcome::TimedOut { kind: TimeoutKind::Idle });
                }
                _ = sleep_until(hard_deadline) => {
                    let recently_active = last_activity.elapsed() <= self.limits().recently_active;
                    let _ = thread.submit(Op::Interrupt).await;
                    return Ok(TurnOutcome::TimedOut {
                        kind: TimeoutKind::Hard { recently_active },
                    });
                }
                event = thread.next_event() => event.map_err(|err| HostError::Failed(err.to_string()))?,
            };
            let reset_idle = !matches!(event.msg, EventMsg::TokenCount(_));
            match event.msg {
                EventMsg::AgentMessageContentDelta(delta) => {
                    if !delta.delta.is_empty() {
                        saw_delta = true;
                        assistant.push_str(&delta.delta);
                        emit(&events, HostEvent::TextDelta { delta: delta.delta }).await?;
                    }
                }
                EventMsg::AgentMessage(message) => {
                    if !message.message.is_empty() {
                        if !saw_delta {
                            emit(
                                &events,
                                HostEvent::TextDelta {
                                    delta: message.message.clone(),
                                },
                            )
                            .await?;
                        }
                        assistant = message.message;
                    }
                }
                EventMsg::TokenCount(count) => {
                    if let Some(info) = count.info {
                        let usage = info.total_token_usage;
                        emit(
                            &events,
                            HostEvent::Usage {
                                input_tokens: usage.input_tokens.max(0) as u64,
                                output_tokens: usage.output_tokens.max(0) as u64,
                            },
                        )
                        .await?;
                    }
                }
                EventMsg::ExecCommandBegin(begin) => {
                    emit(
                        &events,
                        HostEvent::ToolRequest {
                            call_id: begin.call_id,
                            name: "shell".into(),
                            arguments_json: begin.command.join(" "),
                        },
                    )
                    .await?;
                }
                EventMsg::ExecCommandEnd(end) => {
                    emit(
                        &events,
                        HostEvent::ToolResult {
                            call_id: end.call_id,
                            name: "shell".into(),
                            output_preview: truncate_chars(&end.formatted_output, 180),
                            ok: end.exit_code == 0,
                        },
                    )
                    .await?;
                }
                EventMsg::McpToolCallBegin(begin) => {
                    emit(
                        &events,
                        HostEvent::ToolRequest {
                            call_id: begin.call_id,
                            name: format!(
                                "mcp:{}/{}",
                                begin.invocation.server, begin.invocation.tool
                            ),
                            arguments_json: begin
                                .invocation
                                .arguments
                                .map(|v| v.to_string())
                                .unwrap_or_default(),
                        },
                    )
                    .await?;
                }
                EventMsg::McpToolCallEnd(end) => {
                    let ok = end.result.is_ok();
                    let preview = match &end.result {
                        Ok(_) => "ok".to_string(),
                        Err(err) => truncate_chars(err, 180),
                    };
                    emit(
                        &events,
                        HostEvent::ToolResult {
                            call_id: end.call_id,
                            name: format!("mcp:{}/{}", end.invocation.server, end.invocation.tool),
                            output_preview: preview,
                            ok,
                        },
                    )
                    .await?;
                }
                EventMsg::ContextCompacted(_) => {
                    emit(
                        &events,
                        HostEvent::ToolResult {
                            call_id: "context_compacted".into(),
                            name: "context_compacted".into(),
                            output_preview: "已压缩".into(),
                            ok: true,
                        },
                    )
                    .await?;
                }
                EventMsg::DynamicToolCallRequest(req) => {
                    let arguments_json =
                        serde_json::to_string(&req.arguments).unwrap_or_else(|_| "{}".into());
                    let prompt = format!("{} {arguments_json}", req.tool);
                    let pending = if confirms_in_app(&profile, &req.tool) {
                        PendingCodex::Confirm {
                            call_id: req.call_id,
                            name: req.tool,
                            arguments_json,
                            prompt,
                        }
                    } else {
                        PendingCodex::Execute {
                            call_id: req.call_id,
                            name: req.tool,
                            arguments_json,
                        }
                    };
                    let outcome = pending.outcome();
                    slot.pending = Some(pending);
                    return Ok(outcome);
                }
                EventMsg::DynamicToolCallResponse(resp) => {
                    if resp.tool == "send_message" && resp.success {
                        slot.send_ok = true;
                    }
                    emit(
                        &events,
                        HostEvent::ToolResult {
                            call_id: resp.call_id,
                            name: resp.tool,
                            output_preview: truncate_chars(
                                &resp.content_items.len().to_string(),
                                32,
                            ),
                            ok: resp.success,
                        },
                    )
                    .await?;
                }
                EventMsg::ExecApprovalRequest(req) => {
                    let arguments_json = req.command.join(" ");
                    let prompt = req.reason.clone().unwrap_or_else(|| arguments_json.clone());
                    let id = req.approval_id.unwrap_or(req.call_id.clone());
                    let pending = PendingCodex::Exec {
                        id,
                        turn_id: if req.turn_id.is_empty() {
                            None
                        } else {
                            Some(req.turn_id)
                        },
                        call_id: req.call_id,
                        arguments_json,
                        prompt,
                    };
                    let outcome = pending.outcome();
                    slot.pending = Some(pending);
                    return Ok(outcome);
                }
                EventMsg::ApplyPatchApprovalRequest(req) => {
                    let prompt = req.reason.unwrap_or_else(|| "apply patch".into());
                    let pending = PendingCodex::Patch {
                        id: req.call_id.clone(),
                        call_id: req.call_id,
                        arguments_json: prompt.clone(),
                        prompt,
                    };
                    let outcome = pending.outcome();
                    slot.pending = Some(pending);
                    return Ok(outcome);
                }
                EventMsg::TurnComplete(done) => {
                    if let Some(error) = done.error {
                        return Err(HostError::Failed(error.message));
                    }
                    let text = done
                        .last_agent_message
                        .filter(|text| !text.is_empty())
                        .unwrap_or(assistant);
                    let replied = !text.trim().is_empty();
                    return Ok(TurnOutcome::Finished {
                        text,
                        replied,
                        visible: replied || slot.send_ok,
                    });
                }
                EventMsg::Error(error) => return Err(HostError::Failed(error.message)),
                EventMsg::TurnAborted(_) => {
                    return Ok(TurnOutcome::Cancelled {
                        reason: CancelReason::UserAbort,
                    });
                }
                EventMsg::RequestUserInput(_) => {
                    return Err(HostError::Failed("turn requested user input".into()));
                }
                EventMsg::RequestPermissions(_) => {
                    return Err(HostError::Failed("turn requested permissions".into()));
                }
                _ => {}
            }
            if reset_idle {
                last_activity = Instant::now();
                if let Some(limit) = finite(self.limits().idle) {
                    idle_deadline = Some(Instant::now() + limit);
                }
            }
        }
    }

    async fn interrupted(&self, thread: &CodexThread) -> Result<TurnOutcome, HostError> {
        let _ = thread.submit(Op::Interrupt).await;
        Ok(TurnOutcome::Cancelled {
            reason: CancelReason::UserAbort,
        })
    }
}

fn live_thread(slot: &CodexSlot) -> Result<Arc<CodexThread>, HostError> {
    slot.live
        .as_ref()
        .map(|live| Arc::clone(&live.thread))
        .ok_or_else(|| HostError::Failed("codex thread is not open".into()))
}

async fn submit_dynamic(
    thread: &CodexThread,
    call_id: &str,
    text: &str,
    success: bool,
) -> Result<(), HostError> {
    thread
        .submit(Op::DynamicToolResponse {
            id: call_id.to_string(),
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: text.to_string(),
                }],
                success,
            },
        })
        .await
        .map_err(|err| HostError::Failed(err.to_string()))?;
    Ok(())
}

fn review(permission: Permission) -> ReviewDecision {
    match permission {
        Permission::AlwaysAllow => ReviewDecision::ApprovedForSession,
        Permission::AllowOnce => ReviewDecision::Approved,
        Permission::DenyOnce | Permission::AlwaysDeny => ReviewDecision::Denied {
            rejection: "denied by user".into(),
        },
        Permission::Cancel => ReviewDecision::Abort,
    }
}

fn output_ok(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|value| value.get("ok").and_then(|flag| flag.as_bool()))
        .unwrap_or(true)
}

fn finite(duration: Duration) -> Option<Duration> {
    if duration == Duration::MAX {
        None
    } else {
        Some(duration)
    }
}

fn read_transcript(path: &std::path::Path) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    const MAX: usize = 24 * 1024;
    if text.len() <= MAX {
        return text;
    }
    let mut start = text.len() - MAX;
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].to_string()
}

fn append_transcript(path: &std::path::Path, user: &str, assistant: &str) {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = writeln!(
        file,
        "User: {}\nAssistant: {}\n",
        user.trim(),
        assistant.trim()
    );
}

async fn emit(events: &mpsc::Sender<HostEvent>, event: HostEvent) -> Result<(), HostError> {
    events
        .send(event)
        .await
        .map_err(|_| HostError::Failed("event channel closed".into()))
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => {
            tokio::time::sleep(deadline.saturating_duration_since(Instant::now())).await;
        }
        None => std::future::pending().await,
    }
}
