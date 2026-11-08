mod children;
mod outcome;
mod pairing;
mod poison;
mod recovery;
mod steer;
mod timeout;

use std::sync::Arc;
use std::time::{Duration, Instant};

use goose_agent::machine::StateMachine;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::events::{
    stop_reason_finished, CancelReason, HostError, HostEvent, ProviderFail, TimeoutKind,
    TurnOutcome,
};
use crate::machine::MachineFactory;
use crate::ops::compaction::CompactionOp;
use crate::profile::{AgentProfile, HarnessSpec};
use crate::{AgentHost, HostSession};

pub use children::{kill_child_process, kill_group, prepare};
pub use poison::{install_panic_hook, HostPoison};
pub use steer::SteerInbox;
pub use timeout::{HardDeadline, IdleClock};

#[derive(Debug, Clone)]
pub struct HarnessLimits {
    pub idle: Duration,
    pub hard: Duration,
    pub keepalive: Duration,
    pub recently_active: Duration,
    pub cancel_grace: Duration,
    pub yield_wait: Duration,
    pub bash: Duration,
    pub mcp_tool: Duration,
    pub max_compaction_recoveries: u32,
    pub max_token_recoveries: u32,
    pub max_rate_limit_retries: u32,
    pub max_rate_limit_wait: Duration,
    pub prompt_bytes: usize,
    pub tool_result_bytes: usize,
    pub max_mcp_servers: usize,
    pub max_mcp_tools: usize,
}

impl Default for HarnessLimits {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(120),
            hard: Duration::from_secs(600),
            keepalive: Duration::from_secs(30),
            recently_active: Duration::from_secs(60),
            cancel_grace: Duration::from_secs(5),
            yield_wait: Duration::from_secs(15 * 60),
            bash: Duration::from_secs(30),
            mcp_tool: Duration::from_secs(120),
            max_compaction_recoveries: 3,
            max_token_recoveries: 3,
            max_rate_limit_retries: 3,
            max_rate_limit_wait: Duration::from_secs(20),
            prompt_bytes: 1024 * 1024,
            tool_result_bytes: 50 * 1024,
            max_mcp_servers: 16,
            max_mcp_tools: 128,
        }
    }
}

impl HarnessLimits {
    pub fn disabled() -> Self {
        Self {
            idle: Duration::MAX,
            hard: Duration::MAX,
            yield_wait: Duration::MAX,
            ..Self::default()
        }
    }

    pub fn from_spec(spec: &HarnessSpec) -> Self {
        if !spec.enabled {
            return Self::disabled();
        }
        let mut limits = Self::default();
        if let Some(secs) = spec.idle_timeout_secs {
            limits.idle = Duration::from_secs(secs);
        }
        if let Some(secs) = spec.hard_timeout_secs {
            limits.hard = Duration::from_secs(secs);
        }
        if let Some(secs) = spec.bash_timeout_secs {
            limits.bash = Duration::from_secs(secs);
        }
        if let Some(secs) = spec.mcp_tool_timeout_secs {
            limits.mcp_tool = Duration::from_secs(secs);
        }
        if let Some(secs) = spec.yield_wait_secs {
            limits.yield_wait = Duration::from_secs(secs);
        }
        limits
    }
}

pub fn resolve_limits(
    harness_json: &str,
    profile: Option<&HarnessSpec>,
) -> Result<HarnessLimits, HostError> {
    let spec = if !harness_json.trim().is_empty() {
        Some(
            serde_json::from_str::<HarnessSpec>(harness_json)
                .map_err(|err| HostError::Failed(format!("harness_json: {err}")))?,
        )
    } else {
        profile.cloned()
    };
    Ok(match spec {
        Some(spec) => HarnessLimits::from_spec(&spec),
        None => HarnessLimits::disabled(),
    })
}

pub fn bash_timeout(profile: &AgentProfile) -> Duration {
    profile
        .harness
        .as_ref()
        .filter(|spec| spec.enabled)
        .and_then(|spec| spec.bash_timeout_secs)
        .map(Duration::from_secs)
        .unwrap_or_else(|| HarnessLimits::default().bash)
}

pub(crate) struct SessionGuard {
    host: AgentHost,
    session_id: String,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if let Ok(mut busy) = self.host.inner.busy.lock() {
            busy.remove(&self.session_id);
        }
    }
}

impl AgentHost {
    pub fn with_limits(self, limits: HarnessLimits) -> Self {
        if let Ok(mut slot) = self.inner.limits.write() {
            *slot = limits.clone();
        }
        self.inner.mcp.set_bounds(
            limits.mcp_tool,
            limits.max_mcp_servers,
            limits.max_mcp_tools,
        );
        self
    }

    pub fn limits(&self) -> HarnessLimits {
        self.inner
            .limits
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    pub(crate) fn acquire_session(&self, session_id: &str) -> Result<SessionGuard, HostError> {
        let mut busy = self
            .inner
            .busy
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if !busy.insert(session_id.to_string()) {
            return Err(HostError::Busy(session_id.to_string()));
        }
        drop(busy);
        Ok(SessionGuard {
            host: self.clone(),
            session_id: session_id.to_string(),
        })
    }

    pub(crate) fn reset_hard_budget(&self, session_id: &str) {
        let hard = self.limits().hard;
        if let Ok(mut slot) = self.inner.hard_remaining.lock() {
            slot.insert(session_id.to_string(), hard);
        }
    }

    pub(crate) fn hard_budget(&self, session_id: &str) -> Duration {
        self.inner
            .hard_remaining
            .lock()
            .ok()
            .and_then(|slot| slot.get(session_id).copied())
            .unwrap_or_else(|| self.limits().hard)
    }

    pub(crate) fn charge_hard_budget(&self, session_id: &str, elapsed: Duration) {
        if let Ok(mut slot) = self.inner.hard_remaining.lock() {
            let left = slot
                .get(session_id)
                .copied()
                .unwrap_or_else(|| self.limits().hard);
            slot.insert(session_id.to_string(), left.saturating_sub(elapsed));
        }
    }

    pub async fn last_usage(
        &self,
        session_id: &str,
    ) -> Option<goose_provider_types::conversation::token_usage::ProviderUsage> {
        self.inner
            .last_usage
            .lock()
            .ok()
            .and_then(|slot| slot.get(session_id).cloned())
    }

    pub async fn steer(&self, session_id: &str, text: &str) -> Result<(), HostError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(HostError::Failed("empty steer".into()));
        }
        let running = self
            .inner
            .busy
            .lock()
            .map(|slot| slot.contains(session_id))
            .unwrap_or(false);
        let yielded = !self.pending_yields(session_id).await.is_empty();
        if !running && !yielded {
            return Err(HostError::Failed("not running".into()));
        }
        self.inner
            .steer
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(session_id, text.to_string());
        Ok(())
    }

    pub async fn repair_pairing(&self, session_id: &str) {
        let mut store = self.inner.store.lock().await;
        let Some(conversation) = store.conversations.get_mut(session_id) else {
            return;
        };
        pairing::repair_conversation(conversation);
        let _ = crate::persist_session(&store, session_id);
    }

    /// Pair in-process tools only. Kim-world yields and confirmation cards stay
    /// unanswered so `resume()` can replay them.
    pub async fn repair_in_process_pairing(&self, session_id: &str) {
        let names = self.profile_snapshot().project_toolset().kim_world_names();
        let mut store = self.inner.store.lock().await;
        let Some(conversation) = store.conversations.get_mut(session_id) else {
            return;
        };
        pairing::repair_in_process(conversation, &names);
        let _ = crate::persist_session(&store, session_id);
    }

    pub async fn abort_session(&self, session_id: &str) -> Result<TurnOutcome, HostError> {
        if let Some(token) = self
            .inner
            .active_cancel
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
        {
            token.cancel();
        }
        self.repair_pairing(session_id).await;
        Ok(TurnOutcome::Cancelled {
            reason: CancelReason::UserAbort,
        })
    }

    pub(crate) async fn run_loop(
        &self,
        session_id: &str,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError> {
        self.run_supervised(session_id, events, cancel).await
    }

    async fn run_supervised(
        &self,
        session_id: &str,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError> {
        let limits = self.limits();
        let profile_id = self.profile_snapshot().id.clone();
        tracing::info!(session_id, profile_id = %profile_id, "harness.turn_start");
        {
            let mut slot = self
                .inner
                .active_cancel
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            *slot = Some(cancel.clone());
        }
        let _cancel_guard = CancelGuard { host: self.clone() };

        self.drain_steer(session_id).await?;

        let mut compact_left = limits.max_compaction_recoveries;
        let mut token_left = limits.max_token_recoveries;
        let mut rate_left = limits.max_rate_limit_retries;
        let mut stripped_images = false;
        let probe = recovery::ProviderProbe::wrap(Arc::clone(&self.inner.provider));

        loop {
            let started = Instant::now();
            let idle = IdleClock::new(limits.idle);
            {
                let mut slot = self
                    .inner
                    .idle
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                *slot = Some(Arc::clone(&idle));
            }
            {
                let mut slot = self
                    .inner
                    .turn_tx
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                *slot = Some(events.clone());
            }
            let hard = HardDeadline::from_remaining(self.hard_budget(session_id));
            let ka_task = spawn_keepalive(events.clone(), limits.keepalive);

            let (tx, mut rx) = mpsc::channel(64);
            let emit = goose_agent::operation::Emitter::new(tx, cancel.clone());
            let events_for_pump = events.clone();
            let idle_for_pump = Arc::clone(&idle);
            let pump = tokio::spawn(async move {
                while let Some(ev) = rx.recv().await {
                    if let goose_agent::events::AgentEvent::Message(message) = ev {
                        crate::pump_message(&events_for_pump, &message, Some(&idle_for_pump)).await;
                    }
                }
            });

            let profile = self.profile_snapshot();
            let steps = MachineFactory::assemble_tracked(
                &profile,
                Arc::clone(&probe) as Arc<dyn goose_provider_types::base::Provider>,
                self.inner.model.clone(),
                &self.inner.project_root,
                Arc::clone(&self.inner.mcp),
                Arc::clone(&self.inner.input_tokens),
            );
            let machine = StateMachine::new(steps, cancel.clone());
            let host = self.clone();
            let sid = session_id.to_string();
            let mut run = tokio::spawn(async move { machine.run(&host, &sid, &emit).await });

            enum Supervised {
                Join(Result<HostSession, anyhow::Error>),
                Stop(TurnOutcome),
                Poisoned(String),
            }

            let supervised = tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    Supervised::Stop(TurnOutcome::Cancelled { reason: CancelReason::UserAbort })
                }
                _ = idle.sleep() => {
                    tracing::info!(session_id, "harness.idle_timeout");
                    Supervised::Stop(TurnOutcome::TimedOut { kind: TimeoutKind::Idle })
                }
                _ = hard.sleep() => {
                    let recently_active = idle.recently_active(limits.recently_active);
                    tracing::info!(session_id, recently_active, "harness.hard_timeout");
                    Supervised::Stop(TurnOutcome::TimedOut {
                        kind: TimeoutKind::Hard { recently_active },
                    })
                }
                join = &mut run => match join {
                    Ok(Ok(session)) => Supervised::Join(Ok(session)),
                    Ok(Err(err)) => Supervised::Join(Err(err)),
                    Err(err) if err.is_panic() => {
                        Supervised::Poisoned(HostPoison::MachinePanic.message().into())
                    }
                    Err(err) => Supervised::Poisoned(err.to_string()),
                },
            };
            ka_task.abort();
            let elapsed = started.elapsed();

            let join_result = match supervised {
                Supervised::Poisoned(message) => {
                    let recently_active = idle.recently_active(limits.recently_active);
                    clear_hooks(self);
                    let _ = pump.await;
                    return Err(HostError::Poisoned {
                        message,
                        recently_active,
                    });
                }
                Supervised::Stop(outcome) => {
                    if let Err(message) =
                        join_or_poison(&mut run, &cancel, limits.cancel_grace).await
                    {
                        let recently_active = idle.recently_active(limits.recently_active);
                        clear_hooks(self);
                        let _ = pump.await;
                        return Err(HostError::Poisoned {
                            message,
                            recently_active,
                        });
                    }
                    self.charge_hard_budget(session_id, elapsed);
                    self.repair_pairing(session_id).await;
                    clear_hooks(self);
                    let _ = pump.await;
                    log_end(session_id, &outcome);
                    return Ok(outcome);
                }
                Supervised::Join(result) => {
                    self.charge_hard_budget(session_id, elapsed);
                    clear_hooks(self);
                    let _ = pump.await;
                    result
                }
            };

            let session = match join_result {
                Ok(session) => session,
                Err(err) => return Err(HostError::Failed(err.to_string())),
            };

            let usage = self.last_usage(session_id).await;
            let class = outcome::classify_from_conversation(
                &session.conversation,
                &profile.project_toolset(),
                probe.take_fail(),
                usage.as_ref(),
            );
            match class {
                outcome::Classify::Yield(y) => {
                    log_end(session_id, &y);
                    return Ok(y);
                }
                outcome::Classify::Finished(vis) => {
                    let _ = self
                        .replace_conversation(
                            session_id,
                            outcome::strip_empty_placeholder(&session.conversation),
                        )
                        .await;
                    let outcome = TurnOutcome::Finished {
                        text: vis.text.clone(),
                        replied: vis.replied,
                        visible: vis.visible,
                    };
                    tracing::info!(
                        session_id,
                        outcome = "finished",
                        visible = vis.visible,
                        send_ok = vis.send_ok,
                        stop_reason = stop_reason_finished(vis.replied, vis.visible),
                        "harness.turn_end"
                    );
                    return Ok(outcome);
                }
                outcome::Classify::Recover(kind) => {
                    let recovered = self
                        .recover(
                            session_id,
                            &probe,
                            kind,
                            &mut compact_left,
                            &mut token_left,
                            &mut rate_left,
                            &mut stripped_images,
                            &limits,
                            &cancel,
                        )
                        .await?;
                    if let Some(outcome) = recovered {
                        log_end(session_id, &outcome);
                        return Ok(outcome);
                    }
                }
                outcome::Classify::Provider(fail) => {
                    tracing::info!(session_id, kind = %fail, "harness.recover_exhausted");
                    return Err(HostError::Provider(fail));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn recover(
        &self,
        session_id: &str,
        probe: &Arc<recovery::ProviderProbe>,
        kind: outcome::RecoverKind,
        compact_left: &mut u32,
        token_left: &mut u32,
        rate_left: &mut u32,
        stripped_images: &mut bool,
        limits: &HarnessLimits,
        cancel: &CancellationToken,
    ) -> Result<Option<TurnOutcome>, HostError> {
        match kind {
            outcome::RecoverKind::Context if *compact_left > 0 => {
                *compact_left -= 1;
                tracing::info!(
                    session_id,
                    kind = "context",
                    budget_left = *compact_left,
                    "harness.recover"
                );
                let _ = probe.take_fail();
                self.force_compact(session_id, probe, limits).await?;
                let _ = probe.take_fail();
                Ok(None)
            }
            outcome::RecoverKind::Truncated if *token_left > 0 => {
                *token_left -= 1;
                tracing::info!(
                    session_id,
                    kind = "truncated",
                    budget_left = *token_left,
                    "harness.recover"
                );
                let conversation = self.conversation_clone(session_id).await?;
                let conversation = outcome::clear_output_limit_flags(&conversation);
                let conversation = outcome::append_continue_hidden(&conversation);
                self.replace_conversation(session_id, conversation).await?;
                Ok(None)
            }
            outcome::RecoverKind::RateLimit { wait } if *rate_left > 0 => {
                *rate_left -= 1;
                let capped = wait.min(limits.max_rate_limit_wait);
                tracing::info!(
                    session_id,
                    kind = "rate_limit",
                    budget_left = *rate_left,
                    "harness.recover"
                );
                let started = Instant::now();
                tokio::select! {
                    _ = tokio::time::sleep(capped) => {}
                    _ = cancel.cancelled() => {
                        self.repair_pairing(session_id).await;
                        return Ok(Some(TurnOutcome::Cancelled { reason: CancelReason::UserAbort }));
                    }
                }
                self.charge_hard_budget(session_id, started.elapsed());
                let conversation = self.conversation_clone(session_id).await?;
                self.replace_conversation(session_id, outcome::strip_trailing_error(&conversation))
                    .await?;
                let _ = probe.take_fail();
                Ok(None)
            }
            outcome::RecoverKind::StripImages if !*stripped_images => {
                *stripped_images = true;
                tracing::info!(session_id, kind = "strip_images", "harness.recover");
                let conversation = self.conversation_clone(session_id).await?;
                let conversation = outcome::strip_images_keep_pairing(&conversation);
                let conversation = outcome::strip_trailing_error(&conversation);
                self.replace_conversation(session_id, conversation).await?;
                let _ = probe.take_fail();
                Ok(None)
            }
            outcome::RecoverKind::Context => {
                tracing::info!(kind = "context", "harness.recover_exhausted");
                Err(HostError::Provider(ProviderFail::ContextExceeded))
            }
            outcome::RecoverKind::Truncated => {
                tracing::info!(kind = "truncated", "harness.recover_exhausted");
                Err(HostError::Provider(ProviderFail::Truncated))
            }
            outcome::RecoverKind::RateLimit { .. } => {
                tracing::info!(kind = "rate_limit", "harness.recover_exhausted");
                Err(HostError::Provider(ProviderFail::RateLimited {
                    retry_after: None,
                }))
            }
            outcome::RecoverKind::StripImages => {
                tracing::info!(kind = "image", "harness.recover_exhausted");
                Err(HostError::Provider(ProviderFail::UnsupportedImage))
            }
        }
    }

    async fn force_compact(
        &self,
        session_id: &str,
        probe: &Arc<recovery::ProviderProbe>,
        limits: &HarnessLimits,
    ) -> Result<(), HostError> {
        let conversation = self.conversation_clone(session_id).await?;
        let conversation = outcome::strip_trailing_error(&conversation);
        let op = CompactionOp {
            provider: Arc::clone(probe) as Arc<dyn goose_provider_types::base::Provider>,
            model: self.inner.model.clone(),
            tool_result_bytes: limits.tool_result_bytes,
            input_tokens: Arc::clone(&self.inner.input_tokens),
        };
        let next = op.force(&conversation).await?;
        self.replace_conversation(session_id, next).await
    }

    async fn drain_steer(&self, session_id: &str) -> Result<(), HostError> {
        let pending = self
            .inner
            .steer
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .drain(session_id);
        if pending.is_empty() {
            return Ok(());
        }
        tracing::info!(n = pending.len(), "harness.steer_drain");
        let mut store = self.inner.store.lock().await;
        let conversation = crate::ensure_conversation(&mut store, session_id);
        for text in pending {
            conversation.push(
                goose_provider_types::conversation::message::Message::user()
                    .with_text(text)
                    .with_steer(),
            );
        }
        crate::persist_session(&store, session_id)
    }

    pub(crate) async fn conversation_clone(
        &self,
        session_id: &str,
    ) -> Result<goose_provider_types::conversation::Conversation, HostError> {
        let store = self.inner.store.lock().await;
        store
            .conversations
            .get(session_id)
            .cloned()
            .ok_or_else(|| HostError::UnknownSession(session_id.to_string()))
    }

    pub(crate) async fn replace_conversation(
        &self,
        session_id: &str,
        conversation: goose_provider_types::conversation::Conversation,
    ) -> Result<(), HostError> {
        let mut store = self.inner.store.lock().await;
        store
            .conversations
            .insert(session_id.to_string(), conversation);
        crate::persist_session(&store, session_id)
    }
}

struct CancelGuard {
    host: AgentHost,
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.host.inner.active_cancel.lock() {
            *slot = None;
        }
    }
}

fn clear_hooks(host: &AgentHost) {
    if let Ok(mut slot) = host.inner.idle.lock() {
        *slot = None;
    }
    if let Ok(mut slot) = host.inner.turn_tx.lock() {
        *slot = None;
    }
}

fn log_end(session_id: &str, outcome: &TurnOutcome) {
    let name = match outcome {
        TurnOutcome::Finished { .. } => "finished",
        TurnOutcome::Yielded { .. } => "yielded",
        TurnOutcome::Cancelled { .. } => "cancelled",
        TurnOutcome::TimedOut {
            kind: TimeoutKind::Idle,
        } => "idle_timeout",
        TurnOutcome::TimedOut {
            kind: TimeoutKind::Hard { .. },
        } => "hard_timeout",
    };
    tracing::info!(session_id, outcome = name, "harness.turn_end");
}

async fn join_or_poison(
    run: &mut tokio::task::JoinHandle<anyhow::Result<HostSession>>,
    cancel: &CancellationToken,
    grace: Duration,
) -> Result<(), String> {
    cancel.cancel();
    match tokio::time::timeout(grace, &mut *run).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(err)) if err.is_panic() => Err(HostPoison::MachinePanic.message().into()),
        Ok(Err(err)) => Err(err.to_string()),
        Err(_elapsed) => {
            run.abort();
            let _ = run.await;
            Err(HostPoison::CancelGrace.message().into())
        }
    }
}

fn spawn_keepalive(tx: mpsc::Sender<HostEvent>, every: Duration) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let every = if every.is_zero() {
            Duration::from_millis(1)
        } else {
            every
        };
        if every >= Duration::from_secs(24 * 60 * 60) {
            std::future::pending::<()>().await;
            return;
        }
        loop {
            tracing::debug!("harness.keepalive");
            match tx.try_send(HostEvent::Keepalive) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Closed(_)) => break,
                Err(mpsc::error::TrySendError::Full(_)) => {}
            }
            timeout::sleep_for(every).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use goose_provider_types::conversation::message::Message;
    use goose_provider_types::errors::ProviderError;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::profile::LegacyOpenOpts;
    use crate::scripted::ScriptedProvider;
    use crate::AgentProfile;

    fn profile() -> AgentProfile {
        AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            ..LegacyOpenOpts::default()
        })
    }

    fn host_with(provider: ScriptedProvider, limits: HarnessLimits) -> AgentHost {
        AgentHost::from_provider_for_test(profile(), Arc::new(provider), PathBuf::from("/tmp"))
            .expect("host")
            .with_limits(limits)
    }

    fn short_limits() -> HarnessLimits {
        HarnessLimits {
            idle: Duration::from_millis(80),
            hard: Duration::from_secs(30),
            keepalive: Duration::from_millis(15),
            cancel_grace: Duration::from_secs(2),
            ..HarnessLimits::default()
        }
    }

    #[test]
    fn empty_harness_json_is_max() {
        let limits = resolve_limits("", None).expect("limits");
        assert_eq!(limits.idle, Duration::MAX);
        assert_eq!(limits.hard, Duration::MAX);
        assert_eq!(limits.yield_wait, Duration::MAX);
    }

    #[tokio::test]
    async fn idle_timeout_fires_when_scripted_provider_hangs() {
        let host = host_with(ScriptedProvider::hanging(), short_limits());
        let (tx, _rx) = mpsc::channel(8);
        let outcome = host
            .prompt("s", "hello", tx, CancellationToken::new())
            .await
            .expect("outcome");
        assert!(matches!(
            outcome,
            TurnOutcome::TimedOut {
                kind: TimeoutKind::Idle
            }
        ));
    }

    #[tokio::test]
    async fn idle_timeout_no_further_append_message() {
        let host = host_with(ScriptedProvider::hanging(), short_limits());
        let (tx, _rx) = mpsc::channel(8);
        let _ = host
            .prompt("s", "hello", tx, CancellationToken::new())
            .await
            .expect("outcome");
        let before = host.conversation_for_test("s").await.messages().len();
        tokio::time::sleep(Duration::from_millis(80)).await;
        let after = host.conversation_for_test("s").await.messages().len();
        assert_eq!(before, after);
    }

    #[tokio::test]
    async fn keepalive_does_not_reset_idle() {
        let host = host_with(ScriptedProvider::hanging(), short_limits());
        let (tx, mut rx) = mpsc::channel(32);
        let started = Instant::now();
        let outcome = tokio::time::timeout(
            Duration::from_secs(2),
            host.prompt("s", "hello", tx, CancellationToken::new()),
        )
        .await
        .expect("idle must win before 2s")
        .expect("outcome");
        assert!(started.elapsed() < Duration::from_millis(800));
        assert!(matches!(
            outcome,
            TurnOutcome::TimedOut {
                kind: TimeoutKind::Idle
            }
        ));
        let mut kinds = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            kinds.push(match ev {
                HostEvent::Keepalive => "ka",
                HostEvent::TextDelta { .. } => "text",
                HostEvent::Usage { .. } => "usage",
                _ => "other",
            });
        }
        assert!(
            kinds.contains(&"ka"),
            "events={kinds:?} elapsed={:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn hard_timeout_fires_despite_keepalive() {
        let limits = HarnessLimits {
            idle: Duration::from_secs(30),
            hard: Duration::from_millis(80),
            keepalive: Duration::from_millis(15),
            cancel_grace: Duration::from_secs(2),
            ..HarnessLimits::default()
        };
        let host = host_with(ScriptedProvider::hanging(), limits);
        let (tx, _rx) = mpsc::channel(8);
        let outcome = host
            .prompt("s", "hello", tx, CancellationToken::new())
            .await
            .expect("outcome");
        match outcome {
            TurnOutcome::TimedOut {
                kind: TimeoutKind::Hard { recently_active },
            } => assert!(!recently_active),
            other => panic!("expected hard timeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn session_busy_rejects_reentrant_prompt() {
        let host = host_with(ScriptedProvider::hanging(), short_limits());
        let host2 = host.clone();
        let (tx, _rx) = mpsc::channel(4);
        let first = tokio::spawn(async move {
            host.prompt("s", "hello", tx, CancellationToken::new())
                .await
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        let (tx2, _rx2) = mpsc::channel(4);
        let err = host2
            .prompt("s", "again", tx2, CancellationToken::new())
            .await
            .expect_err("busy");
        assert!(matches!(err, HostError::Busy(_)));
        let _ = first.await;
    }

    #[tokio::test]
    async fn prompt_rejects_oversize_bytes() {
        let host = host_with(
            ScriptedProvider::new(vec![Message::assistant().with_text("ok")]),
            HarnessLimits {
                prompt_bytes: 4,
                ..HarnessLimits::default()
            },
        );
        let (tx, _rx) = mpsc::channel(2);
        let err = host
            .prompt("s", "hello", tx, CancellationToken::new())
            .await
            .expect_err("oversize");
        assert!(matches!(err, HostError::Failed(_)));
    }

    #[tokio::test]
    async fn steer_does_not_abort_in_flight_inference() {
        let host = host_with(ScriptedProvider::hang_once(), short_limits());
        let host2 = host.clone();
        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();
        let (tx, _rx) = mpsc::channel(4);
        let task = tokio::spawn(async move { host.prompt("s", "hello", tx, cancel2).await });
        tokio::time::sleep(Duration::from_millis(40)).await;
        host2
            .steer("s", "also check the logs")
            .await
            .expect("steer");
        assert!(!task.is_finished());
        cancel.cancel();
        let outcome = task.await.expect("join").expect("outcome");
        assert!(matches!(
            outcome,
            TurnOutcome::Cancelled {
                reason: CancelReason::UserAbort
            }
        ));
    }

    #[tokio::test]
    async fn steer_folds_user_message_on_next_round() {
        let host = host_with(
            ScriptedProvider::hang_once().with_followup(Message::assistant().with_text("folded")),
            short_limits(),
        );
        let host2 = host.clone();
        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();
        let (tx, _rx) = mpsc::channel(4);
        let task = tokio::spawn(async move { host.prompt("s", "hello", tx, cancel2).await });
        tokio::time::sleep(Duration::from_millis(40)).await;
        host2.steer("s", "fold me").await.expect("steer");
        cancel.cancel();
        let _ = task.await;
        let (tx, _rx) = mpsc::channel(4);
        let outcome = host2
            .prompt("s", "continue", tx, CancellationToken::new())
            .await
            .expect("second");
        assert!(matches!(outcome, TurnOutcome::Finished { .. }));
        let text = host2
            .conversation_for_test("s")
            .await
            .messages()
            .iter()
            .map(|m| m.as_concat_text())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("fold me"), "{text}");
    }

    #[tokio::test]
    async fn compaction_on_context_exceeded_retries_once() {
        let provider = ScriptedProvider::new(vec![
            Message::assistant().with_text("older turn"),
            Message::assistant().with_text("recovered"),
        ])
        .with_errors(vec![ProviderError::ContextLengthExceeded(
            "too long".into(),
        )])
        .delay_errors(1)
        .with_summarizer(vec![Message::assistant().with_text("summary of older")]);
        let host = host_with(provider, HarnessLimits::default());
        let (tx, _rx) = mpsc::channel(8);
        let outcome = host
            .prompt("s", "hello", tx, CancellationToken::new())
            .await
            .expect("first");
        assert!(matches!(outcome, TurnOutcome::Finished { .. }));
        let (tx, _rx) = mpsc::channel(8);
        let outcome = host
            .prompt("s", "again", tx, CancellationToken::new())
            .await
            .expect("recovered");
        match outcome {
            TurnOutcome::Finished { text, replied, .. } => {
                assert!(replied);
                assert!(text.contains("recovered"), "{text}");
            }
            other => panic!("expected finished, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn compaction_budget_exhausted_surfaces_provider() {
        let provider = ScriptedProvider::new(vec![])
            .with_errors(vec![
                ProviderError::ContextLengthExceeded("a".into()),
                ProviderError::ContextLengthExceeded("b".into()),
            ])
            .with_summarizer(vec![Message::assistant().with_text("sum")]);
        let host = host_with(
            provider,
            HarnessLimits {
                max_compaction_recoveries: 1,
                ..HarnessLimits::default()
            },
        );
        let (tx, _rx) = mpsc::channel(4);
        let err = host
            .prompt("s", "hello", tx, CancellationToken::new())
            .await
            .expect_err("budget");
        assert!(matches!(
            err,
            HostError::Provider(ProviderFail::ContextExceeded)
        ));
    }

    #[tokio::test]
    async fn rate_limited_retries_with_capped_wait() {
        let provider = ScriptedProvider::new(vec![Message::assistant().with_text("after wait")])
            .with_errors(vec![ProviderError::RateLimitExceeded {
                details: "slow".into(),
                retry_delay: Some(Duration::from_secs(60)),
            }]);
        let host = host_with(
            provider,
            HarnessLimits {
                max_rate_limit_wait: Duration::from_millis(20),
                ..HarnessLimits::default()
            },
        );
        let started = Instant::now();
        let (tx, _rx) = mpsc::channel(4);
        let outcome = host
            .prompt("s", "hello", tx, CancellationToken::new())
            .await
            .expect("rate");
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(matches!(
            outcome,
            TurnOutcome::Finished { replied: true, .. }
        ));
    }

    #[tokio::test]
    async fn max_tokens_recovery_budget() {
        let mut partial = Message::assistant().with_text("partial");
        partial.metadata.output_token_limit_reached = true;
        let provider = ScriptedProvider::new(vec![
            partial,
            Message::assistant().with_text("finished the thought"),
        ]);
        let host = host_with(provider, HarnessLimits::default());
        let (tx, _rx) = mpsc::channel(4);
        let outcome = host
            .prompt("s", "hello", tx, CancellationToken::new())
            .await
            .expect("tokens");
        match outcome {
            TurnOutcome::Finished { text, .. } => {
                assert!(text.contains("finished"), "{text}");
            }
            other => panic!("expected finished, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn poisoned_host_recreated_from_persist() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("session.json");
        let host = host_with(
            ScriptedProvider::new(vec![Message::assistant().with_text("saved")]),
            HarnessLimits::default(),
        );
        host.configure_persist(Some(path.clone()), true).await;
        let (tx, _rx) = mpsc::channel(4);
        host.prompt("s", "hello", tx, CancellationToken::new())
            .await
            .expect("first");
        let state = host.runtime_state().await;
        let host2 = host_with(
            ScriptedProvider::new(vec![Message::assistant().with_text("again")]),
            HarnessLimits::default(),
        );
        host2.restore_runtime_state(state).await;
        let (tx, _rx) = mpsc::channel(4);
        let outcome = host2
            .prompt("s", "next", tx, CancellationToken::new())
            .await
            .expect("second");
        assert!(matches!(outcome, TurnOutcome::Finished { .. }));
        assert!(path.exists());
    }

    #[tokio::test]
    async fn cancel_leaves_history_valid_for_next_prompt() {
        let mut args = rmcp::model::JsonObject::new();
        args.insert("path".into(), serde_json::json!("note.txt"));
        let mut call = rmcp::model::CallToolRequestParams::new("read_file");
        call.arguments = Some(args);
        let provider = ScriptedProvider::new(vec![
            Message::assistant().with_tool_request("c1", Ok(call)),
            Message::assistant().with_text("done"),
        ]);
        let dir = tempfile::tempdir().expect("dir");
        let host = AgentHost::from_provider_for_test(
            AgentProfile::from_legacy(&LegacyOpenOpts {
                enable_fs_tools: true,
                model: "scripted".into(),
                llm_backend: "scripted".into(),
                ..LegacyOpenOpts::default()
            }),
            Arc::new(provider),
            dir.path().to_path_buf(),
        )
        .expect("host");
        let (tx, _rx) = mpsc::channel(8);
        let cancel = CancellationToken::new();
        let running = host.prompt("s", "read", tx, cancel.clone());
        tokio::pin!(running);
        tokio::select! {
            _ = &mut running => {}
            _ = tokio::time::sleep(Duration::from_millis(200)) => {
                cancel.cancel();
            }
        }
        let _ = host.abort_session("s").await;
        assert!(pairing::history_valid(
            &host.conversation_for_test("s").await
        ));
        let (tx, _rx) = mpsc::channel(4);
        let outcome = host
            .prompt("s", "again", tx, CancellationToken::new())
            .await
            .expect("next");
        assert!(matches!(
            outcome,
            TurnOutcome::Finished { .. } | TurnOutcome::Yielded { .. }
        ));
    }
}
