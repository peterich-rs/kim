# Agent runtime (Goose / Codex) Fixes

Issue 1 [P0]: Submit the final reply even when FRB stream cancel hangs
Issue 2 [P1]: Bound Codex thread startup with cancel and timeout
Issue 3 [P1]: Shut Codex down on close and re-attach it on host recreate
Issue 4 [P1]: Disable WebSockets on the official OpenAI provider
Issue 5 [P2]: Keep sync `listen` off the UI-thread tokio context
Issue 6 [P2]: Build embed Config through ConfigBuilder, not a sample literal
Issue 7 [P2]: Keep one CodexThread per dest×profile

Do not edit `third_party/codex`. Pin stays `be2951ea34` (`rust-v0.155.1`). Facade gaps are handled in `kim-agent-host`.

---

Issue 1 [P0]: Submit the final reply even when FRB stream cancel hangs

File: `sdk/mobile/lib/bridge/agent_bridge.dart` — `driveSession` (lines 321-395)

Secondary:
- `sdk/mobile/test/agent/drive_session_stop_reason_test.dart` — `_StopSession` (lines 35-91)
- `research/agent-frb-stream-cancel.dart` (already reproduces the native hang)

Problem: Goose (and Codex on a normal finish) emit `assistant_finished` / a quiet `stopReason`. `driveSession` is ready to return `DriveResult.fromEvent` at line 371, but `finally` at line 384 does `await sub.cancel()` first. The pinned FRB 2.13 `RustStreamSink` is an `async*` over `receivePort`; after the last event the generator sits on the next native message, so `cancel()` does not complete until another event arrives. `session.close()` at line 390 is what would emit `aborted` (`session.rs` `abort` lines 606-608), but it never runs. `submitAgentRun` at line 149 never runs, SDK typing stays on.

Goose `finish_turn` has already set phase Idle and released `complete_gate` before that event, so this is a Dart teardown deadlock, not a missing `completed` branch. `DriveResult.quiet` already includes `completed`.

Approach — close the session before cancelling the subscription, and bound both:

Closing first matches the probe in `research/agent-frb-stream-cancel.dart`: posting the close-generated event before `cancel()` returns immediately. Fire-and-forget `cancel()` would also unblock the return, but it leaves the native generator parked until some later event, and it hides close failures. A 1s timeout on `cancel()` after close is the backstop if abort does not emit.

```dart
    } finally {
      permissions?.detach(dest);
      try {
        await session.close().timeout(const Duration(seconds: 2));
      } on TimeoutException {
        KimLogger.warn('agent session close timed out');
      } catch (e, st) {
        KimLogger.warn('agent session close', e, st);
      }
      try {
        await sub.cancel().timeout(const Duration(seconds: 1));
      } on TimeoutException {
        KimLogger.warn('agent event subscription cancel timed out');
      }
      if (!inbox.isClosed) {
        await inbox.close();
      }
    }
```

Add `import 'dart:async' show TimeoutException;` only if it is not already exported via the existing `dart:async` import (line 4 already imports `dart:async`).

Existing test adjustments:
- `driveSession does not throw quiet stop reasons` (`drive_session_stop_reason_test.dart:94`): still valid; `_StopSession.close` is a no-op so order change is invisible.
- `driveSession throws DriveStop with typed stopReason` (line 120): same.

New tests:
- `driveSession returns completed when subscription.cancel never completes until close`. Setup: custom `AgentSessionPort` whose `listen()` subscription `cancel()` waits on a `Completer` that `close()` completes (and also emits `aborted`). Action: emit `assistant_finished` / `completed`. Assert: `driveSession` returns within 200ms with `stopReason == 'completed'`, and `close()` was invoked. Guards the production FRB order.
- Keep `research/agent-frb-stream-cancel.dart` as a manual native probe; do not wire it into `flutter test` (it talks to `NativeApi.postCObject`).

---

Issue 2 [P1]: Bound Codex thread startup with cancel and timeout

File: `crates/kim-agent-host/src/codex_drive.rs` — `codex_prompt` (lines 206-279), `ensure_live` (lines 425-457)

Problem: `codex_prompt` takes `self.inner.codex` at line 213 and holds it through `ensure_live` (line 223), `start_turn_if_idle` (lines 247-253), and `pump` (line 267). `arm_deadlines` runs at line 265, after startup. `pump`'s `select!` on `cancel` / idle / hard (lines 487-499) therefore never sees a stuck `ThreadManager::start_thread` / `EnvironmentManager::from_codex_home` / MCP spawn. `codex_pending` (line 387) and `snapshot` (`session.rs:715`) wait on the same mutex, so Dart `resume`/`snapshot` also stall. `start_prompt` (`session.rs:394`) holds `complete_gate` for that whole future; `abort` then waits `cancel_grace` (default 5s, `harness/mod.rs:57`) and only then `recreate_host`.

This is the machine-independent hang on the first Codex send. It does not by itself prove a Flutter UI-isolate freeze; it does prove the agent turn, abort, and status queries can sit forever.

Approach — start the thread without the slot lock, under `cancel` + a startup cap:

Do not wait for `Duration::MAX` when harness is disabled (`HarnessLimits::disabled`, `harness/mod.rs:74-80`). Use `min(limits.idle, 30s)` as `startup`, falling back to 30s when idle is `MAX`.

```rust
pub(crate) async fn codex_prompt(
    &self,
    text: &str,
    context_json: Option<&str>,
    events: mpsc::Sender<HostEvent>,
    cancel: CancellationToken,
) -> Result<TurnOutcome, HostError> {
    {
        let slot = self.inner.codex.lock().await;
        if slot.pending.is_some() {
            return Err(HostError::Failed("agent waiting for tool".into()));
        }
        if let Some(max) = self.profile_snapshot().max_turns {
            if slot.turns >= max {
                return Err(HostError::Failed("max turns".into()));
            }
        }
    }
    self.ensure_live(&cancel).await?;
    if cancel.is_cancelled() {
        return self.interrupted_from_slot().await;
    }

    let (thread, transcript, body) = {
        let mut slot = self.inner.codex.lock().await;
        let body = assemble_user_body(&slot, text, context_json);
        let thread = live_thread(&slot)?;
        let transcript = slot.launch.as_ref().and_then(|l| l.transcript.clone());
        (thread, transcript, body)
    };

    let submission = tokio::select! {
        _ = cancel.cancelled() => return self.interrupted_from_slot().await,
        submission = thread.start_turn_if_idle(TurnInputRequest::user_input(vec![
            UserInput::Text { text: body, text_elements: Vec::new() },
        ])) => submission.map_err(|err| HostError::Failed(err.to_string()))?,
    };
    if let StartIfIdleSubmission::NotSubmitted { reason } = submission {
        return Err(HostError::Failed(format!(
            "turn input was not submitted: {reason:?}"
        )));
    }

    let mut slot = self.inner.codex.lock().await;
    slot.turns = slot.turns.saturating_add(1);
    slot.send_ok = false;
    let (idle, hard) = self.arm_deadlines(true);
    slot.hard_deadline = hard;
    let outcome = self.pump(&mut slot, thread, events, cancel, idle).await?;
    // transcript append — unchanged
    Ok(outcome)
}

async fn ensure_live(&self, cancel: &CancellationToken) -> Result<(), HostError> {
    {
        let slot = self.inner.codex.lock().await;
        if slot.live.is_some() {
            return Ok(());
        }
    }
    let (launch, profile) = {
        let slot = self.inner.codex.lock().await;
        let launch = slot
            .launch
            .as_ref()
            .cloned()
            .ok_or_else(|| HostError::Failed("codex runtime is not configured".into()))?;
        (launch, self.profile_snapshot())
    };
    let mut opts = CodexEmbedOpts { /* same field fill as today, lines 434-442 */ };
    if !opts.cwd.is_absolute() {
        opts.cwd = std::env::current_dir().unwrap_or(opts.cwd);
    }
    let mut config = embed_config(&opts).await?;
    apply_profile(&mut config, &profile, &launch.api_key)?;
    let tools = kim_dynamic_tools(&profile);
    let startup = startup_budget(self.limits());
    let started = tokio::select! {
        _ = cancel.cancelled() => {
            return Err(HostError::Failed("codex startup cancelled".into()));
        }
        _ = tokio::time::sleep(startup) => {
            return Err(HostError::Failed("codex startup timed out".into()));
        }
        started = start_thread(config, &launch.api_key, tools) => started?,
    };
    let mut slot = self.inner.codex.lock().await;
    if slot.live.is_some() {
        let _ = started.thread.shutdown_and_wait().await;
        let _ = started.manager.remove_thread(&started.thread_id).await;
        return Ok(());
    }
    slot.live = Some(LiveThread {
        manager: started.manager,
        thread: started.thread,
        thread_id: started.thread_id,
    });
    Ok(())
}

fn startup_budget(limits: HarnessLimits) -> Duration {
    const CAP: Duration = Duration::from_secs(30);
    if limits.idle == Duration::MAX || limits.idle > CAP {
        CAP
    } else {
        limits.idle
    }
}
```

Structural changes:
- `CodexLaunch` must be `Clone` (it already is field-wise; add `#[derive(Clone)]` if missing).
- `embed_config` becomes `async` (Issue 6). Until Issue 6 lands, keep a sync `embed_config` and call it from `ensure_live` without `.await`.
- Extract `assemble_user_body` from lines 224-241 so `codex_prompt` stays readable.

Existing test adjustments:
- `crates/kim-agent-host/src/codex_rt.rs` `empty_api_key_returns_before_codex_home_io` (line 452) / `missing_helper_returns_before_codex_home_io` (line 468): still valid; they never reach `ensure_live`.

New tests:
- `startup_budget` unit test: disabled limits → 30s; idle 10s → 10s; idle 120s → 30s.
- `ensure_live` cancel: inject a `start_thread` future that pending-forever via `#[cfg(test)]` hook (`codex_drive` `start_thread_for_test`). Cancel the token; expect `HostError::Failed("codex startup cancelled")` within 1s and `slot.live.is_none()`.
- `ensure_live` timeout: same hook, no cancel, `startup_budget` overridden to 50ms; expect `"codex startup timed out"`.
- `codex_pending` during startup: while the test future is pending, `codex_pending().await` must return (empty) rather than wait on the startup future.

---

Issue 3 [P1]: Shut Codex down on close and re-attach it on host recreate

File: `sdk/mobile/rust_agent/src/api/session.rs` — `close` (lines 755-763), `recreate_host` (lines 1148-1173)

Secondary: `crates/kim-agent-host/src/codex_drive.rs` (new `shutdown_live`), `crates/kim-agent-host/src/lib.rs` `disconnect_extensions` (lines 336-338)

Problem: `close` calls `abort` then `host.disconnect_extensions()` (Goose MCP only). The live `CodexThread` / `ThreadManager` are dropped without `shutdown_and_wait` or `remove_thread`. The sample at `codex_rt.rs:111-112` and Codex `CodexThread::shutdown_and_wait` (`codex_thread.rs:240`) are the documented teardown. `recreate_host` builds a fresh Goose `AgentHost` from `ResolvedProfile` and never calls `attach_codex` (`session.rs:271-307`), so a poisoned Codex session comes back as Goose. `abort` on a stuck `ensure_live` waits `cancel_grace` then takes this path.

Approach — shut the thread down on host teardown, and restore Codex on recreate:

```rust
// session.rs close
pub async fn close(&self) -> Result<(), String> {
    let _ = self.abort().await;
    let host = {
        let guard = self.inner.host.read().await;
        guard.clone()
    };
    host.disconnect_extensions().await;
    #[cfg(feature = "codex")]
    host.shutdown_codex().await;
    Ok(())
}

// kim-agent-host
pub async fn shutdown_codex(&self) {
    #[cfg(feature = "codex")]
    {
        let mut slot = self.inner.codex.lock().await;
        if let Some(live) = slot.live.take() {
            let _ = tokio::time::timeout(
                Duration::from_secs(2),
                live.thread.shutdown_and_wait(),
            )
            .await;
            let _ = live.manager.remove_thread(&live.thread_id).await;
        }
        slot.pending = None;
        self.inner
            .codex_runtime
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }
}
```

`recreate_host` must pass through the original `SessionOpenOpts` (or a stored `CodexLaunch`) and call `attach_codex` when `resolved.profile.agent_runtime()` is Codex. Store `SessionOpenOpts` (or `sqlite_path` + `project_root` + `opts`) on `Shared` at `session_open` (lines 309-347) next to `resolved`.

```rust
async fn recreate_host(inner: &Shared) -> Result<(), String> {
    // existing from_resolved + limits …
    if let Some(opts) = inner.open_opts.lock().ok().and_then(|g| g.clone()) {
        attach_codex(
            &host,
            &opts,
            &resolved.profile,
            &inner.sqlite_for_codex,
            &inner.root_for_codex,
        )
        .await?;
    }
    // existing restore_runtime_state / swap
}
```

Do not call `connect_extensions` for Codex (same skip as `session_open` lines 339-341).

Existing test adjustments:
- Session scripted tests in `sdk/mobile/rust_agent/tests/session_scripted.rs` stay Goose; they should still pass because `shutdown_codex` is a no-op when `codex_runtime == 0`.

New tests:
- Host unit test with a real `use_codex` + fake/missing helper: `shutdown_codex` on a slot that only has `launch` (no `live`) returns, `codex_runtime` is 0.
- `recreate_host` after `use_codex`: `host.is_codex()` remains true. Fixture: open with `harness_json` `{"runtime":"codex"}` and a helper file on disk; poison via abort timeout hook if needed.

---

Issue 4 [P1]: Disable WebSockets on the official OpenAI provider

File: `crates/kim-agent-host/src/codex_inject.rs` — `apply_profile` (lines 77-81), `install_custom_provider` (lines 159-162)

Problem: Custom providers already set `supports_websockets = false` because the built-in OpenAI clone defaults to `true` (`model-provider-info/src/lib.rs:476`) and Codex tries `wss://…/responses` first. `official_openai` (lines 176-187) skips `install_custom_provider`, so the official path keeps WebSockets. A hung WS connect sits inside `start_thread` / the first turn, which Issue 2 then has to time out. Fix the provider flag rather than relying on the timeout.

Approach — always clear the flag after provider install:

```rust
    if official_openai(&profile.provider.kind, &profile.provider.base_url) {
        config.forced_login_method = Some(ForcedLoginMethod::Api);
    } else {
        install_custom_provider(config, profile, api_key)?;
    }
    config.model_provider.supports_websockets = false;
    if let Some(slot) = config.model_providers.get_mut(&config.model_provider_id) {
        slot.supports_websockets = false;
    }
```

Keep the custom-provider assignment at line 162; the new lines make official OpenAI match it. Do not change `requires_openai_auth` on the official path.

Existing test adjustments:
- `chat_profile_is_read_only_and_compacts_at_seventy_percent` (`codex_inject.rs:520`): add `assert!(!config.model_provider.supports_websockets);`
- `custom_provider_keeps_the_key_on_the_bearer` (line 544): already asserts `supports_websockets` is false; unchanged.

New tests:
- `official_openai_disables_websockets`: `legacy("openai", "chat")` with empty `base_url` → `model_provider_id == OPENAI_PROVIDER_ID` and both `config.model_provider` and `config.model_providers[id]` have `supports_websockets == false`.

---

Issue 5 [P2]: Keep sync `listen` off the UI-thread tokio context

File: `sdk/mobile/rust_agent/src/api/session.rs` — `listen` (lines 548-580)

Secondary: `start_prompt` / `complete_tool` / `respond_permission` `rt().spawn` (lines 393, 441, 506)

Problem: `listen` is `#[frb(sync)]` and runs on the Dart UI isolate via `executeSync`. Line 550 does `rt().enter()`, installing the app `Runtime` as thread-local on that isolate for the duration of the call. `Runtime::spawn` (line 558) does not need `enter()`. A leftover UI-thread tokio context is the only sync-FFI behavior on this path that can interact badly with later `blocking_read` on the FRB opaque lock (`flutter_rust_bridge` `lockable_decode_sync_ref`). `prompt`/`session_open` are `wrap_async` on FRB's separate `SimpleAsyncRuntime`; `start_prompt` then `rt().spawn`s the real work onto a second runtime.

Approach — drop `enter()` on sync FFI; spawn prompt work on the current runtime when one exists:

Do not change `listen` to async. FRB stream methods that return `Stream<T>` stay sync so Dart keeps `Stream<AgentUiEvent> listen()`.

```rust
    #[flutter_rust_bridge::frb(sync)]
    pub fn listen(&self, sink: StreamSink<AgentUiEvent>) -> Result<(), String> {
        let replay = self
            .inner
            .replay
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let mut rx = self.inner.events.subscribe();
        rt().spawn(async move {
            // existing replay + recv loop
        });
        Ok(())
    }
```

For `start_prompt` (and the same pattern in complete_tool / respond_permission):

```rust
        spawn_on_current(async move {
            let _gate = inner.complete_gate.lock().await;
            // existing body
        });
        recv_op_id(op_rx, "prompt start timeout").await
```

```rust
fn spawn_on_current<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(fut);
    } else {
        rt().spawn(fut);
    }
}
```

When called from FRB `wrap_async`, work stays on the FRB runtime (same as `session_open`). Tests that `rt().block_on(session.prompt(..))` still spawn on `rt()`. `listen` from Dart still uses `rt().spawn` because the UI isolate has no Handle.

Existing test adjustments: none expected; scripted session tests already `rt().block_on`.

New tests:
- `listen_does_not_require_current_handle`: call `session.listen(sink)` from a plain `std::thread` (no tokio context); it must return `Ok(())`. Guards removal of `enter()`.

---

Issue 6 [P2]: Build embed Config through ConfigBuilder, not a sample literal

File: `crates/kim-agent-host/src/codex_rt.rs` — `embed_config` (lines 204-367)

Secondary:
- `crates/kim-agent-host/Cargo.toml` (codex feature deps)
- `crates/kim-agent-host/src/codex_inject.rs` `apply_profile` / `config_for` tests (lines 492-507)

Problem: `embed_config` copies `thread-manager-sample`'s `Config { ... }`. Pin `core-api` (`third_party/codex/codex-rs/core-api/src/lib.rs`) re-exports `Config` but not `ConfigBuilder` / `ConfigOverrides`. Leftover sample values that `apply_profile` does not overwrite: `agents_enabled: true` (line 296), `agent_max_threads: Some(6)` (297), `cli_auth_credentials_store_mode: File` (285), `mcp_oauth_credentials_store_mode: File` (288), `file_opener: VsCode` (310), `include_environment_context: false` (258), `tui_whimsy` / `animations` (262-263). File credential mode can touch `codex_home/auth.json` or the macOS keyring from a Tokio worker. `ConfigBuilder` lives at `codex_core::config` lines 1382-1440 and is the documented inject surface (`docs/impl/codex-embed.md` CX-KD 2).

Approach — add an optional `codex-core` path dep and load through the builder with a sealed loader:

Do not patch `third_party/codex/codex-rs/core-api`. `kim-agent-host` already path-depends on `codex-config` / `codex-protocol` beyond the facade.

`Cargo.toml` `codex` feature: add `dep:codex-core`.

```toml
codex-core = { path = "../../third_party/codex/codex-rs/core", optional = true }
```

```rust
pub(crate) async fn embed_config(opts: &CodexEmbedOpts) -> Result<Config, HostError> {
    let cwd = AbsolutePathBuf::from_absolute_path_checked(&opts.cwd)
        .map_err(|err| HostError::Failed(format!("cwd: {err}")))?;
    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(opts.codex_home.clone())
        .harness_overrides(codex_core::config::ConfigOverrides {
            cwd: Some(opts.cwd.clone()),
            workspace_roots: Some(vec![cwd]),
            model: (!opts.model.is_empty()).then(|| opts.model.clone()),
            codex_self_exe: opts.codex_self_exe.clone(),
            codex_linux_sandbox_exe: opts.codex_linux_sandbox_exe.clone(),
            ephemeral: Some(true),
            ..codex_core::config::ConfigOverrides::default()
        })
        .loader_overrides(codex_config::LoaderOverrides {
            ignore_user_config: true,
            ignore_project_config: true,
            ignore_login_requirements: true,
            ..codex_config::LoaderOverrides::default()
        })
        .build()
        .await
        .map_err(|err| HostError::Failed(format!("codex config: {err}")))?;
    config.cli_auth_credentials_store_mode = AuthCredentialsStoreMode::Ephemeral;
    config.mcp_oauth_credentials_store_mode = OAuthCredentialsStoreMode::File; // unused; keys stay in memory
    config.agents_enabled = false;
    config.agent_max_threads = Some(0);
    config.check_for_update_on_startup = false;
    config.analytics_enabled = Some(false);
    config.include_environment_context = true;
    config.include_permissions_instructions = true;
    Ok(config)
}
```

`apply_profile` still overlays model window / compact / permissions / developer_instructions / MCP / skills / provider. It already sets `ephemeral = false` (line 93) and `analytics_enabled = Some(false)` (94). After Issue 4 it also clears websockets.

`ConfigOverrides` has no MCP / skills / context-window fields; those stay in `apply_profile`.

Existing test adjustments:
- `codex_inject.rs` `config_for` (line 492): make it `async` and `embed_config(&opts).await`, convert the `#[test]` functions that call it to `#[tokio::test]`.
- Any other sync `embed_config` call site.

New tests:
- `embed_config_does_not_read_user_codex_home`: set a temp `$HOME` with a `~/.codex/config.toml` that sets `model = "should-not-load"`; built config model is the harness override or default, never `should-not-load`.
- `embed_config_disables_subagents_and_file_auth`: assert `agents_enabled == false`, `cli_auth_credentials_store_mode == Ephemeral`, `check_for_update_on_startup == false`.
- `embed_config_creates_only_app_codex_home`: after build, `opts.codex_home` exists and `home.path().join(".codex")` does not (same invariant as `codex_rt.rs` lines 463-464).

---

Issue 7 [P2]: Keep one CodexThread per dest×profile

File: `sdk/mobile/lib/bridge/agent_bridge.dart` — `_promptGoose` (lines 213-317), `driveSession` (lines 321-395)

Secondary:
- `crates/kim-agent-host/src/codex_drive.rs` transcript stitch (lines 225-236)
- `sdk/mobile/rust_agent/src/api/session.rs` `reconfigure` (lines 724-753)

Problem: Every IM message opens a new `AgentSession` (`_promptGoose` line 287) and `StartThreadOptions::new` (`codex_rt.rs:170`). History is a plaintext `.codex-transcript` prepend (`codex_drive.rs:232-234`). Native tool history, roles, and compaction are dropped. `reconfigure` fast-path only updates Goose steer (line 736) and never `attach_codex`. Codex `StartThreadOptions` already has `initial_history` and `dynamic_tools` (`thread_manager.rs:231-238`); `resume_thread_with_history` rebuilds options without `dynamic_tools`, so the durable path is “hold the same `CodexThread`”.

Approach — cache the Dart session for dest×profile; park the FRB listener without aborting Codex:

`AgentRunLoop` gains `final _sessions = <String, AgentSessionPort>{}` keyed by `${req.dest}:${req.profileId}` (already the `sessionId` at line 309).

```dart
    final key = '${req.dest}:${req.profileId}';
    final cached = _sessions[key];
    final session = cached ??
        await goose.open(/* existing SessionOpenOpts */);
    _sessions[key] = session;
    return driveSession(
      session,
      dest: req.dest,
      text: req.text,
      persist: true,
    );
```

`driveSession(..., {bool persist = false})`: when `persist` is true, `finally` calls a new `session.park()` instead of `close()`. `park` is a thin FFI that sends a non-terminal wake event so FRB `cancel()` can complete (Issue 1 probe) without `abort` / `shutdown_codex`.

```rust
    pub fn park(&self) -> Result<(), String> {
        let _ = self.inner.events.send(AgentUiEvent::listener_released());
        Ok(())
    }
```

Use a new `kind: 'listener_released'` that `driveSession`'s event loop ignores (not quiet, not failed). FRB: add `park` as `#[frb(sync)]` next to `listen` (same “post and return” shape). Regenerating: `cd sdk/mobile && flutter_rust_bridge_codegen generate --config flutter_rust_bridge.agent.yaml`.

Stop prepending `.codex-transcript` in `codex_prompt` once `persist` is on; the live thread already has the rollout. Keep the file as an optional debug dump, not as model input.

`reconfigure` slow-path must call `attach_codex` when the new profile runtime is Codex. Fast-path may stay Goose-only when `!host.is_codex()` and provider/model are unchanged.

On `AgentRunLoop.stop` / profile delete, `close()` every cached session.

Existing test adjustments:
- All `driveSession` tests pass `persist: false` (default) so they still `close()`.
- `NativeAgentSession` in `goose_bridge.dart` implements `park`.

New tests:
- `driveSession persist true calls park not close`: fake session records `park` vs `close`; completed turn → `park == 1`, `close == 0`.
- `AgentRunLoop` second turn same dest/profile reuses the session: fake `AgentBridge.open` counts calls == 1.
- `codex_prompt` with live thread and empty transcript file sends only `text`, not the `Earlier turns` prefix. Rust unit test with a `CodexSlot` that has `live` skipped if we cannot mock `CodexThread`; otherwise assert `assemble_user_body` omits transcript when `reuse_thread` is set on the slot.

---

Verification

- `cd sdk/mobile && dart --packages=.dart_tool/package_config.json ../../research/agent-frb-stream-cancel.dart` — close-before-cancel still 0ms; current-order still needs a wake if someone regresses the probe
- `cd sdk/mobile && flutter test test/agent/drive_session_stop_reason_test.dart test/agent/kim_im_tools_test.dart test/agent/agent_permission_test.dart` — existing + new Dart tests
- `cargo test -p kim-agent-host --features codex` — inject / embed_config / startup_budget / shutdown
- `cd sdk/mobile/rust_agent && cargo test` — listen without Handle, scripted Goose sessions
- `cargo clippy -p kim-agent-host --features codex -- -D warnings`
- `cd sdk/mobile && flutter analyze --fatal-infos --fatal-warnings`
- If Issue 7 adds `park`: regenerate FRB agent bindings and `git diff --check sdk/mobile/lib/src/rust_agent sdk/mobile/rust_agent/src/frb_generated.rs`
- Manual: Goose turn shows the final reply and typing stops; Codex first send either replies or fails with `codex startup timed out` / `cancelled` instead of wedging the window. Capture `sample` only if a window still freezes.

Implement in issue order. Issue 1 unblocks Goose without Codex. Issues 2–4 are the Codex hang. Issue 6 can land with Issue 2 (`embed_config` async) or immediately after. Issue 7 depends on Issue 1's teardown contract (`park` vs `close`).
