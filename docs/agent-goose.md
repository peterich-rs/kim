# Local Goose agent in KIM

Goose runs on the **desktop IM** (macOS / Windows / Linux). iOS and Android
are IM clients only: they do not compile or ship `kim_agent_ffi`. Mobile and
Web can view and send in a registered 1:1; only desktop runs Goose.

KIM does not ship a custom agent loop. Desktop IM talks to a **local Goose host**:

- Loop: [`goose-agent`](https://crates.io/crates/goose-agent) state machine, assembled per `AgentProfile` by `MachineFactory`
- Providers: [`goose-providers`](https://crates.io/crates/goose-providers) (OpenAI Completions/Responses, Anthropic Messages)
- Host: `crates/kim-agent-host` (profile, vendor catalog, session map, @mention helper, system prompt)
- Catalog: `catalog/vendors.json` + `catalog_surface` 投影。UI switches on `ReasoningSurface.kind`, not vendor id. Catalog URLs feed `build_openai` / `build_anthropic`.
- FFI: `crates/kim-agent-ffi` (`kim_agent_ffi`) — still isolated from `kim_client_ffi`
- Dest: a stored row with `id=goose` still uses local dest `goose` (alias of `agent:goose`). Other personas use `agent:<profile_id>`. After `chat.bot.create`, the IM dest is the server-assigned `serverAccount` (`b_…`). New installs have an empty agent list; goose is not synthesized.

Three layers: **Agent** (`AgentProfile`: persona + model + `ReasoningChoice`), **ProviderAccount** (vendor + URL + key), **ModelChoice** (model id + reasoning). Disk profiles do not persist `provider`; `session_open` injects it.

The client flag `agent.server_identity` defaults **on** for desktop (`agentHostSupported`). The list UI hides that switch; creating an Agent still `chat.bot.create`s. When the hidden flag is off, 1:1 stays
on-device (`dest=goose` / `agent:<id>`, `_appendLocal`, no WGateway). When on
and logged in, registered 1:1 user↔agent traffic goes through WGateway
(`chat.user.talk` / `chat.bot.reply`). Group `@mention` is not implemented.
`agent.multi_profile` is a hidden kill-switch that also defaults on for desktop.

## Harness

`AgentProfile.runtime` 选底座。产品层和 Dart 不引用 goose 或 codex 的 crate。

| runtime | 底座 | 适合 |
| --- | --- | --- |
| `goose` | `goose-agent` 状态机。代码量小，协议面宽 | 产品、调研、文档这类轻 Agent |
| `codex` | `codex_core_api::ThreadManager`。OpenAI 维护，压缩、工具、MCP、审批已在库里 | 编码和长上下文 |

嵌入步骤在 [impl/codex-embed.md](impl/codex-embed.md)。两条路径共用 `AgentRuntime::run_turn`。

## Product

1. Open **我 → Agent**. List / create / edit personas at `/agent`, `/agent/new`, `/agent/:id`; provider keys at `/agent/accounts`. `/agent/settings` redirects to `/agent`.
2. Create is a short form: name, Provider (inline-create if none), that account's models, `catalog_surface` reasoning, system prompt. Fetch models lives on the Provider sheet. Custom OpenAI-compatible needs https (loopback http allowed).
3. An Agent is a **1:1 contact** in 通讯录 / 消息 after `chat.bot.create`. New / duplicate profiles register immediately when the identity flag is on; an existing empty `serverAccount` registers on first 1:1 open. Login / `ConnStatus.online` does **not** batch-ensure. Any persona (including goose) can be deleted: `botDelete` then drop the local row.
4. Tap the contact and chat like a friend. Registered 1:1 messages are stored on the server so phone / web can read and send. Goose still runs only on this desktop; the owner session posts `chat.bot.reply`.
5. Human 1:1 composer does **not** intercept `@助手`. Talk to an Agent by opening its 1:1. Mentions stay available for a later group-@ path.

## Workspace and Skills

Each persona has a **Workspace** (`sandbox` | `repo`) that becomes Goose `project_root`. Sandbox lives under Application Support `agent/workspaces/<id>/` with seed `AGENTS.md` / `MEMORY.md` / `notes/`. Repo is a user-picked absolute folder (macOS security-scoped bookmark on Release).

**Skills** are two classes (see glossary):

- **portable** — discovered under real `~/.agents/skills` and `<cwd>/.agents/skills`; muted via `portable_denylist`. Never copied into the app.
- **app / `kim-*`** — assigned in `/agent/:id/skills` or the plaza KIM shelf; resolved by `SkillResolver` (bundled → cache). Bodies inject only via `activate_skill`.

Global plaza: `/agent/plaza` (ecosystem shelf + KIM shelf). Import writes into real `~/.agents/skills`, not Application Support.

Registered 1:1 busy state reuses `typingProvider` / `KimTypingRow`. kim-sdk persists `chat.bot.reply` into the local timeline (same `message_id` as the server) so the posting desktop does not depend on Push echo. It also sends `chat.bot.typing` (heartbeat while Running) so other owner devices see `TypingPush.typer=bot`. Owner typing in a `b_…` thread uses `chat.typing` and fans out to the owner's other devices (bot has no Location).

## Layout

```
Flutter UI  --intent / watch projection-->  KimUiHandle
Desktop Rust HostAgentRuntime
  consumes MobileAgent turns in-process
  kim-agent-host session + IM tools on KimSdk
  permission cards are the only Dart round-trip
  assistant_finished → chat.bot.reply inside the runtime
Phone stays NoopAgent (no agent host linked)
```

Desktop Rust orchestrates IM and the agent host. Dart is a UI subscriber: handles, intents, and snapshots. Do not merge the `kim-agent-host` crate into `kim-client`. `kim_agent_ffi` must not depend on `kim-client`. The orchestrator is `kim-desktop-runtime`. See [ffi-oo-contract.md](ffi-oo-contract.md).

## Build / test

```bash
cargo test -p kim-agent-host
cargo test -p kim-protocol --lib
cd sdk/mobile && flutter test test/agent_mention_test.dart test/agent_settings_test.dart test/agent/catalog_test.dart test/agent/reasoning_controls_test.dart test/state/chat_agent_test.dart test/agent_paths_test.dart
```
