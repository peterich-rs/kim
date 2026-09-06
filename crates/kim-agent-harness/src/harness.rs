use crate::effects::Effects;
use futures::StreamExt;
use kim_agent_hooks::{BeforeToolDecision, Hooks, NoopHooks, ToolInvocation};
use kim_agent_llm::{reduce_events, ContentPart, ModelRef, ResponseInputItem, ResponseRequest};
use kim_agent_session::{
    discover_agents_context, SessionFacade, DEFAULT_AGENTS_CHAR_CAP,
};
use kim_agent_storage::{now_ms, Storage, ValueAddr, Write};
use kim_agent_tools::ToolRegistry;
use kim_agent_types::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HarnessError {
    #[error(transparent)]
    Storage(#[from] kim_agent_storage::StorageError),
    #[error(transparent)]
    Session(#[from] kim_agent_session::SessionError),
    #[error("serde: {0}")]
    Serde(String),
    #[error("fault: {0}")]
    Fault(String),
    #[error("no current operation")]
    NoOperation,
    #[error("llm: {0}")]
    Llm(String),
}

#[derive(Debug, Clone)]
pub struct AcceptRequest {
    pub prompt: String,
    pub lane: String,
}

impl AcceptRequest {
    pub fn prompt(text: impl Into<String>) -> Self {
        Self {
            prompt: text.into(),
            lane: "main".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperationAdmission {
    pub operation_id: OperationId,
    pub prompt_entry_id: EntryId,
}

#[derive(Debug, Clone)]
pub enum DriveStatus {
    Parked { reason: String },
    Completed { result: OperationResult },
    Aborted { result: OperationResult },
    Fault { error: String },
}

#[derive(Debug, Clone, Default)]
pub struct ResumeReport {
    pub resumed_operations: Vec<OperationId>,
    pub drive_statuses: Vec<(OperationId, String)>,
}

pub struct Harness {
    session: SessionFacade,
    effects: Arc<dyn Effects>,
    hooks: Arc<dyn Hooks>,
    tools: ToolRegistry,
    project_root: PathBuf,
    model: String,
}

impl Harness {
    pub fn builder(storage: Arc<dyn Storage>) -> HarnessBuilder {
        HarnessBuilder {
            storage,
            effects: None,
            hooks: Arc::new(NoopHooks),
            tools: ToolRegistry::new(),
            project_root: PathBuf::from("."),
            model: "gpt-test".into(),
        }
    }

    pub async fn accept(
        &self,
        req: AcceptRequest,
        _cx: &Context,
    ) -> Result<OperationAdmission, HarnessError> {
        let tip = self.session.branch_tip().await?;
        let prompt_entry = SessionFacade::message_entry(
            tip,
            AgentMessage::User {
                text: req.prompt.clone(),
            },
        );
        let prompt_entry_id = prompt_entry.id();
        let op_id = OperationId::new();
        let started = now_ms();
        let meta = OperationMeta {
            operation_id: op_id,
            lane: req.lane.clone(),
            source_tip_id: tip,
            started_at_ms: started,
            intent: OperationIntent::Run {
                prompt_entry_ids: vec![prompt_entry_id],
            },
        };
        let state = OperationState::Starting {
            scope: OperationScope::new(),
        };
        let lane_state = LaneState {
            current_operation_id: Some(op_id),
            last_operation_id: Some(op_id),
            inbox: Vec::new(),
        };

        self.session
            .storage()
            .commit(vec![
                Write::InsertEntry(prompt_entry),
                self.session.tip_write(prompt_entry_id),
                Write::SetValue {
                    address: ValueAddr::op_meta(op_id),
                    value: to_json(&meta)?,
                },
                Write::SetValue {
                    address: ValueAddr::op_state(op_id),
                    value: to_json(&state)?,
                },
                Write::SetValue {
                    address: ValueAddr::lane_state(&req.lane),
                    value: to_json(&lane_state)?,
                },
            ])
            .await?;

        Ok(OperationAdmission {
            operation_id: op_id,
            prompt_entry_id,
        })
    }

    pub async fn request_abort(&self, lane: &str, _cx: &Context) -> Result<(), HarnessError> {
        let mut lane_state: LaneState = match self
            .session
            .storage()
            .get_value(&ValueAddr::lane_state(lane))
            .await?
        {
            Some(v) => serde_json::from_value(v).map_err(|e| HarnessError::Serde(e.to_string()))?,
            None => return Ok(()),
        };
        let Some(op) = lane_state.current_operation_id else {
            return Ok(());
        };
        let mut state = self.load_op_state(op).await?;
        state.scope_mut().control = Control::CancelRequested {
            requested_at_ms: now_ms(),
        };
        self.session
            .storage()
            .commit(vec![Write::SetValue {
                address: ValueAddr::op_state(op),
                value: to_json(&state)?,
            }])
            .await?;
        let _ = &mut lane_state;
        Ok(())
    }

    pub async fn drive(&self, op: OperationId, cx: &Context) -> Result<DriveStatus, HarnessError> {
        loop {
            if cx.is_cancelled() {
                let _ = self.request_abort(self.session.lane(), cx).await;
            }
            let state = self.load_op_state(op).await?;
            match state {
                OperationState::Starting { scope } => {
                    if scope.is_cancel_requested() || cx.is_cancelled() {
                        return self.terminal_abort(op, None).await;
                    }
                    let tip = self.session.branch_tip().await?.unwrap_or_else(EntryId::new);
                    let next = OperationState::Checkpoint {
                        scope: {
                            // before_run skipped (D4 optional)
                            scope
                        },
                        continuation: Continuation::NeedAssistant,
                        trigger_entry_id: tip,
                    };
                    self.save_op_state(op, &next).await?;
                }
                OperationState::Checkpoint {
                    scope,
                    continuation,
                    trigger_entry_id,
                } => {
                    if scope.is_cancel_requested() || cx.is_cancelled() {
                        return self.terminal_abort(op, Some(trigger_entry_id)).await;
                    }
                    match continuation {
                        Continuation::MayFinish => {
                            return self.terminal_complete(op, Some(trigger_entry_id)).await;
                        }
                        Continuation::NeedAssistant => {
                            let next = OperationState::AssistantReady { scope };
                            self.save_op_state(op, &next).await?;
                        }
                    }
                }
                OperationState::AssistantReady { scope } => {
                    if scope.is_cancel_requested() || cx.is_cancelled() {
                        return self.terminal_abort(op, scope.latest_assistant_entry_id).await;
                    }
                    let response_entry_id = EntryId::new();
                    let usage_id = UsageId::new();
                    let next = OperationState::AssistantEffectPending {
                        scope,
                        response_entry_id,
                        usage_id,
                        attempt: 0,
                    };
                    self.save_op_state(op, &next).await?;
                }
                OperationState::AssistantEffectPending {
                    mut scope,
                    response_entry_id,
                    usage_id,
                    attempt,
                } => {
                    if scope.is_cancel_requested() || cx.is_cancelled() {
                        // Unknown outcome policy: settle aborted under reserved ids
                        return self
                            .settle_assistant_aborted(op, scope, response_entry_id, usage_id)
                            .await;
                    }
                    match self
                        .run_assistant_effect(op, &mut scope, response_entry_id, usage_id, attempt, cx)
                        .await
                    {
                        Ok(DriveStep::Continue) => {}
                        Ok(DriveStep::Done(status)) => return Ok(status),
                        Err(e) => {
                            return Ok(DriveStatus::Fault {
                                error: e.to_string(),
                            });
                        }
                    }
                }
                OperationState::AssistantRetryWait {
                    scope,
                    not_before_ms,
                    ..
                } => {
                    if now_ms() < not_before_ms {
                        return Ok(DriveStatus::Parked {
                            reason: "retry_wait".into(),
                        });
                    }
                    let next = OperationState::AssistantReady { scope };
                    self.save_op_state(op, &next).await?;
                }
                OperationState::Tools {
                    scope,
                    step_id,
                    calls,
                } => {
                    match self
                        .drive_tools(op, scope, step_id, calls, cx)
                        .await?
                    {
                        DriveStep::Continue => {}
                        DriveStep::Done(status) => return Ok(status),
                    }
                }
            }
        }
    }

    pub async fn prompt(
        &self,
        text: impl Into<String>,
        cx: &Context,
    ) -> Result<OperationResult, HarnessError> {
        let adm = self.accept(AcceptRequest::prompt(text), cx).await?;
        match self.drive(adm.operation_id, cx).await? {
            DriveStatus::Completed { result } | DriveStatus::Aborted { result } => Ok(result),
            DriveStatus::Parked { reason } => Err(HarnessError::Fault(format!("parked: {reason}"))),
            DriveStatus::Fault { error } => Err(HarnessError::Fault(error)),
        }
    }

    pub async fn resume(&self, cx: &Context) -> Result<ResumeReport, HarnessError> {
        let lane_state = self.session.lane_state().await?;
        let mut report = ResumeReport::default();
        if let Some(op) = lane_state.current_operation_id {
            report.resumed_operations.push(op);
            // Ensure op_state exists (total restart point)
            let _ = self.load_op_state(op).await?;
            let status = self.drive(op, cx).await?;
            let label = match &status {
                DriveStatus::Completed { .. } => "completed",
                DriveStatus::Aborted { .. } => "aborted",
                DriveStatus::Parked { reason } => reason.as_str(),
                DriveStatus::Fault { error } => error.as_str(),
            };
            report.drive_statuses.push((op, label.to_string()));
        }
        Ok(report)
    }

    async fn run_assistant_effect(
        &self,
        op: OperationId,
        scope: &mut OperationScope,
        response_entry_id: EntryId,
        usage_id: UsageId,
        _attempt: u32,
        cx: &Context,
    ) -> Result<DriveStep, HarnessError> {
        let history = self.session.project_context(200).await?;
        let (instructions, _) =
            discover_agents_context(&self.project_root, DEFAULT_AGENTS_CHAR_CAP);
        let input = map_messages_to_input(&history);
        let req = ResponseRequest {
            model: ModelRef::new(self.model.clone()),
            input,
            tools: self.tools.schemas(),
            tool_choice: ToolChoice::Auto,
            parallel_tool_calls: scope.settings.parallel_tool_calls,
            stream: true,
            previous_response_id: None,
            max_output_tokens: None,
            instructions: if instructions.is_empty() {
                None
            } else {
                Some(instructions)
            },
        };

        let stream = self
            .effects
            .complete_llm(req, cx)
            .await
            .map_err(|e| HarnessError::Llm(e.to_string()))?;
        futures::pin_mut!(stream);

        let mut events = Vec::new();
        let mut aborted = false;
        while let Some(item) = stream.next().await {
            match item {
                Ok(ev) => events.push(ev),
                Err(kim_agent_llm::LlmError::Aborted) => {
                    aborted = true;
                    break;
                }
                Err(e) => {
                    // Settle as error assistant
                    let settled = SettledAssistantMessage {
                        content: vec![AssistantContent::Text {
                            text: format!("error: {e}"),
                        }],
                        stop_reason: StopReason::Error,
                        provider_response_id: None,
                    };
                    return self
                        .settle_assistant(op, scope, response_entry_id, usage_id, settled, Usage::default())
                        .await;
                }
            }
            if cx.is_cancelled() || scope.is_cancel_requested() {
                aborted = true;
                break;
            }
        }

        let mut settled = reduce_events(events);
        if aborted {
            settled.stop_reason = StopReason::Aborted;
        }
        self.settle_assistant(op, scope, response_entry_id, usage_id, settled, Usage::default())
            .await
    }

    async fn settle_assistant_aborted(
        &self,
        op: OperationId,
        mut scope: OperationScope,
        response_entry_id: EntryId,
        usage_id: UsageId,
    ) -> Result<DriveStatus, HarnessError> {
        let settled = SettledAssistantMessage {
            content: vec![AssistantContent::Text {
                text: "[aborted]".into(),
            }],
            stop_reason: StopReason::Aborted,
            provider_response_id: None,
        };
        match self
            .settle_assistant(op, &mut scope, response_entry_id, usage_id, settled, Usage::default())
            .await?
        {
            DriveStep::Done(s) => Ok(s),
            DriveStep::Continue => self.terminal_abort(op, Some(response_entry_id)).await,
        }
    }

    async fn settle_assistant(
        &self,
        op: OperationId,
        scope: &mut OperationScope,
        response_entry_id: EntryId,
        usage_id: UsageId,
        settled: SettledAssistantMessage,
        usage: Usage,
    ) -> Result<DriveStep, HarnessError> {
        let tip = self.session.branch_tip().await?;
        let entry = Entry::Message {
            base: EntryBase {
                id: response_entry_id,
                parent_id: tip,
                seq: 0,
                timestamp_ms: 0,
            },
            message: AgentMessage::Assistant {
                content: settled.content.clone(),
                stop_reason: settled.stop_reason,
                provider_response_id: settled.provider_response_id.clone(),
            },
        };
        scope.latest_assistant_entry_id = Some(response_entry_id);

        let usage_row = UsageRow {
            id: usage_id,
            operation_id: op,
            usage,
            model: Some(self.model.clone()),
        };

        let next_state = match settled.stop_reason {
            StopReason::ToolUse => {
                let mut calls = Vec::new();
                for c in &settled.content {
                    if let AssistantContent::ToolCall { source_index, .. } = c {
                        calls.push(ToolCallState::Planned {
                            source_index: *source_index,
                            result_entry_id: EntryId::new(),
                        });
                    }
                }
                OperationState::Tools {
                    scope: scope.clone(),
                    step_id: format!("step-{}", response_entry_id),
                    calls,
                }
            }
            StopReason::Aborted => {
                // Commit then terminal abort
                self.session
                    .storage()
                    .commit(vec![
                        Write::InsertEntry(entry),
                        Write::InsertUsage(usage_row),
                        self.session.tip_write(response_entry_id),
                        Write::SetValue {
                            address: ValueAddr::op_state(op),
                            value: to_json(&OperationState::Checkpoint {
                                scope: scope.clone(),
                                continuation: Continuation::MayFinish,
                                trigger_entry_id: response_entry_id,
                            })?,
                        },
                    ])
                    .await?;
                return Ok(DriveStep::Done(
                    self.terminal_abort(op, Some(response_entry_id)).await?,
                ));
            }
            StopReason::Error | StopReason::Length | StopReason::Stop => OperationState::Checkpoint {
                scope: scope.clone(),
                continuation: Continuation::MayFinish,
                trigger_entry_id: response_entry_id,
            },
        };

        self.session
            .storage()
            .commit(vec![
                Write::InsertEntry(entry),
                Write::InsertUsage(usage_row),
                self.session.tip_write(response_entry_id),
                Write::SetValue {
                    address: ValueAddr::op_state(op),
                    value: to_json(&next_state)?,
                },
            ])
            .await?;
        Ok(DriveStep::Continue)
    }

    async fn drive_tools(
        &self,
        op: OperationId,
        scope: OperationScope,
        step_id: String,
        mut calls: Vec<ToolCallState>,
        cx: &Context,
    ) -> Result<DriveStep, HarnessError> {
        if scope.is_cancel_requested() || cx.is_cancelled() {
            return Ok(DriveStep::Done(
                self.terminal_abort(op, scope.latest_assistant_entry_id)
                    .await?,
            ));
        }

        // Load assistant content for args
        let assistant_id = scope
            .latest_assistant_entry_id
            .ok_or_else(|| HarnessError::Fault("tools without assistant".into()))?;
        let assistant = self
            .session
            .storage()
            .get_entry(assistant_id)
            .await?
            .ok_or_else(|| HarnessError::Fault("missing assistant".into()))?;
        let tool_calls: Vec<(usize, String, String, JsonValue, Option<String>)> =
            match assistant.message() {
                Some(AgentMessage::Assistant { content, .. }) => content
                    .iter()
                    .filter_map(|c| match c {
                        AssistantContent::ToolCall {
                            source_index,
                            call_id,
                            name,
                            arguments,
                            parse_error,
                        } => Some((
                            *source_index,
                            call_id.clone(),
                            name.clone(),
                            arguments.clone(),
                            parse_error.clone(),
                        )),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };

        // Advance Planned → EffectPending → OutcomeReady → Completed
        for i in 0..calls.len() {
            match &calls[i] {
                ToolCallState::Planned {
                    source_index,
                    result_entry_id,
                } => {
                    let Some((_, _, name, args, parse_err)) =
                        tool_calls.iter().find(|(si, ..)| si == source_index)
                    else {
                        continue;
                    };
                    let inv_id = format!("{step_id}:{i}");
                    if let Some(err) = parse_err {
                        // Synthetic error, skip effect
                        self.stage_tool_pending(
                            *result_entry_id,
                            name,
                            &tool_calls
                                .iter()
                                .find(|(si, ..)| si == source_index)
                                .map(|t| t.1.clone())
                                .unwrap_or_default(),
                            &format!("invalid arguments: {err}"),
                            true,
                        )
                        .await?;
                        calls[i] = ToolCallState::OutcomeReady {
                            source_index: *source_index,
                            result_entry_id: *result_entry_id,
                            terminate: false,
                        };
                        self.save_tools_state(op, &scope, &step_id, &calls).await?;
                        continue;
                    }

                    let inv = ToolInvocation {
                        operation_id: op.to_string(),
                        invocation_id: inv_id.clone(),
                        name: name.clone(),
                        arguments: args.clone(),
                    };
                    let decision = self
                        .hooks
                        .before_tool(&inv)
                        .await
                        .map_err(|e| HarnessError::Fault(e.to_string()))?;
                    let (args, blocked) = match decision {
                        BeforeToolDecision::Allow(a) => (a, None),
                        BeforeToolDecision::Block { message } => (args.clone(), Some(message)),
                    };

                    let replay = self
                        .tools
                        .get(name)
                        .map(|t| t.replay_policy())
                        .unwrap_or(ReplayPolicy::Never);

                    self.session
                        .storage()
                        .commit(vec![Write::SetValue {
                            address: ValueAddr::op_tool_args(op, &step_id, i),
                            value: args.clone(),
                        }])
                        .await?;

                    if let Some(msg) = blocked {
                        self.stage_tool_pending(
                            *result_entry_id,
                            name,
                            &inv.invocation_id,
                            &msg,
                            true,
                        )
                        .await?;
                        calls[i] = ToolCallState::OutcomeReady {
                            source_index: *source_index,
                            result_entry_id: *result_entry_id,
                            terminate: false,
                        };
                    } else {
                        calls[i] = ToolCallState::EffectPending {
                            source_index: *source_index,
                            result_entry_id: *result_entry_id,
                            replay,
                        };
                    }
                    self.save_tools_state(op, &scope, &step_id, &calls).await?;
                }
                ToolCallState::EffectPending {
                    source_index,
                    result_entry_id,
                    replay,
                } => {
                    let Some((_, call_id, name, _, _)) =
                        tool_calls.iter().find(|(si, ..)| si == source_index)
                    else {
                        continue;
                    };
                    let args = self
                        .session
                        .storage()
                        .get_value(&ValueAddr::op_tool_args(op, &step_id, i))
                        .await?
                        .unwrap_or(json!({}));

                    // On resume with Never: do not re-exec — synthetic interrupted
                    // We detect "fresh" vs resume by pending_tool_output presence? Design says:
                    // EffectPending{Never} → do not re-exec; synthetic interrupted.
                    // For first entry into EffectPending we need to run once.
                    // Use a marker: if pending_tool_output already set with __started, then resume path.
                    let pending_addr =
                        ValueAddr::pending_tool_output(op, &format!("{step_id}:{i}"));
                    let already = self.session.storage().get_value(&pending_addr).await?;
                    let result = if already.is_some() && *replay == ReplayPolicy::Never {
                        ToolResult {
                            output: "[interrupted: tool effect not replayed]".into(),
                            is_error: true,
                        }
                    } else {
                        // mark started
                        self.session
                            .storage()
                            .commit(vec![Write::SetValue {
                                address: pending_addr.clone(),
                                value: json!({"started": true}),
                            }])
                            .await?;
                        let inv = ToolInvocation {
                            operation_id: op.to_string(),
                            invocation_id: format!("{step_id}:{i}"),
                            name: name.clone(),
                            arguments: args.clone(),
                        };
                        let raw = match self.effects.call_tool(name, args).await {
                            Ok(r) => r,
                            Err(e) => ToolResult {
                                output: e.to_string(),
                                is_error: true,
                            },
                        };
                        self.hooks
                            .after_tool(&inv, &raw)
                            .await
                            .map_err(|e| HarnessError::Fault(e.to_string()))?
                    };

                    self.stage_tool_pending(
                        *result_entry_id,
                        name,
                        call_id,
                        &result.output,
                        result.is_error,
                    )
                    .await?;
                    calls[i] = ToolCallState::OutcomeReady {
                        source_index: *source_index,
                        result_entry_id: *result_entry_id,
                        terminate: false,
                    };
                    self.save_tools_state(op, &scope, &step_id, &calls).await?;
                }
                _ => {}
            }
        }

        // Materialize OutcomeReady in source order
        let mut tip = self.session.branch_tip().await?;
        let mut writes = Vec::new();
        let mut progressed = false;
        for i in 0..calls.len() {
            // Only materialize if all previous are Completed
            if calls.iter().take(i).any(|c| !matches!(c, ToolCallState::Completed { .. })) {
                break;
            }
            if let ToolCallState::OutcomeReady {
                source_index,
                result_entry_id,
                terminate,
            } = calls[i].clone()
            {
                let pending_addr = ValueAddr::pending_entry(result_entry_id);
                let pending = self
                    .session
                    .storage()
                    .get_value(&pending_addr)
                    .await?
                    .ok_or_else(|| HarnessError::Fault("missing pending tool entry".into()))?;
                let mut entry: Entry = serde_json::from_value(pending)
                    .map_err(|e| HarnessError::Serde(e.to_string()))?;
                entry.base_mut().parent_id = tip;
                writes.push(Write::InsertEntry(entry));
                writes.push(Write::DeleteValue {
                    address: pending_addr,
                });
                tip = Some(result_entry_id);
                writes.push(self.session.tip_write(result_entry_id));
                calls[i] = ToolCallState::Completed {
                    source_index,
                    result_entry_id,
                    terminate,
                };
                progressed = true;
            }
        }

        if progressed {
            let all_done = calls
                .iter()
                .all(|c| matches!(c, ToolCallState::Completed { .. }));
            let next = if all_done {
                OperationState::Checkpoint {
                    scope: scope.clone(),
                    continuation: Continuation::NeedAssistant,
                    trigger_entry_id: tip.unwrap_or_else(EntryId::new),
                }
            } else {
                OperationState::Tools {
                    scope: scope.clone(),
                    step_id: step_id.clone(),
                    calls: calls.clone(),
                }
            };
            writes.push(Write::SetValue {
                address: ValueAddr::op_state(op),
                value: to_json(&next)?,
            });
            self.session.storage().commit(writes).await?;
            return Ok(DriveStep::Continue);
        }

        // If all completed already
        if calls
            .iter()
            .all(|c| matches!(c, ToolCallState::Completed { .. }))
        {
            let next = OperationState::Checkpoint {
                scope,
                continuation: Continuation::NeedAssistant,
                trigger_entry_id: tip.unwrap_or_else(EntryId::new),
            };
            self.save_op_state(op, &next).await?;
            return Ok(DriveStep::Continue);
        }

        Ok(DriveStep::Continue)
    }

    async fn stage_tool_pending(
        &self,
        result_entry_id: EntryId,
        name: &str,
        call_id: &str,
        output: &str,
        is_error: bool,
    ) -> Result<(), HarnessError> {
        let entry = Entry::Message {
            base: EntryBase {
                id: result_entry_id,
                parent_id: None,
                seq: 0,
                timestamp_ms: 0,
            },
            message: AgentMessage::Tool {
                call_id: call_id.to_string(),
                name: name.to_string(),
                output: output.to_string(),
                is_error,
            },
        };
        self.session
            .storage()
            .commit(vec![Write::SetValue {
                address: ValueAddr::pending_entry(result_entry_id),
                value: to_json(&entry)?,
            }])
            .await?;
        Ok(())
    }

    async fn save_tools_state(
        &self,
        op: OperationId,
        scope: &OperationScope,
        step_id: &str,
        calls: &[ToolCallState],
    ) -> Result<(), HarnessError> {
        self.save_op_state(
            op,
            &OperationState::Tools {
                scope: scope.clone(),
                step_id: step_id.to_string(),
                calls: calls.to_vec(),
            },
        )
        .await
    }

    async fn terminal_complete(
        &self,
        op: OperationId,
        final_entry_id: Option<EntryId>,
    ) -> Result<DriveStatus, HarnessError> {
        let result = OperationResult::Completed {
            operation_id: op,
            final_entry_id,
        };
        self.clear_op(op, &result).await?;
        Ok(DriveStatus::Completed { result })
    }

    async fn terminal_abort(
        &self,
        op: OperationId,
        final_entry_id: Option<EntryId>,
    ) -> Result<DriveStatus, HarnessError> {
        let result = OperationResult::Aborted {
            operation_id: op,
            final_entry_id,
        };
        self.clear_op(op, &result).await?;
        Ok(DriveStatus::Aborted { result })
    }

    async fn clear_op(
        &self,
        op: OperationId,
        result: &OperationResult,
    ) -> Result<(), HarnessError> {
        let mut lane = self.session.lane_state().await?;
        lane.current_operation_id = None;
        lane.last_operation_id = Some(op);
        self.session
            .storage()
            .commit(vec![
                Write::DeleteValue {
                    address: ValueAddr::op_meta(op),
                },
                Write::DeleteValue {
                    address: ValueAddr::op_state(op),
                },
                Write::SetValue {
                    address: ValueAddr::op_result(op),
                    value: to_json(result)?,
                },
                Write::SetValue {
                    address: ValueAddr::lane_state(self.session.lane()),
                    value: to_json(&lane)?,
                },
            ])
            .await?;
        Ok(())
    }

    async fn load_op_state(&self, op: OperationId) -> Result<OperationState, HarnessError> {
        let v = self
            .session
            .storage()
            .get_value(&ValueAddr::op_state(op))
            .await?
            .ok_or(HarnessError::NoOperation)?;
        serde_json::from_value(v).map_err(|e| HarnessError::Serde(e.to_string()))
    }

    async fn save_op_state(&self, op: OperationId, state: &OperationState) -> Result<(), HarnessError> {
        self.session
            .storage()
            .commit(vec![Write::SetValue {
                address: ValueAddr::op_state(op),
                value: to_json(state)?,
            }])
            .await?;
        Ok(())
    }
}

enum DriveStep {
    Continue,
    Done(DriveStatus),
}

fn to_json<T: serde::Serialize>(v: &T) -> Result<JsonValue, HarnessError> {
    serde_json::to_value(v).map_err(|e| HarnessError::Serde(e.to_string()))
}

fn map_messages_to_input(msgs: &[AgentMessage]) -> Vec<ResponseInputItem> {
    let mut out = Vec::new();
    for m in msgs {
        match m {
            AgentMessage::System { text } | AgentMessage::User { text } => {
                out.push(ResponseInputItem::Message {
                    role: "user".into(),
                    content: vec![ContentPart::InputText {
                        text: text.clone(),
                    }],
                });
            }
            AgentMessage::Assistant { content, .. } => {
                let mut text = String::new();
                for c in content {
                    match c {
                        AssistantContent::Text { text: t } => text.push_str(t),
                        AssistantContent::ToolCall {
                            call_id,
                            name,
                            arguments,
                            ..
                        } => {
                            if !text.is_empty() {
                                out.push(ResponseInputItem::Message {
                                    role: "assistant".into(),
                                    content: vec![ContentPart::OutputText { text: text.clone() }],
                                });
                                text.clear();
                            }
                            out.push(ResponseInputItem::FunctionCall {
                                call_id: call_id.clone(),
                                name: name.clone(),
                                arguments: arguments.to_string(),
                            });
                        }
                    }
                }
                if !text.is_empty() {
                    out.push(ResponseInputItem::Message {
                        role: "assistant".into(),
                        content: vec![ContentPart::OutputText { text }],
                    });
                }
            }
            AgentMessage::Tool {
                call_id, output, ..
            } => {
                out.push(ResponseInputItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output: output.clone(),
                });
            }
        }
    }
    out
}

pub struct HarnessBuilder {
    storage: Arc<dyn Storage>,
    effects: Option<Arc<dyn Effects>>,
    hooks: Arc<dyn Hooks>,
    tools: ToolRegistry,
    project_root: PathBuf,
    model: String,
}

impl HarnessBuilder {
    pub fn effects(mut self, effects: Arc<dyn Effects>) -> Self {
        self.effects = Some(effects);
        self
    }
    pub fn hooks(mut self, hooks: Arc<dyn Hooks>) -> Self {
        self.hooks = hooks;
        self
    }
    pub fn tools(mut self, tools: ToolRegistry) -> Self {
        self.tools = tools;
        self
    }
    pub fn project_root(mut self, p: impl AsRef<Path>) -> Self {
        self.project_root = p.as_ref().to_path_buf();
        self
    }
    pub fn model(mut self, m: impl Into<String>) -> Self {
        self.model = m.into();
        self
    }
    pub fn build(self) -> Result<Harness, HarnessError> {
        let effects = self
            .effects
            .ok_or_else(|| HarnessError::Fault("effects required".into()))?;
        Ok(Harness {
            session: SessionFacade::new(self.storage),
            effects,
            hooks: self.hooks,
            tools: self.tools,
            project_root: self.project_root,
            model: self.model,
        })
    }
}
