use std::path::PathBuf;
use std::time::Duration;

use kim_agent_host::{AgentHost, AgentProfile, HostEvent, TurnOutcome, YieldKind};
use kim_sdk::{AgentRunResult, SdkError};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::tools::execute_im_tool;
use crate::{HostAgentRuntime, PermissionEvent, PreparedTurn};

const MAX_YIELDS: usize = 8;

pub async fn drive_host(
    runtime: &HostAgentRuntime,
    prepared: PreparedTurn,
) -> Result<AgentRunResult, SdkError> {
    let profile: AgentProfile =
        serde_json::from_str(&prepared.profile_json).map_err(|err| SdkError::InvalidArgument {
            message: format!("profile_json: {err}"),
        })?;
    let host = AgentHost::from_spec(
        &profile,
        &prepared.api_key,
        PathBuf::from(&prepared.project_root),
    )
    .map_err(map_host)?;
    let session_id = format!("{}:{}", prepared.dest, prepared.profile_id);
    let mut outcome = prompt(&host, &session_id, &prepared.text).await?;
    let mut yields = 0;
    loop {
        match outcome {
            TurnOutcome::Finished { text, .. } => {
                return Ok(AgentRunResult::ok(
                    prepared.dest.clone(),
                    prepared.profile_id.clone(),
                    prepared.epoch,
                    text,
                ));
            }
            TurnOutcome::Cancelled { .. } => return Ok(failed(&prepared, "aborted")),
            TurnOutcome::TimedOut { .. } => return Ok(failed(&prepared, "idle_timeout")),
            TurnOutcome::Yielded {
                kind,
                call_id,
                name,
            } => {
                yields += 1;
                if yields > MAX_YIELDS {
                    return Ok(failed(&prepared, "yield_abandoned"));
                }
                let pending = host.pending_yields(&session_id).await;
                let slot = pending.into_iter().find(|item| item.call_id == call_id);
                let arguments = slot
                    .as_ref()
                    .map(|item| item.arguments_json.clone())
                    .unwrap_or_else(|| "{}".into());
                let output = match kind {
                    YieldKind::ActionRequired => {
                        let preview = slot
                            .as_ref()
                            .map(|item| item.prompt.clone())
                            .filter(|text| !text.is_empty())
                            .unwrap_or_else(|| name.clone());
                        let wait = runtime.permissions.publish(PermissionEvent {
                            dest: prepared.dest.clone(),
                            call_id: call_id.clone(),
                            name: name.clone(),
                            preview,
                        });
                        let allow = tokio::time::timeout(Duration::from_secs(120), wait)
                            .await
                            .map_err(|_| SdkError::Internal {
                                message: "permission timed out".into(),
                            })?
                            .map_err(|_| SdkError::Internal {
                                message: "permission waiter dropped".into(),
                            })?;
                        if !allow {
                            return Ok(failed(&prepared, "permission_denied"));
                        }
                        serde_json::json!({"ok": true, "permission": "allow"}).to_string()
                    }
                    YieldKind::ToolRequest => {
                        execute_im_tool(&runtime.sdk, &name, &arguments, &prepared.dest).await
                    }
                };
                outcome = complete(&host, &session_id, &call_id, &output).await?;
            }
        }
    }
}

async fn prompt(host: &AgentHost, session_id: &str, text: &str) -> Result<TurnOutcome, SdkError> {
    let (tx, rx) = mpsc::channel::<HostEvent>(64);
    let pump = drain(rx);
    let cancel = CancellationToken::new();
    let outcome = host
        .prompt(session_id, text, tx, cancel)
        .await
        .map_err(map_host)?;
    let _ = pump.await;
    Ok(outcome)
}

async fn complete(
    host: &AgentHost,
    session_id: &str,
    call_id: &str,
    output: &str,
) -> Result<TurnOutcome, SdkError> {
    let (tx, rx) = mpsc::channel::<HostEvent>(32);
    let pump = drain(rx);
    let cancel = CancellationToken::new();
    let outcome = host
        .complete_tool(session_id, call_id, output, tx, cancel)
        .await
        .map_err(map_host)?;
    let _ = pump.await;
    Ok(outcome)
}

fn drain(mut rx: mpsc::Receiver<HostEvent>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move { while rx.recv().await.is_some() {} })
}

fn failed(prepared: &PreparedTurn, reason: &str) -> AgentRunResult {
    AgentRunResult {
        dest: prepared.dest.clone(),
        profile_id: prepared.profile_id.clone(),
        epoch: prepared.epoch,
        output: String::new(),
        error: Some(reason.to_string()),
        stop_reason: reason.to_string(),
        replied: false,
        visible: false,
        recently_active: false,
    }
}

fn map_host(err: kim_agent_host::HostError) -> SdkError {
    SdkError::Internal {
        message: err.to_string(),
    }
}
