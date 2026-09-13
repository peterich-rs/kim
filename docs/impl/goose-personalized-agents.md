# Assemble Personalized Goose Agents inside KIM IM

| Field | Value |
|---|---|
| Author | KIM Agent Working Group |
| Date | 2026-09-11 |
| Status | Draft |
| Audience | Senior engineers implementing the Goose × KIM integration |
| Goose crates verified | `goose-agent` / `goose-provider-types` / `goose-providers` **0.1.0-alpha.8** (KIM pin) and **0.1.0-alpha.9** (crates.io, fetched for this design) |
| Unpublished Goose | `block/goose` `crates/goose` pipeline is **copy-from reference only**. KIM must not git-depend on it. |

---

## Overview

KIM 今天把 Goose 当成一个全局、无工具、单 provider 的本地对话循环：Flutter composer 在 `dest=goose` 或 `@助手` 时走 `kim_agent_ffi` → `kim-agent-host` → `StateMachine[SystemPromptOp, InferenceRunner]`，OpenAI/Anthropic 二选一。IM 能力（通讯录、历史、发消息）完全在另一条 FFI（`kim_client_ffi`）上，两条 Rust 客户端按 `docs/agent-goose.md` 硬隔离。

本设计把 host 从「一个 Inner、一个 HashMap session、两步机器」升级为 **AgentProfile → MachineFactory → Vec&lt;Step&gt;**：每个 persona 有自己的模型、系统提示、工具集、权限策略和 IM 身份（`KimPerson(kind: bot)`，`dest = agent:<profile_id>`，`goose` 作为默认 profile 的别名）。进程内工具（fs / bash / MCP）由 Rust `ToolOperation` 执行；KIM 世界工具（`send_message` / `search_contacts` / `search_messages` / `get_conversation_context` / `read_clipboard` / `list_profiles`）由 **Dart `KimCapabilityHost` 在 `SessionPhase::Yielded` 之后执行**，再由 **ChatAgent**（唯一持有 `AgentSession` 的调用方）调 `session.complete_tool(call_id, output)` 写回 `ToolResponse` 并重新 `StateMachine::run`。两条 FFI 永不合并。PR1 只引入类型与工厂；`AgentHost::prompt` 仍返回 `Result<String, HostError>`，`assistant_finished` 仍由 FFI 在 `host.prompt` 返回后合成。聊天路径以 `chat_agent_test`「一条回复气泡」为等价标准。

---

## Feasibility Assessment

对照当前代码，结论先写在前面，依据写在后面。

**结论：Feasible with caveats。** 个性化 Agent + KIM IM 工具在已发布 Goose crates 上可以落地；不能承诺的是 OS sandbox、未发布的 Goose 全量 pipeline（Steer / Doctor / Recipe / Skill）、以及把 `sqlite_path` 立刻变成真正的 SQLite 会话库。

已验证、可直接依赖的表面：

1. **状态机可按 profile 组装。** `goose_agent::machine::StateMachine::new(Vec<Step>, CancellationToken)`（`goose-agent-0.1.0-alpha.8/src/machine.rs:72`）接受任意 `Step::Operation` / `Step::Inference`。KIM 当前写死两步（`crates/kim-agent-host/src/lib.rs:265-269`）。`MachineFactory` 只是把这段 `vec![]` 参数化，不改 crate 边界。
2. **工具环路已在已发布 crate 里。** `ToolOperation<S>` + `ToolProvider<S>`（`goose-agent-.../src/tool.rs:96-159`）在 inference 前收集 `rmcp::model::Tool`，在 `run` 里对未回答且 `!was_executed_externally()` 的 `ToolRequest` 调 `provider.call`。KIM 的 fs/bash/MCP 走这条路。集成测试范本在 `goose-agent-.../tests/tool_operation.rs`（`with_sync_tool` / `with_async_tool` / `ToolProvider`）。
3. **让出客户端是一等语义。** `operation::yielded()` / `yielded_with()` 把 `StepResult.yield_to_client = true`（`operation.rs:180-194`）；`StateMachine::run` 在 apply 后 `break`（`machine.rs:168-170`）。下一轮 LLM 的真正闸门是 `InferenceRunner::applies` → `ends_with_provider_turn`（最后一条 **effective role** 为 User 或 Tool，`inference.rs:231-238, 341-347`）。bare `yielded()` 后 conversation 末尾仍是 assistant `ToolRequest`，`applies` 为 false，不会误触发 LLM。`ends_turn`（`operation.rs:60-70`）存在于 crate 但 **goose-agent 无调用点**，KIM 不要用它当闸门。续跑是一次全新的 `StateMachine::run`（从头找第一个 Applicable），不是旧 iterator 的 resume。
4. **权限数据类型已在 provider-types。** `GooseMode { Auto, Approve, SmartApprove, Chat }`（`goose-provider-types-.../src/goose_mode.rs`）、`Permission { AlwaysAllow, AllowOnce, Cancel, DenyOnce, AlwaysDeny }`（`permission.rs`）、`ActionRequiredData::ToolConfirmation` / `ToolConfirmationResponse`（`conversation/message.rs:196-218`）。**没有**已发布的 `PermissionOperation`——KIM 必须自实现，抄 unpublished Goose 的 ToolApproval 模式。
5. **Provider 表面够用。** `Provider::stream` / `fetch_supported_models` / `fetch_supported_model_info`（`base.rs:464-522`）；默认 `fetch_supported_models` 是 `Ok(vec![])`（`base.rs:511-512`），OpenAI/Anthropic 覆盖。空列表必须显式回退到 known-models，不能当「无模型」。`ModelConfig::new` + `with_thinking_effort(ThinkingEffort)` + `with_temperature`（`model.rs:103, 167-169, 220-227`）；`OpenAiProviderBuilder` / `AnthropicProviderBuilder`（KIM 已用，`kim-agent-host/src/lib.rs:363-395`）；`declarative::from_json(json, tls, key_resolver)`（`goose-providers-.../src/declarative.rs:348-365`），bundled JSON **46** 个（`src/declarative/definitions/*.json`）。`from_json` 在 `KeyResolver` **之前** 用 `std::env::var` 展开 `base_url` 的 `${ENV}`（`declarative.rs:266-307`）；移动端只启用 **字面 base_url、无 required `env_vars`** 的 JSON，不声称 46 家都能在 iOS 上工作。`AuthMethod::{BearerToken, ApiKey}`、`TlsConfig` 已在 `api_client.rs`。`Provider::get_context_limit` 是 **async** `(model, override) -> usize`（`base.rs:501-505`），CompactionOp 必须 `.await`，不是同步字段。
6. **FFI 事件槽位已预留。** `AgentUiEvent` 是扁平 string-kind（`sdk/mobile/rust_agent/src/api/session.rs:39-53`），字段 `call_id` / `name` / `arguments_json` / `output_preview` / `input_tokens` / `output_tokens` 目前恒为空。`sdk/mobile/lib/src/rust_agent/api/session.dart:10` 的 `tool_started` / `usage` 列表是 FRB **忽略未导出符号** 的生成注释，**不是** 保留 kind 合同；本设计自行规定 kind 字符串。扩展 kind，不引入 freezed enum。
7. **IM 工具的执行面已存在于另一条 FFI。** `KimClient::send_message`（`crates/kim-client/src/client.rs:173`）、`history`（同文件 `:275`）；Dart `KimClientPort.sendMessage` / `friendList` / `searchUsers` / `history`（`sdk/mobile/lib/kim_bridge.dart:53-77`）；本地缓存 `ConversationStore.loadMessages`（`sdk/mobile/lib/data/conversation_store.dart:344`）。Dart 编排两条 FFI 是既定架构（`docs/agent-goose.md:24`）。
8. **本地 bot 身份已有产品槽。** `kGooseAgentPerson`（`ProfileKind.bot`，`sdk/mobile/lib/agent/mention.dart:9-13`）注入通讯录、会话列表、搜索；`OutboxNotifier._assertCanQueue` 拒绝 `dest=goose` 出站（`sdk/mobile/lib/state/outbox.dart:330-332`）；`ChatAgent._appendLocal` 只写 `ConversationStore`，不碰 WGateway。

Caveats（必须在实现中显式处理，不是阻塞）：

| Caveat | 证据 | 处理 |
|---|---|---|
| `enable_fs_tools` / `bash_enabled` 未接线 | FFI 有字段（`session.rs:20-21`）；设置页写死 `false`（`agent_settings_page.dart:79-80`）；host 完全忽略 | PR3 才接线；默认保持 false |
| `sqlite_path` 被当成 `session_id` 字符串 | `session.rs:153-157`；`ChatAgent._ensureSession` 传入 `dest`（`chat_agent.dart:86`） | **PR7** 才做真正文件持久化；此前文档化 in-memory HashMap |
| `project_root` 被丢掉 | `session_open(..., _project_root, ...)`（`session.rs:149`） | PR3 fs/bash 起用 `KimPaths.agentWorkspace` |
| `resume` / `resume_on_open` 空操作 | `AgentSession::resume` 返回空 vec（`session.rs:272-277`） | PR7 前进程内 HashMap 即「resume」 |
| 系统提示声称「host pasted conversation」但没有 paste | `DEFAULT_SYSTEM_PROMPT`（`lib.rs:32-36`）；`ChatAgent._prompt` 只送当前句（`chat_agent.dart:68`） | PR4 `get_conversation_context` + prompt 可选 `context_json`（`user_visible=false`） |
| 无 scripted provider，`session_scripted.rs` 用空 key 会 `MissingApiKey` | `ProviderKind::parse("scripted")` 映射到 OpenAi（`lib.rs:70`）；`AgentHost::new` 拒空 key（`lib.rs:354-356`）；测试未 `#[ignore]` | PR0 **重写测试**：dummy key + `snapshot()` + `close()`，不 `prompt`、不断言 sqlite 文件。`ScriptedProvider` 在 PR1（host 单测）/ PR3（FFI yield 测试） |
| 单 `ChatAgent` 同时只持一个 `AgentSession` | `chat_agent.dart:24-26` | PR8 改成 per-dest LRU；此前 dest 切换会 close 旧 session |
| 已发布 goose-agent **没有** Permission / Compaction / Subagent / MCP Operation | `goose-agent-.../src/lib.rs` 只有 `events, inference, machine, operation, tool` | KIM 自实现；MCP 用 `ToolProvider` 包 `rmcp` client |
| Goose 无通用 OS sandbox | 已发布 crates 无 seatbelt/docker API | 早期用 permission list + readonly fs；Phase 5 可选，不承诺 |
| alpha.9 未进 Cargo.lock | host pin `0.1.0-alpha.8`（`crates/kim-agent-host/Cargo.toml:13-15`） | PR0 bump；alpha.8→9 的 goose-agent **源码树 diff 为空**（仅 version / `rmcp 3.0.0→3.2.0` / types 版本） |
| 新 FFI 方法必须跑 FRB codegen | `flutter_rust_bridge.agent.yaml`；hook 编 `rust_agent`（`sdk/mobile/hook/build.dart:9-12`） | `complete_tool` / `respond_permission` / `fetch_supported_models` 所在 PR 提交生成文件 |

**Fully feasible** 的子集：PR0–PR2（升级、Profile 类型、设置 UI、declarative 列表、`fetch_supported_models`、reasoning）在现有两步机器上即可，聊天行为可用 flag / 默认 profile 保持不变。

**Feasible with caveats** 的全集：PR3 起的工具、权限卡片、Dart capability host、多 Agent 通讯录。阻塞项不在架构，而在「自实现 Operation + 两条 FFI 的事件协议」工作量，以及 fs/bash 的产品风险控制。

---

## Background & Motivation

当前产品路径（`docs/agent-goose.md`）：

1. 「我 → Agent 设置」填 OpenAI 或 Anthropic key（`sdk/mobile/lib/screens/home/me_page.dart:137-140` → `/agent/settings`）。
2. 通讯录「本地 Agent」一节只有一个 `助手`（`contacts_page.dart:209-217`）。
3. 点开当好友聊；消息走 `ChatAgent._appendLocal`，`Outbox` 拒绝 `dest=goose`。
4. 真人会话里 `@助手` / `@goose` 拉起同一个全局 host。

痛点：

- **一个全局 `AgentSettings`**（`sdk/mobile/lib/state/agent_settings.dart`），无法「翻译 Agent」与「编码 Agent」并存。
- **机器不可配置**：所有 session 都是 SystemPrompt + Inference，无工具、无审批、无 compaction。
- **HostEvent 只有 TextDelta/Finished/Failed**（`lib.rs:99-103`），且 host 实际只 emit TextDelta；`Finished` 由 FFI 在 `prompt` 返回后补发（`session.rs:222-224`）。`ChatAgent` 只处理 `assistant_finished` 与 `failed`（`chat_agent.dart:92-97`），流式 delta 被丢弃。
- **Goose 已发布能力闲置**：46 家 declarative provider、`ThinkingEffort`、`GooseMode`、`ToolOperation`、`ActionRequired`、`fetch_supported_models`。
- **KIM 自己的 IM 能力 Agent 用不上**：发消息、搜好友、搜本地历史都在 Dart/`kim_client_ffi`，host 看不见。

目标用户体验：通讯录出现多个本地 bot；每个 bot 能力组合不同；`@译者` 只翻译不代发；`助手` 可以搜聊天记录并在确认后代发；`coder` 可以读工作区文件。全部在本机跑，真人会话仍走 WGateway。

---

## Goals & Non-Goals

### Goals

- 每个 Agent 一份 `AgentProfile`：provider / model / reasoning / system prompt / `GooseMode` / 工具集 / 权限 / 沙箱策略（早期为 permission 列表）。
- `MachineFactory` 按 profile 组装 `Vec<Step>`，chat-only 与 tools+approval 与 MCP 是不同机器。
- KIM 原生能力以工具形式暴露，**Dart 执行 IM 工具**，Rust 执行进程内工具。
- 多 profile 在 IM 中表现为 `kind: bot` 的 `KimPerson`，`dest=agent:<id>`，`goose` 别名兼容。
- `@助手` / `@goose` / `@<display_name>` 在真人会话拉起对应本地 Agent；该会话的人类消息仍走 IM。
- 权限：`ActionRequired::ToolConfirmation` → 本地气泡确认卡 → `respond_permission`。
- API key 只在 Flutter 安全存储；profile JSON 不落明文 key。
- PR1 聊天路径与今天等价（测试：`chat_agent_test` 一条回复气泡；`assistant_finished` 仍由 FFI 合成）。默认 profile = 当前 `AgentSettings`。
- 所有生产路径 rust-strict：无 `unwrap`/`expect`；错误类型化；日志永不打印 key。

### Non-Goals

- 合并 `kim_agent_ffi` 与 `kim_client_ffi` / `kim-client`。
- git 依赖 unpublished `block/goose`（含 `crates/goose` 的 EntryHook…ExitOnError 全管道）。
- 把本地 Agent 消息发到 WGateway。
- 早期 PR 承诺 seatbelt / Docker / 通用 OS sandbox。
- 用户导入任意 declarative JSON（晚于 bundled providers）。
- 云端多租户 Agent、服务端代跑 Goose。
- 把 `AgentUiEvent` 改成 freezed / Rust enum 桥接。
- 桌面/Web 独立 agent 壳（本设计范围是 `sdk/mobile` + `kim-agent-host`）。
- 图像/语音作为 Agent 输入（现有 `KimComposer` 对 agent chat 仍可发图，但 host 只吃 text；保持现状直到另开 RFC）。
- 把 Agent 回复 fan-out 到群其它成员。群/真人线程里 `@mention` 的 Agent 回复保持 **本机 `_appendLocal`**，其它设备看不到。
- 在同一线程里同时跑两个 Agent 的交错工具环（每个 `(threadDest, profileId)` 独立 session；UI 串行）。
- Profile JSON 落到磁盘文件（`agent/profiles.json` 不存在）。权威存储是 SharedPreferences `agent.profiles`。

---

## Current Surface Inventory

本计划会碰到的 API / 调用点。未列出的 crate（`services/*`、`crates/kim-client` 生产路径、`sdk/mobile/rust` IM FFI）**保持不变**。

### Rust host — `crates/kim-agent-host`

| 符号 | 路径 | 用途 |
|---|---|---|
| `DEFAULT_AGENT_ID` / `DEFAULT_AGENT_NAME` / `DEFAULT_SYSTEM_PROMPT` | `crates/kim-agent-host/src/lib.rs:30-36` | 内置「助手」身份与提示 |
| `HostError` | `lib.rs:41-53` | `MissingApiKey` / `UnknownProvider` / `InvalidUrl` / `Busy` / `Failed` |
| `ProviderKind::{OpenAi, Anthropic}` + `parse` | `lib.rs:62-77` | 含 `scripted`→OpenAi 别名 |
| `ProviderConfig` / `openai()` | `lib.rs:80-96` | 当前唯一 provider 输入 |
| `HostEvent::{TextDelta, Finished, Failed}` | `lib.rs:99-103` | host→FFI 事件 |
| `HostSession` + `MachineSession` | `lib.rs:106-119` | 仅 `id` + `Conversation` |
| `HostEffect::{Append, Usage}` | `lib.rs:122-147` | `InferenceEffect` + `MachineEffect` |
| `SystemPromptOp` | `lib.rs:149-166` | `prompt_parts` 注入 system |
| `Store` | `lib.rs:168-171` | `HashMap<String, Conversation>` + busy |
| `AgentHost::new` | `lib.rs:186-200` | 单 provider / 单 model / 固定 prompt |
| `AgentHost::prompt` | `lib.rs:202-235` | 追加 user 消息、跑机器、清 busy |
| `AgentHost::run_loop` | `lib.rs:239-284` | 组装两步机器；pump 只转发 `AgentEvent::Message` 文本 |
| `SessionLoader` / `EffectHandler` | `lib.rs:288-324` | 按 session_id load/append |
| `model_config` | `lib.rs:344-351` | 仅 `ModelConfig::new(name)`，空/`scripted`→`gpt-4o` |
| `build_provider` / `build_openai` / `build_anthropic` | `lib.rs:353-395` | `ApiClient::with_timeout_and_tls` + builder |
| `mentions_default_agent` | `lib.rs:431-436` | Rust 侧 @ 检测（Dart 另有一份） |
| crate deps | `crates/kim-agent-host/Cargo.toml:13-15` | goose `0.1.0-alpha.8` |

### Goose published crates（实现时对照源码，勿对照二手笔记）

| 符号 | 路径 | 用途 |
|---|---|---|
| `StateMachine::new/step/apply/run` | `goose-agent-0.1.0-alpha.8/src/machine.rs:72-173` | 无步可应用或 `yield_to_client` 则停 |
| `Step::{Operation, Inference}` | `machine.rs:34-37` | 机器步骤 |
| `Operation` / `Inference` / `InferenceInput` | `operation.rs:74-155` | `run` / `inference_tools` / `prompt_parts` / `moim_parts` |
| `applied` / `yielded` / `yielded_with` / `not_applicable` | `operation.rs:168-194` | 步骤结果 |
| `ConversationEffect` | `operation.rs:200-212` | Append / Replace / PatchToolRequestMeta / SetMessageVisibility |
| `Emitter` / `AgentEvent` | `operation.rs:243-270`, `events.rs:9-18` | Message / Usage / HistoryReplaced / McpNotification |
| `InferenceRunner` | `inference.rs:183-301` | `new(provider, ModelConfig)`；`applies` 看 kickoff + trailing_error |
| `ToolOperation` / `ToolProvider` | `tool.rs:96-273` | 进程内工具广告与分发 |
| `ToolRequest::was_executed_externally` | `goose-provider-types-.../src/conversation/tool_request.rs:58-64` | meta `goose.external_dispatch` |
| `TOOL_META_EXTERNAL_DISPATCH_KEY` | `conversation/message.rs:158` | `"goose.external_dispatch"` |
| `MessageContentBlock` / `ActionRequired` / `ActionRequiredData` | `conversation/message.rs:187-330` | ToolRequest/Response、确认、思考 |
| `GooseMode` | `goose_mode.rs:22-32` | Auto / Approve / SmartApprove / Chat |
| `Permission` / `PermissionConfirmation` | `permission.rs` | AlwaysAllow / AllowOnce / Cancel / DenyOnce / AlwaysDeny |
| `Provider` | `base.rs:464+` | `stream` / `complete` / `fetch_supported_models`（默认空 vec） / `fetch_model_info` / **async** `get_context_limit(model, override)` |
| `ModelConfig` / `with_thinking_effort` / `with_temperature` | `model.rs:40-60, 103-118, 167-169, 220-227` | reasoning 走 `request_params["thinking_effort"]` |
| `stream_from_single_message` | `goose_providers::base`（**不是** provider-types） | scripted `Provider::stream` 用 |
| `ThinkingEffort::{Off,Low,Medium,High,Max}` | `thinking.rs:307-313` | 设置页下拉 |
| `declarative::from_json` / `FIXED_PROVIDERS` / `fixed_provider_config_entries` | `goose-providers-.../src/declarative.rs:10, 76-78, 348` | 46 家 JSON |
| `ProviderEngine::{OpenAI,Ollama,Anthropic}` | `declarative.rs:96-103` | from_json 分发 |
| `KeyResolver` / `EnvKeyResolver` | `declarative.rs:238-264` | `SessionKeyResolver` 只覆盖 `api_key_env`；`base_url` `${ENV}` 仍走 `std::env::var`，bundled 无字面 URL 的 JSON 跳过 |
| `OpenAiCompatibleProvider` | `openai_compatible.rs:31-53` | OpenAI 兼容端点 |
| `AuthMethod` / `TlsConfig` | `api_client.rs:45-62` | 与现 host 相同 |
| `OpenAiProvider::fetch_supported_models` | `openai.rs:764-785` | `/v1/models`，可回退 custom list |

alpha.9：`goose-agent` 源码与 alpha.8 **逐文件相同**；`rmcp` 3.0.0 → 3.2.0。`goose-provider-types` 变更在 canonical JSON 与 `formats/openai.rs` / `model.rs` 内部，**KIM 使用的 pub 方法签名未变**。PR0 仍需 `cargo test -p kim-agent-host` 实编。

### Agent FFI — `sdk/mobile/rust_agent`（`kim_agent_ffi`）

| 符号 | 路径 | 用途 |
|---|---|---|
| `SessionOpenOpts` | `sdk/mobile/rust_agent/src/api/session.rs:14-22` | model / llm_backend / resume_on_open / base_url / api_key / enable_fs_tools / bash_enabled |
| `AgentUiEvent` | `session.rs:39-53` | 扁平事件；tool 字段未填充 |
| `session_open` | `session.rs:147-170` | `sqlite_path`→session_id；丢弃 `project_root`；`AgentHost::new` |
| `AgentSession::prompt` | `session.rs:173-241` | 置 busy、spawn run、返回 operation_id |
| `listen` | `session.rs:244-261` | broadcast → `StreamSink<AgentUiEvent>` |
| `abort` / `resume` / `snapshot` / `reconfigure` / `close` | `session.rs:263-300` | resume 空；reconfigure 重建 host（丢 conversation，因 host 是新的——**现有 bug/缺口**：reconfigure 换 Inner，旧 HashMap 随旧 host 丢掉） |
| `config_from_opts` | `session.rs:137-145` | 忽略 fs/bash |
| Dart 镜像 | `sdk/mobile/lib/src/rust_agent/api/session.dart` | FRB 生成，禁止手改 |
| codegen 配置 | `sdk/mobile/flutter_rust_bridge.agent.yaml` | `AgentRustLib` |
| native assets | `sdk/mobile/hook/build.dart:9-12` | 同时编 `rust` 与 `rust_agent` |
| 测试 | `sdk/mobile/rust_agent/tests/session_scripted.rs` | 空 key + `sqlite.exists()` 与实现不一致；PR0 重写为 dummy key + snapshot，不 prompt |

### Flutter agent shell

| 符号 | 路径 | 用途 |
|---|---|---|
| `AgentBridge` | `sdk/mobile/lib/agent_bridge.dart` | 隔离 `AgentRustLib.init`；`open`→`sessionOpen` |
| `ChatAgent` | `sdk/mobile/lib/state/chat_agent.dart` | 单 session；direct vs @mention；本地气泡 |
| `chatAgentProvider` | `chat_agent.dart:142-148` | 进程级单例；两个 ChatPage 对同一 dest 的 prompt 被这一份 session 串行 |
| `AgentSettings` / `toOpts` | `sdk/mobile/lib/state/agent_settings.dart:16-68` | SharedPreferences + `flutter_secure_storage` key `agent.api_key` |
| `AgentSettingsPage` | `sdk/mobile/lib/screens/agent/agent_settings_page.dart` | OpenAI/Anthropic segmented；fs/bash 硬编码 false |
| `mention.dart` | `sdk/mobile/lib/agent/mention.dart` | `kGooseAgentId="goose"`；`@助手` / `@goose` |
| `ChatSessionNotifier.sendText` | `sdk/mobile/lib/state/chat_session.dart:155-167` | agent dest→`sendDirect`；否则 outbox + `onOutgoingText` |
| `ChatSessionNotifier.isAgent` | `chat_session.dart:43` | `isGooseAgentDest` |
| `withGooseAgent` / `ContactsNotifier` | `sdk/mobile/lib/state/contacts.dart:39-47, 105, 130, 156-161` | 注入本地 bot |
| `withGooseThread` | `sdk/mobile/lib/state/inbox.dart:56-68` | 会话列表置顶助手 |
| `OutboxNotifier._assertCanQueue` | `sdk/mobile/lib/state/outbox.dart:329-332` | 禁止 goose 出站 |
| `ChatPage` | `sdk/mobile/lib/screens/chat/chat_page.dart:68, 79-83, 237-239` | agent 会话跳过好友门与 typing |
| `ContactsPage` 本地 Agent 节 | `sdk/mobile/lib/screens/home/contacts_page.dart:209-217` | 写死一个 ListTile |
| `KimPaths.agentRoot/Sessions/Workspace` | `sdk/mobile/lib/core/paths.dart:74-94` | 目录已建，host 未用 sessions |
| `KimMessageRow` | `sdk/mobile/lib/widgets/kim_bubble.dart:54-98` | `sys` 居中灰字；无 tool/确认卡 |
| l10n | `sdk/mobile/lib/l10n/app_zh.arb:148-164` | Agent 文案 |
| 路由 | `sdk/mobile/lib/router/app_router.dart:117-123` | `/agent/settings` |
| 测试 | `sdk/mobile/test/state/chat_agent_test.dart`；`test/agent_settings_test.dart`；`test/agent_mention_test.dart` | 设置加载、mention、direct prompt 会 open session |

### IM 能力（Agent 工具将调用；**不改 Rust IM crate**）

| 符号 | 路径 | 用途 |
|---|---|---|
| `KimClient::send_message` | `crates/kim-client/src/client.rs:173-193` | dest / kind / OutgoingContent / client_id |
| `KimClient::talk_to_user` | `client.rs:197-206` | 文本便捷封装 |
| `KimClient::history` | `client.rs:275-300` | dest / kind / before_id / limit |
| `KimApi::send_message` / `history` | `sdk/mobile/rust/src/api/client.rs:172-213` | IM FFI |
| `KimClientPort.sendMessage` | `sdk/mobile/lib/kim_bridge.dart:53-58` | Dart IM 端口 |
| `friendList` / `searchUsers` | `kim_bridge.dart:73-77` | 通讯录 |
| `OutboxNotifier.sendText` | `sdk/mobile/lib/state/outbox.dart:72-78` | 真正发 IM（好友校验、本地草稿、pump） |
| `MessageRepository.applyLive` / `applyOwn` | `sdk/mobile/lib/data/message_repository.dart:70-94` | 本地落库 |
| `ConversationStore.loadMessages` | `sdk/mobile/lib/data/conversation_store.dart:344-367` | 线程全量（无 FTS） |
| `Clipboard.setData` | `sdk/mobile/lib/screens/chat/chat_chrome.dart:61` | 仅复制；读取要用 `Clipboard.getData` |
| `KimPerson` / `ProfileKind.bot` | `sdk/mobile/lib/models/models.dart:6-27` | bot 身份 |
| `KimChatMsg` / `KimMsgKind` | `models.dart:281-316` | `text/image/video`；无 agentCard |

### 明确不改（审计用）

| 路径 | 原因 |
|---|---|
| `sdk/mobile/rust/**`（`kim_client_ffi`） | 与 agent FFI 隔离；Goose 仍只经 `kim_agent_ffi`，代发走 Dart → `kim_client_ffi` |
| `sdk/mobile/lib/src/rust/**` | IM FRB 生成物 |
| `crates/kim-core` 等传输层 | 无关 |

1:1 用户↔已注册 Agent 消息现在经过 WGateway（见 `docs/impl/goose-bot-first-class.md`）。群 `@mention` 仍本机。`gateway` / `router` 转发不变。

---

## Design

### 目标架构（相对现状的 delta）

当前：

```text
Flutter composer
  ├─► kim_client_ffi ──► WGateway ──► chat          (human IM)
  └─► @助手 / dest=goose
         ──► kim_agent_ffi
              ──► AgentHost
                   → HashMap session
                   → StateMachine[SystemPromptOp, Inference]
                   → OpenAI|Anthropic builder
```

目标：

```text
Flutter composer / 通讯录
  ├─► kim_client_ffi ──► WGateway ──► chat          (unchanged)
  │
  └─► dest=agent:<id> | dest=goose | @<profile>
         ──► kim_agent_ffi
              ──► AgentHost ★
                   → Profile registry (Dart persisted, resolved at open)
                   → MachineFactory(profile) ★
                   → HashMap session (until PR7 → JSON file)
                   │
                   ├─ SystemPromptOp
                   ├─ MaxTurnsOp ★
                   ├─ CompactionOp ★ (PR7)
                   ├─ PermissionOp ★ ──┤ yield ActionRequired
                   ├─ DeferredKimToolOp ★ ──┤ yield (no UI event yet)
                   ├─ ToolOperation (fs/bash/MCP) ★
                   ├─ ChatGuardOp / UnknownToolOp ★
                   └─ InferenceRunner
                          │
                          ▼
                   run() returns ★
                          │
                          ▼
                   AgentSession sets Yielded/Idle, THEN emit ★
                          ══► ChatAgent  (sole AgentSession caller)
                                ├─ local bubbles (text / tool / card)
                                └─ KimCapabilityHost.execute ★
                                      ──► Outbox.sendText / store / Clipboard
                                      │
                                      └─ ChatAgent.complete_tool / respond_permission
```

### 决策 1 — IM 工具的 Capability Bridge（必选默认）

**选择：** 双执行器。

- Rust `ToolProvider`：进程内工具（fs、bash、MCP、以及将来纯 Rust clipboard）。
- Dart `KimCapabilityHost`：KIM 世界工具（含 `list_profiles`）。Host 侧 `DeferredKimToolOp` 只 **广告 schema**；看到未回答的匹配 `ToolRequest` 时 `yielded()`（**不**在 `run()` 内发 UI 事件）。`run()` 返回 `TurnOutcome::Yielded` 之后，FFI 把 `SessionPhase` 设为 `Yielded`、清 running，**然后**从 conversation 重放 pending id，每个发一条 `tool_request` / `action_required`。ChatAgent（唯一 FFI 调用方）执行 `KimCapabilityHost.execute`，再 `session.complete_tool`。`KimCapabilityHost` **不**持有 `AgentSession`。

**拒绝：**

- 在 `kim-agent-host` 里 `path-dep kim-client`，或 `kim_agent_ffi` 调 `kim_client_ffi`。违反 `docs/agent-goose.md:24`「Dart orchestrates the two FFIs」。
- 用 oneshot channel 把 `ToolProvider::call` 阻塞到 Dart 回包。机器在 `run()` 内持有 session lock/busy，Flutter 无法在同一 session 上 `complete_tool`；也更难做确认卡与取消。
- ACP `was_executed_externally` 冒充：该 meta（`goose.external_dispatch`）表示「已经在外部执行完、loop 不要再 dispatch」。Dart 工具在 yield 时 **尚未执行**，不能打这个标记。Dart 跑完之后写的是正常 `ToolResponse`；**任何阶段都不得**设置 `goose.external_dispatch`。

**为何 yield 而不是阻塞：** `StateMachine::run` 在 `yield_to_client` 后返回（`machine.rs:168-170`）。返回后 FFI 必须进入 `Yielded`（不再 Running），否则 `complete_tool` 进不来。这是对当前 `AgentSession::prompt`「全程 busy」（`session.rs:174-235`）的关键修正。

```text
sequence  (Dart IM tool; event AFTER phase transition)

ChatAgent          AgentSession           StateMachine         KimCapabilityHost
  │                     │                      │                     │
  │ prompt(text)        │                      │                     │
  │ ──►                 │ phase=Running        │                     │
  │                     │ run()                │                     │
  │                     │ ──►                  │ Inference → ToolReq │
  │                     │                      │ DeferredKim yielded │
  │                     │ ◄── TurnOutcome::Yielded
  │                     │ phase=Yielded ★      │                     │
  │                     │ (busy/running clear) │                     │
  │  tool_request  ◄══  │ replay from conv ★   │                     │
  │ ──► execute ─────────────────────────────────────────────────────►│
  │                     │                      │                     │ Outbox.sendText
  │ complete_tool(id,out)                      │                     │
  │ ──►                 │ only if Yielded ★    │                     │
  │                     │ append ToolResponse  │                     │
  │                     │ pending empty? ★     │                     │
  │                     │  yes → Running+run() │                     │
  │                     │  no  → stay Yielded  │                     │
  │                     │ ──►                  │ Inference (if ran)  │
  │                     │ phase=Idle           │                     │
  │  assistant_finished ◄══  (FFI synthesize until PR4 host-Finished)
```

### 决策 2 — Profile → IM 身份

**选择：** 每个 **enabled** profile 是本地 `KimPerson { account: dest, nickname: display_name, kind: ProfileKind.bot }`。

- 规范 dest：`agent:<profile_id>`。
- 兼容：`goose` **永久是默认 profile `id="goose"` 的别名**。`isGooseAgentDest("goose")` 继续为 true；`isAgentDest` 同时接受 `goose` 与 `agent:` 前缀。
- `@mention`：在现有 `@goose` / `@助手` 之外，按 enabled profile 的 `display_name` 与 `id` 扩展正则。冲突时精确 id 优先。
- 真人/群线程里 mention 某个 profile：人类消息仍 `Outbox.sendText` → WGateway；Agent 回复继续 `_appendLocal` 到 **该 thread dest**，**不** fan-out 给群其它成员（本机可见）。
- Live session 键是 `(threadDest, profileId)`，不是单独 dest。同一群里 `@助手` 再 `@译者` 打开两条独立 conversation，互不 reconfigure。PR8 之前只有 goose，不会出现 mismatch。
- `_appendLocal` 的 `sender` = `profile.display_name`，`account` = canonical dest（PR8；此前只有助手，仍是 `kGooseAgentName`）。

**拒绝：** 用 nickname 当 dest（非稳定、不可当 session key）；把所有 profile 复用 `dest=goose`（无法分会话、分权限）；用 dest 单键 LRU 导致同线程换 persona 复用错误机器。

**迁移：** 现有本地会话 id 就是 `goose`。不要 rewrite 历史消息。UI 上 `goose` 与 `agent:goose` 视为同一线程：`isAgentDest` 归一到 canonical `goose`（默认 profile）以免出现两个「助手」会话。

### 决策 3 — Machine assembly

**选择：** `MachineFactory::assemble(&AgentProfile, Arc<dyn Provider>, ModelConfig, &HostPaths) -> Vec<Step<'static, HostSession, HostEffect>>`。每个 `AgentSession` 仍持有自己的 `AgentHost`（现状如此，`session_open` 已 `AgentHost::new`），工厂在每次 `run_loop` / `continue_turn` 调用，使 permission store 与 tool 开关变更无需重建 host。

**拒绝：** 一个全局机器（无法表达 translator vs coder）；把 unpublished 全管道（EntryHook…ExitOnError）一次性抄进来。KIM 只实现需要的 Operation。

推荐步骤顺序（与 Goose 参考管道对齐的最小子集；机器每轮从头找第一个 Applicable）：

1. `SystemPromptOp` — 仅 `prompt_parts`，`run` 恒 `NotApplicable`
2. `MaxTurnsOp` — `assistant_turn_count >= max` 则 append 错误消息并 `yielded_with`
3. `CompactionOp` — **PR7**；超上下文时 `ReplaceConversation`（需 `apply_effects` 实现该变体）
4. `PermissionOp` — 未确认的 ToolRequest → append `ActionRequired::ToolConfirmation` + `yielded_with`；已确认则 `NotApplicable` 或对 Deny 写错误 `ToolResponse`。状态只看 conversation 里的 `ActionRequired` / `ToolConfirmationResponse`，**不用** `set_message_meta`
5. `DeferredKimToolOp` — 未回答的 KIM 工具请求 → `yielded()`（UI 事件由 FFI 在 Yielded 后重放）
6. `ToolOperation` — 进程内工具
7. `UnknownToolOp` / `ChatGuardOp` — 仍未回答的 ToolRequest → 错误 ToolResponse。二者 **PR3** 才进 factory，不是 PR1，也不是 PR5
8. `InferenceRunner`

**Chat-only 判定：`ToolSet` 为空。** `GooseMode::Chat` 只允许出现在空 ToolSet 上（`from_legacy` 与 factory 都执行这条）。若 JSON 误带 `mode=Chat` 且 tools 非空：factory **仍安装工具步骤**（tools 赢），`tracing::warn`。不要用 `mode==Chat || no tools` 双条件让 Chat 把已打开的 fs/kim 工具短路掉。

**PR1 空 ToolSet 机器只有 2 步：** `SystemPromptOp` + `InferenceRunner`。不要在 PR1 装 ChatGuard（那会变成 3 步，破坏测试 X）。空 ToolSet 的 ChatGuard 从 **PR3** 起才加。MaxTurns 仅当 `profile.max_turns` 被 factory 读取且该 Op 已存在（PR3 起）；PR1 `from_legacy` 虽写 `Some(16)` 但 factory 忽略，直到 `MaxTurnsOp` 落地。

### 决策 4 — Goose 升级 alpha.8 → alpha.9

**选择：** PR0 单独 bump `crates/kim-agent-host/Cargo.toml` 三个 crate 到 `0.1.0-alpha.9`，并 `cargo update -p goose-agent -p goose-provider-types -p goose-providers`。`sdk/mobile/rust_agent/Cargo.lock` 随 workspace path-dep 更新。

**兼容性（已对照源码）：**

- `goose-agent` alpha.8/9 `src/` **diff 为空**。KIM 使用的 `StateMachine` / `Operation` / `ToolOperation` / `InferenceRunner` / `Emitter` 签名不变。
- `rmcp` 3.0.0 → 3.2.0（agent 的传递依赖）。**bump 的产品理由是给 PR3 `ToolOperation` 对齐 3.2**，不是 goose-agent 源码差。PR0 编译时确认 `rmcp::model::Tool` / `CallToolRequestParams` / `CallToolResult` 仍匹配。若 3.2 有 breaking，只在 host 工具代码（PR3 才写）暴露；PR0 主机尚未调用 `ToolOperation`。
- `goose-provider-types`：canonical 模型表与 openai format 内部变化；`ModelConfig::new` / `with_thinking_effort` pub 签名未变。
- `goose-providers`：`openai.rs` / `databricks_v2.rs` / `snowflake.rs` 内部；KIM 的 `OpenAiProviderBuilder::new(client).base_path(...).supports_streaming(true).build()` 与 Anthropic builder 在 alpha.8 的 pub 方法，alpha.9 未删。

**拒绝：** 跳到 unpublished git；或把 bump 混进行为 PR。

### 决策 5 — Permission UX

**选择：**

```text
PermissionOp
  → append ActionRequired { ToolConfirmation { id, tool_name, arguments, prompt } }
  → run() returns Yielded
  → FFI phase=Yielded, THEN emit action_required (replay from conversation)
  → ChatAgent upsert 本地气泡 (KimMsgKind.agentCard, key 按 call_id 稳定)
  → 用户 Allow once / Always allow / Deny（先本地 state=resolved 灰按钮）
  → session.respond_permission(...)  仅 Yielded 合法
  → 追加 ActionRequiredData::ToolConfirmationResponse { id, permission }
  → run() from step 0
```

按钮映射 `goose_provider_types::permission::Permission`：Allow once → `AllowOnce`；Always → `AlwaysAllow`（写入 `PermissionStore`）；Deny → `DenyOnce`；没有「Always deny」主按钮（设置页里用 never_allow）。`Cancel` 用于 abort 整轮，走现有 `session.abort()`。

**拒绝：** 系统级 modal（离开聊天会丢上下文）；把确认做成非持久 overlay（滚动即消失）。卡片作为 `KimChatMsg` 落 `ConversationStore`，`body` 为 JSON。

### 决策 6 — Sandbox

**选择：** 早期不做 OS sandbox。控制面：

- profile `tools.fs` / `tools.bash` 默认 false（与今天设置页一致）。
- fs：只暴露 `read_file` / `list_dir`；`write_file` 另开关 `tools.fs_write`，默认 false。根目录固定 `KimPaths.agentWorkspace`，拒绝 `..` 逃逸。
- bash：独立 `tools.bash`，默认 false，权限默认 `ask_before`，超时 30s，**不经过 shell**（argv 数组，仿 `declarative::AuthConfig` 注释：never interpolating a shell）。
- 后续可选调研 macOS seatbelt，**不写进 PR1–PR7 的 API 承诺**。Goose 已发布 crates 无通用 sandbox trait。

### 决策 7 — Secrets

**选择：**

- 每个 profile 一个 key ref：`agent.api_key.<profile_id>`，放 `SettingsStore.productionSecureStorage()`（已有 macOS Data Protection keychain，`sdk/mobile/lib/core/settings.dart:62-72`）。
- 默认 profile 迁移：读旧键 `agent.api_key`，写入 `agent.api_key.goose`，保留旧键一个版本以免回滚丢 key。
- Profile 持久化 JSON（SharedPreferences `agent.profiles`）只含 `provider` / `base_url` / `model` / tools / prompt，**禁止** `api_key` 字段；serde 若遇到则丢弃并 warn。
- Rust 只在 `session_open` / `reconfigure` / `fetch_supported_models` 接收 key；`Inner` 持有 `Provider` 后不再把 key 放进可序列化 profile。
- 日志：`tracing` 可记 `provider` / `model` / `profile_id`，禁止 `base_url` 带 userinfo、禁止 key。

### 决策 8 — Declarative providers

**选择：**

- Phase 1 内置引擎：`openai` / `anthropic` 继续走现有 builder（`lib.rs:363-395`）。
- `openai_compatible`：同一 `OpenAiProviderBuilder`，自定义 `base_url`（今天设置页已能改 URL，只是 UI 只展示两家）。
- Bundled JSON：PR2 用 `fixed_provider_config_entries()` **列出** name/display_name。真正 `from_json` 只启用 **字面 `base_url`、无 required `env_vars`** 的条目（groq 等字面 URL 可以；依赖进程环境的跳过，设置页灰掉并注明「桌面/需环境变量」）。`SessionKeyResolver` 只解析 `api_key_env`。openai_compatible 继续走 KIM 的 `OpenAiProviderBuilder` + `split_openai_url`（`lib.rs:397-412` 固定 `OPEN_AI_DEFAULT_BASE_PATH`）；declarative 的完整 `.../chat/completions` path 走 `from_declarative_config`，不要塞进 `split_openai_url`。
- 用户自定义 JSON 目录（`declarative::load_custom_providers`）= 独立后续 PR，不在 PR1。

### 决策 9 — Session persistence

**选择：** **PR7 之前**保持 `Store.conversations: HashMap<String, Conversation>`。`resume_on_open=true` 的语义 = 「同进程同 session_id 复用 HashMap」；跨进程冷启动是空会话。文档与设置文案必须改掉「SQLite session tree is preserved」（现 Dart 注释 `session.dart:34` 是错的）。

**PR7：** 把 `sqlite_path` 真正当作文件路径。`ChatAgent` 传 `KimPaths.agentSessions/<canonical_dest>__<profile_id>.json`。优先 JSON 文件。逻辑 id 与路径分离：新增 `SessionOpenOpts.session_id`；旧客户端 `sqlite_path` 不含 `/` 且非绝对时仍当 session_id、不落盘。Profile 元数据 **不** 写这个目录（见 Data Model：`agent.profiles` 只在 SharedPreferences）。

### 决策 10 — Streaming UI

**选择：** Phase 1 保持「等 `assistant_finished` 再插一条气泡」。Phase 3 加 tool/确认气泡。流式 delta 气泡（边生成边改同一 `KimChatMsg`）为 Phase 3 可选，默认关（`agent.stream_deltas` pref），因为 `KimChatMsg` 无「正在生成」态，需要 `status=sending` + 原地 `copyWith(body:)`。

---

### 核心类型

以下为 KIM 自有类型（不存在于 goose-agent）。放 `crates/kim-agent-host/src/profile.rs`。

```rust
use goose_provider_types::goose_mode::GooseMode;
use goose_provider_types::thinking::ThinkingEffort;
use serde::{Deserialize, Serialize};

/// Stable id. Default persona is "goose" (IM dest alias too).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProfile {
    pub id: String,
    pub display_name: String,
    /// Mention tokens in addition to `@id` and `@display_name`.
    #[serde(default)]
    pub aliases: Vec<String>,
    pub provider: ProviderSpec,
    pub model: ModelSpec,
    pub system_prompt: String,
    /// Missing JSON → SmartApprove. Never GooseMode::Auto (enum Default).
    #[serde(default = "default_smart_approve")]
    pub mode: GooseMode,
    /// 助手 default 16; coder template 32. Missing → Some(16) at from_legacy.
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub tools: ToolSet,
    #[serde(default)]
    pub permissions: PermissionConfig,
    #[serde(default)]
    pub sandbox: SandboxPolicy,
    #[serde(default)]
    pub extensions: Vec<ExtensionSpec>,
    #[serde(default = "enabled_true")]
    pub enabled: bool,
}

fn enabled_true() -> bool {
    true
}

fn default_smart_approve() -> GooseMode {
    GooseMode::SmartApprove
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderSpec {
    /// "openai" | "anthropic" | "openai_compatible" | bundled declarative name
    /// (e.g. "groq", "deepseek"). Not a secret.
    pub kind: String,
    #[serde(default)]
    pub base_url: String,
    /// flutter_secure_storage key. Never the raw secret.
    pub key_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSpec {
    pub name: String,
    pub thinking_effort: Option<ThinkingEffort>,
    pub temperature: Option<String>, // stored as string to avoid JSON f32 pain; parse in factory
    pub max_tokens: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolSet {
    pub send_message: bool,
    pub search_contacts: bool,
    pub search_messages: bool,
    pub get_conversation_context: bool,
    pub read_clipboard: bool,
    /// Dart-only. Never advertised by a Rust ToolProvider / DeferredKim hybrid.
    pub list_profiles: bool,
    pub fs: bool,
    pub fs_write: bool,
    pub bash: bool,
}

impl ToolSet {
    /// Names DeferredKimToolOp advertises. `list_profiles` is Dart-only: still
    /// listed here so the yield path is the same, but the executor is always
    /// KimCapabilityHost reading AgentProfileStore — never a host ToolProvider.
    pub fn kim_world_names(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.send_message { v.push("send_message"); }
        if self.search_contacts { v.push("search_contacts"); }
        if self.search_messages { v.push("search_messages"); }
        if self.get_conversation_context { v.push("get_conversation_context"); }
        if self.read_clipboard { v.push("read_clipboard"); }
        if self.list_profiles { v.push("list_profiles"); }
        v
    }

    pub fn has_in_process(&self) -> bool {
        self.fs || self.fs_write || self.bash
    }

    pub fn has_any(&self) -> bool {
        !self.kim_world_names().is_empty() || self.has_in_process()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDefault {
    AlwaysAllow,
    AskBefore,
    NeverAllow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionConfig {
    /// Tool name → override. Unlisted tools use `default_for(name, mode)`.
    #[serde(default)]
    pub tools: std::collections::BTreeMap<String, PermissionDefault>,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self { tools: std::collections::BTreeMap::new() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxPolicy {
    /// Reserved. PR1–PR7 ignore. Never claim enforcement.
    #[serde(default)]
    pub mode: SandboxMode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionSpec {
    pub name: String,
    /// "stdio" | "sse" — Phase 6
    pub transport: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub url: String,
}

/// Secrets travel beside the profile, never inside it.
pub struct ResolvedProfile {
    pub profile: AgentProfile,
    pub api_key: String,
    pub project_root: std::path::PathBuf,
}
```

`GooseMode` / `ThinkingEffort` 直接用 goose-provider-types，避免再定义一套。FFI 侧用 string 传输（`auto` / `approve` / `smart_approve` / `chat`；`off|low|medium|high|max`），host 内 `parse`。单测：JSON 无 `mode` 字段 + tools 非空 → `SmartApprove`，**不是** `Auto`。

### Host 运行时类型

```rust
// crates/kim-agent-host/src/events.rs
pub enum HostEvent {
    TextDelta { delta: String },
    ToolRequest { call_id: String, name: String, arguments_json: String },
    ToolResult { call_id: String, name: String, output_preview: String, ok: bool },
    ActionRequired { call_id: String, name: String, arguments_json: String, prompt: String },
    Usage { input_tokens: u64, output_tokens: u64 },
    Finished { text: String },
    Failed { message: String },
}

pub enum TurnOutcome {
    Finished { text: String },
    Yielded { kind: YieldKind, call_id: String, name: String },
}

pub enum YieldKind {
    ToolRequest,
    ActionRequired,
}

pub enum HostError {
    MissingApiKey,
    UnknownProvider(String),
    InvalidUrl(String),
    Busy(String),
    UnknownSession(String),
    UnknownToolCall(String),
    Profile(String),
    Failed(String),
}

pub enum HostEffect {
    Conversation(goose_agent::operation::ConversationEffect),
    Usage(goose_provider_types::conversation::token_usage::ProviderUsage),
}
```

`From<Message> for HostEffect` → `Conversation(AppendMessage)`。`InferenceEffect::record_usage` → `Usage`。`MachineEffect::ensure_message_ids` 委托 `ConversationEffect`。

**`apply_effects`（PR1 起必须写全，否则 Permission/Compaction 无法落地）：**

```rust
async fn apply_effects(&self, session: &HostSession, effects: &mut [HostEffect], _emit: &Emitter) -> Result<()> {
    let mut store = self.inner.store.lock().await;
    let conversation = store.conversations.entry(session.id.clone()).or_insert_with(Conversation::empty);
    for effect in effects.iter_mut() {
        match effect {
            HostEffect::Usage(_) => {}
            HostEffect::Conversation(ConversationEffect::AppendMessage(m)) => conversation.push(m.clone()),
            HostEffect::Conversation(ConversationEffect::ReplaceConversation(c)) => *conversation = c.clone(),
            HostEffect::Conversation(ConversationEffect::PatchToolRequestMeta { tool_call_id, patch }) => {
                patch_tool_request_meta(conversation, tool_call_id, patch)?;
            }
            HostEffect::Conversation(ConversationEffect::SetMessageVisibility { message_id, user_visible, agent_visible }) => {
                set_visibility(conversation, message_id, *user_visible, *agent_visible)?;
            }
        }
    }
    Ok(())
}
```

**不要用 `Operation::set_message_meta` 保存权限。** 那是对 `&Conversation` 的原地 mutate，不产生 effect，`run()` 结束即丢。AllowOnce / 已询问状态只从 conversation 里的 `ActionRequired` + `ToolConfirmationResponse` 按 `id` 重建。PR7 resume 同一规则。不设 host `HashSet` 当权威。

**`AgentHost::prompt` 返回类型按 PR 切开：**

- **PR1–PR3：** 保持 `prompt(...) -> Result<String, HostError>`（全文或错误）。`run_loop` **只发 `TextDelta`**。FFI 在 `host.prompt` 返回 `Ok(text)` 后合成 **一条** `assistant_finished`（`session.rs:222-224` 现状）。不要在 PR1 让 host 发 `Finished`。
- **PR4：** 改为 `-> Result<TurnOutcome, HostError>`，新增 `complete_tool` / `respond_permission`。此时 host 在 `TurnOutcome::Finished` 发 `HostEvent::Finished`，FFI **停止**合成 `assistant_finished`。同一 PR 的 Dart 测试断言恰好一条回复气泡。

```rust
impl AgentHost {
    pub fn new(resolved: ResolvedProfile) -> Result<Self, HostError> { /* ... */ }

    // PR1–PR3 signature (do not change):
    pub async fn prompt(
        &self,
        session_id: &str,
        text: &str,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<String, HostError>;

    // PR4 replaces the above with TurnOutcome + optional context:
    pub async fn prompt_with_context(
        &self,
        session_id: &str,
        text: &str,
        context_json: Option<&str>,
        events: mpsc::Sender<HostEvent>,
        cancel: CancellationToken,
    ) -> Result<TurnOutcome, HostError>;

    pub async fn complete_tool(...) -> Result<TurnOutcome, HostError>;
    pub async fn respond_permission(...) -> Result<TurnOutcome, HostError>;
    pub fn pending_yields(&self, session_id: &str) -> Vec<PendingYield>;
}
```

**`context_json`（PR4）：** 在真正的 user 文本 **之前** 插入：

```rust
Message::user()
    .with_text(preview)
    .with_visibility(false, true) // user_visible=false, agent_visible=true
```

该消息 **不是** kickoff：`messages_since_kickoff` 要最后一条 **user-visible 且非 tool-response** 的 user 消息（`operation.rs:20-30`）。不写入 `ConversationStore` UI。字节上限 8 KiB（截断旧行）。Host 单测：注入 context 后 tool turn 仍以原始用户句为 kickoff。

Yielded 时 host 与 FFI **都拒绝** 新的 `prompt()`（`HostError::Busy` / `"agent waiting for tool"`）。

### MachineFactory 用法

**PR1 `assemble`（可编译、2 步聊天路径）：** 只推 `SystemPromptOp` + `InferenceRunner`。`from_legacy` + `enable_fs_tools=true` 的步数单测：额外推一个空的 `ToolOperation::new()`（尚无 `FsToolProvider`）。不推 ChatGuard / Unknown / Permission / Bash。

```rust
// PR1 only
impl MachineFactory {
    pub fn assemble(
        profile: &AgentProfile,
        provider: Arc<dyn goose_provider_types::base::Provider>,
        model: ModelConfig,
        _project_root: &std::path::Path,
    ) -> Vec<goose_agent::machine::Step<'static, HostSession, HostEffect>> {
        let mut steps = vec![
            Step::Operation(Arc::new(SystemPromptOp { prompt: profile.system_prompt.clone() })),
        ];
        if profile.tools.fs {
            steps.push(Step::Operation(Arc::new(goose_agent::tool::ToolOperation::new())));
        }
        steps.push(Step::Inference(Arc::new(
            goose_agent::inference::InferenceRunner::new(provider, model),
        )));
        steps
    }
}
```

**终态 `assemble`（PR5+ 目标机器，不要复制进 PR1）：** ChatGuard/Unknown 来自 PR3；PermissionOp / DeferredKim 来自 PR4–PR5；`BashToolProvider` 仅 PR5b 且 `profile.tools.bash`（**不要**在 PR5b 前把 `opts.bash_enabled` 映射进 `from_legacy`）。

```rust
// destination machine (PR5+)
pub struct MachineFactory;

impl MachineFactory {
    pub fn assemble(
        profile: &AgentProfile,
        provider: Arc<dyn goose_provider_types::base::Provider>,
        model: ModelConfig,
        project_root: &std::path::Path,
    ) -> Vec<goose_agent::machine::Step<'static, HostSession, HostEffect>> {
        use goose_agent::machine::Step;
        use std::sync::Arc;
        let mut steps = Vec::new();
        steps.push(Step::Operation(Arc::new(SystemPromptOp {
            prompt: profile.system_prompt.clone(),
        })));
        if let Some(max) = profile.max_turns {
            steps.push(Step::Operation(Arc::new(MaxTurnsOp { max })));
        }
        if profile.mode == GooseMode::Chat && profile.tools.has_any() {
            tracing::warn!(profile_id = %profile.id, "GooseMode::Chat ignored because ToolSet is non-empty");
        }
        let chat_only = !profile.tools.has_any() && profile.extensions.is_empty();
        if !chat_only {
            steps.push(Step::Operation(Arc::new(PermissionOp {
                mode: profile.mode,
                config: profile.permissions.clone(),
            })));
            let kim = profile.tools.kim_world_names();
            if !kim.is_empty() {
                steps.push(Step::Operation(Arc::new(DeferredKimToolOp {
                    names: kim.into_iter().map(str::to_string).collect(),
                })));
            }
            let mut tools = goose_agent::tool::ToolOperation::new();
            if profile.tools.fs {
                tools = tools.with_provider(Arc::new(FsToolProvider {
                    root: project_root.to_path_buf(),
                    writable: profile.tools.fs_write,
                }));
            }
            if profile.tools.bash {
                tools = tools.with_provider(Arc::new(BashToolProvider {
                    root: project_root.to_path_buf(),
                }));
            }
            for ext in &profile.extensions {
                // PR6: tools = tools.with_provider(mcp_provider(ext));
                let _ = ext;
            }
            steps.push(Step::Operation(Arc::new(tools)));
            steps.push(Step::Operation(Arc::new(UnknownToolOp)));
        } else {
            steps.push(Step::Operation(Arc::new(ChatGuardOp)));
        }
        steps.push(Step::Inference(Arc::new(
            goose_agent::inference::InferenceRunner::new(provider, model),
        )));
        steps
    }
}
```

`model_config` 升级：

```rust
fn model_config(spec: &ModelSpec) -> Result<ModelConfig, HostError> {
    let mut cfg = ModelConfig::new(&spec.name);
    if let Some(effort) = spec.thinking_effort {
        cfg = cfg.with_thinking_effort(effort);
    }
    if let Some(max) = spec.max_tokens {
        cfg = cfg.with_max_tokens(Some(max));
    }
    if let Some(raw) = spec.temperature.as_deref() {
        let t: f32 = raw.parse().map_err(|_| HostError::Profile("bad temperature".into()))?;
        cfg = cfg.with_temperature(Some(t));
    }
    Ok(cfg)
}
```

### Provider 构建

```rust
struct SessionKeyResolver {
    api_key: String,
}

impl goose_providers::declarative::KeyResolver for SessionKeyResolver {
    type Error = std::io::Error;
    fn resolve_key(&self, _name: &str) -> Result<String, Self::Error> {
        if self.api_key.trim().is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "api key missing"));
        }
        Ok(self.api_key.clone())
    }
}

fn build_provider(spec: &ProviderSpec, api_key: &str) -> Result<Arc<dyn Provider>, HostError> {
    match spec.kind.as_str() {
        "openai" | "openai_compatible" | "responses_http" | "live" | "responses" | "" => {
            // existing OpenAiProviderBuilder path; openai_compatible is the same builder + custom URL
            build_openai(spec, api_key)
        }
        "anthropic" | "messages" => build_anthropic(spec, api_key),
        "scripted" => {
            // test-only; see PR1
            Err(HostError::UnknownProvider("scripted outside tests".into()))
        }
        other => {
            let json = bundled_declarative_json(other)?; // look up FIXED_PROVIDERS by name
            let boxed = goose_providers::declarative::from_json(
                json,
                None,
                SessionKeyResolver { api_key: api_key.to_string() },
            )
            .map_err(|e| HostError::Failed(e.to_string()))?;
            Ok(Arc::from(boxed))
        }
    }
}
```

### FFI 增量（`SessionOpenOpts` 向后兼容）

现有字段全部保留。新增：

```rust
pub struct SessionOpenOpts {
    pub model: String,
    pub llm_backend: String,
    pub resume_on_open: bool,
    pub base_url: String,
    pub api_key: String,
    pub enable_fs_tools: bool,
    pub bash_enabled: bool,
    // NEW — empty means "synthesize a profile from the fields above"
    pub profile_id: String,
    pub profile_json: String,      // AgentProfile JSON, no api_key
    pub thinking_effort: String,   // "" | off|low|medium|high|max
    pub goose_mode: String,        // "" | auto|approve|smart_approve|chat
    pub enable_kim_tools: bool,    // PR4; Default false
    pub enable_approvals: bool,    // PR5; Default false — write tools not advertised until true
}

### `session_open` 映射（PR1–PR8 唯一真相）

`profile.tools` / `profile.mode` / `profile.permissions` 在 `session_open` 之后是 host 的权威。Dart flag **只**决定 `toOpts` 置哪些 bit，**不**在 ChatAgent 里丢弃 host 已发出的 tool 事件。禁止生产路径把 SmartApprove 当成 Auto。

```text
session_open(opts):
  if opts.profile_json 非空:
      profile = serde AgentProfile (无 api_key)
      // 调用方 (PR4+ ChatAgent) 已把 tools/mode/permissions 写进 JSON
  else:
      profile = from_legacy(opts)

from_legacy(opts) -> AgentProfile { id: "goose", max_turns: Some(16), ... }:
  tools.fs        = opts.enable_fs_tools       // PR3 起生效
  tools.fs_write  = false                      // 设置页另开后才进 profile_json
  tools.bash      = false                      // 忽略 opts.bash_enabled 直到 PR5b
  tools.search_contacts =
  tools.search_messages =
  tools.get_conversation_context = opts.enable_kim_tools
  tools.list_profiles            = opts.enable_kim_tools
  tools.send_message  =
  tools.read_clipboard = opts.enable_kim_tools && opts.enable_approvals
  if tools.has_any():
      mode = parse(opts.goose_mode) ?? SmartApprove
      // Chat 禁止与非空 ToolSet 共存
  else:
      mode = Chat
```

| 阶段 | ChatAgent 送什么 | 结果机器 |
|---|---|---|
| PR1–PR2 | `toOpts()`，新字段空/false，`profile_json=""` | `from_legacy` → 空 tools → `GooseMode::Chat` → **恰好 2 步** SystemPrompt+Inference |
| PR3 | 同上 + 用户打开 fs 则 `enable_fs_tools=true` | tools.fs → SmartApprove + ToolOperation(fs) + ChatGuard/Unknown |
| PR4 | `profile_json` = goose 行（读 `AgentProfileStore`，fallback 旧键）或 legacy + `enable_kim_tools` | 读 IM 工具 + DeferredKim。**不**广告 send_message / clipboard |
| PR5 | `enable_approvals=true` 写入 profile.tools.send_message | PermissionOp + 写工具 |
| PR8 | 每 persona 一份 `profile_json`；session 键 `(threadDest, profileId)` | 多机器 |

`AgentSettings.toOpts` 从 PR3 起映射 `enableFsTools` → `enable_fs_tools`；从 PR4 起映射 store 的 kim 开关 → `enable_kim_tools`。不再另设 `agent.enable_tools` 让 UI 忽略事件。

PR1 `SessionOpenOpts` 加字段后：`rg "SessionOpenOpts\\(" sdk/mobile` 更新每一个手写构造（含测试）。Rust `Default` 新字段为 `false`/`""`。移动端 native 与 Dart 同列车发布，不考虑旧 so。

// NEW FFI functions (same crate, FRB codegen required)
pub fn fetch_supported_models(opts: SessionOpenOpts) -> Result<Vec<String>, String>;
pub fn list_builtin_profiles() -> Result<Vec<String>, String>; // JSON strings

impl AgentSession {
    pub fn complete_tool(&self, call_id: String, output_json: String) -> Result<String, String>;
    pub fn respond_permission(&self, call_id: String, permission: String) -> Result<String, String>;
    pub fn prompt_with_context(&self, text: String, context_json: String) -> Result<String, String>;
}
```

`prompt` 在 PR1 保留（返回 operation_id 字符串）。空 `profile_json` 走 `from_legacy`。

`AgentUiEvent.kind` 扩展（字符串，不加 enum）：

| kind | 填充字段 | 阶段 |
|---|---|---|
| `session_ready` | — | 已有 |
| `operation_started` | `operation_id` | 已有 |
| `assistant_text_delta` | `delta` | 已有，Phase 1 UI 仍忽略 |
| `assistant_finished` | `message`, `ok=true` | 已有 |
| `failed` / `aborted` | `message` / `stop_reason` | 已有 |
| `tool_request` | `call_id`, `name`, `arguments_json` | PR3/4 |
| `tool_started` | `call_id`, `name` | PR3 |
| `tool_finished` | `call_id`, `name`, `output_preview`, `ok` | PR3 |
| `action_required` | `call_id`, `name`, `arguments_json`, `message` | PR5 |
| `usage` | `input_tokens`, `output_tokens` | PR3 |

### Dart 侧

```dart
class AgentProfile {
  const AgentProfile({
    required this.id,
    required this.displayName,
    this.aliases = const [],
    required this.providerKind,
    required this.baseUrl,
    required this.model,
    required this.keyRef,
    required this.systemPrompt,
    this.mode = 'smart_approve',
    this.maxTurns,
    this.thinkingEffort = '',
    this.tools = const AgentToolSet(),
    this.permissionOverrides = const {},
    this.enabled = true,
  });

  final String id;
  final String displayName;
  final List<String> aliases;
  final String providerKind;
  final String baseUrl;
  final String model;
  final String keyRef;
  final String systemPrompt;
  final String mode;
  final int? maxTurns;
  final String thinkingEffort;
  final AgentToolSet tools;
  final Map<String, String> permissionOverrides;
  final bool enabled;

  String get dest => id == kGooseAgentId ? kGooseAgentId : 'agent:$id';
}

class KimCapabilityHost {
  KimCapabilityHost(this.ref);
  final Ref ref;

  Future<String> execute({
    required String name,
    required String argumentsJson,
    required String sessionDest,
    required String profileId,
  }) async { /* switch (name) ... */ }
}
```

`ChatAgent` 事件循环（PR3 起）：

```dart
_sub = session.listen().listen((ev) {
  switch (ev.kind) {
    case 'assistant_finished':
      unawaited(_appendLocal(dest, ev.message.trim()));
    case 'failed':
      unawaited(_appendLocal(dest, ev.message.trim()));
    case 'tool_request':
      unawaited(_onToolRequest(dest, ev));
    case 'tool_started':
    case 'tool_finished':
      unawaited(_upsertToolCard(dest, ev));
    case 'action_required':
      unawaited(_appendActionCard(dest, ev));
    default:
      break; // session_ready / delta / usage
  }
});
```

`_onToolRequest`：只在收到 **phase 转换之后** 的 `tool_request` 时调用；`KimCapabilityHost.execute` → **串行** `session.completeTool`（见协议一页纸）。`send_message` 若 dest 是 agent dest 或群 dest 则拒绝，返回 JSON 错误，不抛到机器外。

### `complete_tool` / yield 协议（一页纸，PR4 起）

**状态（FFI `SessionPhase`，PR4 替换 `AtomicBool busy`）：** `Idle` | `Running` | `Yielded`。`snapshot().phase` 与 `pending_call_ids: Vec<String>` 一并导出。

```text
state  (AgentSession)

Idle ──prompt()──► Running ── machine.run()
                     │
                     ├─ Ok(Finished)  → Idle  + assistant_finished
                     ├─ Ok(Yielded)   → Yielded ★ THEN replay events
                     ├─ Err           → Idle  + failed
                     └─ abort()       → Idle  + aborted
Yielded ──complete_tool / respond_permission──► append ★
                     │
                     ├─ pending still non-empty → stay Yielded
                     └─ pending empty           → Running ── run() from 0
Yielded ──prompt()──► ✗ "agent waiting for tool"
Yielded ──abort()/close()──► append error ToolResponse per pending id → Idle
Running ──complete_tool──► ✗
Idle    ──complete_tool──► ✗ unknown / not yielded
```

锁：一条 assistant 两个 ToolRequest → complete 第一个 **仍 Yielded**，complete 第二个才 `run()`。不要在第一次 complete 时进入 Running。

**事件顺序（硬约束）：**

1. `run()` 返回 `TurnOutcome::Yielded`。
2. FFI 设 `SessionPhase::Yielded`，清 running/`busy`。
3. **然后**从 conversation 扫描未回答的 KIM `ToolRequest` 与未响应的 `ActionRequired::ToolConfirmation`，对每个 id **恰好**发一条 `tool_request` 或 `action_required`。
4. `listen()` 不得在 `RecvError::Lagged` 时丢掉这些 kind：改用有界 replay buffer（session 内 pending 列表）；新订阅者先收到当前 pending 再跟 live。Dart 用 `Set<call_id>` 去重。

**`complete_tool` 规则：**

- 仅 `Yielded` 合法。
- `call_id` 必须对应未回答的 `ToolRequest`；错误 session / 已 Finished / 已有 `ToolResponse` → `UnknownToolCall`。
- **单飞 + 多 pending：** FFI mutex 串行化 `complete_tool` / `respond_permission`。每次调用 **只 append** 对应 `ToolResponse` / `ToolConfirmationResponse`，从 pending 去掉该 id。pending **仍非空** → 留在 `Yielded`。pending **空** → 自动 `run()`（`Running`）。禁止在 Running 时 complete。脚本测试：一条 assistant 两个 ToolRequest → 两个 `tool_request`（此时 `snapshot.phase==yielded`）→ complete 第一个仍 Yielded → complete 第二个才 Finished。
- `output_json`：一律当 JSON object/array/string 解析；成功用 `CallToolResult::structured(value)`；解析失败用 `CallToolResult::error([ContentBlock::text(raw)])`。`ToolResponse.id` **必须**等于 `ToolRequest.id`（`Message::with_tool_response`）。
- **超时：** Dart 持有 10 分钟 timer（每轮 Yielded 重置）。到期：对未完成 id `complete_tool(id, {"ok":false,"error":"timeout"})`。FFI 不另开 timer。
- **Abort：** `abort()` 在 Yielded 时为每个 pending 追加 error ToolResponse（`"cancelled"`），phase=Idle，发 `aborted`。之后的 `complete_tool` → `UnknownToolCall`（Dart 忽略）。Dart `execute()` 用 `CancellationToken`/`bool aborted`；Outbox 已发出的 `send_message` **不撤回**（`client_id` = `call_id` 以便用户重试时 Outbox 幂等）。
- **崩溃 / 冷启动（PR7 前）：** in-flight yield 丢失。pending `agentCard` 加载后若 `state=pending` 且无对应 live session：按钮变 Retry/Cancel；Retry 只提示「请再发一条」；Cancel 把卡片 `state=resolved`。PR7 后 conversation 在，但 Dart 仍不自动 complete（Open Question 4 维持：不自动 resume 工具）。
- **Host `Store.busy`：** PR4 删除 host 层 busy，只保留 FFI `SessionPhase`。`complete_tool` 不得在 `prompt` 仍持有 `host.read()` 时进入；因 `run()` 已返回，`prompt` 的 read 已 drop。
- **Dest 切换（PR8 前单 session）：** `close()` = abort 协议。in-flight Dart tool 的 complete 打到已 close 的 session → 错，忽略。

必测：scripted 一个（及两个）ToolRequest → `snapshot.phase==yielded` 且 `busy==false` **之后**才有 `tool_request` 事件 → `complete_tool` 成功 → Finished。

---

## API / Interface Changes

### Before（host）

`AgentHost::new(ProviderConfig)`; `prompt(session_id, text, events, cancel) -> Result<String>`。

### After

- **PR1：** `AgentHost::new` 可从 `ResolvedProfile` 或 `ProviderConfig`（`from_legacy`）构建。**`prompt` 仍 `-> Result<String, HostError>`。** 新增字段的 `SessionOpenOpts` 走 FRB；`HostError` 可加变体但 FFI 仍 `to_string()`。
- **PR4：** `prompt` → `TurnOutcome`；新增 `complete_tool` / `respond_permission`；host 发 `Finished`，FFI 停止合成。
- `ProviderConfig` 保留到 PR8。

### FFI Dart（生成后）

`AgentSession.completeTool` / `respondPermission` / `promptWithContext`；顶层 `fetchSupportedModels` / `listBuiltinProfiles`。

### IM FFI

**无变化。** `send_message` 工具只走 `OutboxNotifier.sendText`（`ThreadKind.user`），**禁止** `KimClientPort.sendMessage`。群 dest / 非好友 / self / `isAgentDest` → 错误 JSON。

### Chat 消息模型

`KimMsgKind` 增加 `agentCard`。`ConversationStore._ensureColumn` 不需要新列：`kind` 已是 TEXT（`conversation_store.dart:141`）。**必须同时改每一个 kind 开关**，否则 `kind='agentCard'` 会当 text 把 JSON 当气泡：

| 位置 | 现行为 | PR3 要求 |
|---|---|---|
| `enum KimMsgKind` `models.dart:281` | `text, image, video` | 加 `agentCard` |
| `KimChatMsg.fromJson` `models.dart:414-418` | 非 video/image → text | 识别 `agentCard` |
| `ConversationStore._msg` `:488-492` | 同上 | 识别 `agentCard` |
| `_msgValues` 写回 | 写 `kind.name` | 写 `agentCard` |
| `kindFromWire` `core/image_extra.dart` | 线协议 | 不变（服务端不会发 agentCard） |
| `KimMessageRow` | 当普通文本 | `kind==agentCard` → `AgentActionBubble` |
| `showChatMessageSheet` `chat_chrome.dart:39` | sys/image/video 复制空 | **agentCard 列入 no-copy** |
| 未知 kind | 当 text | 保持当 text，**唯独** `agentCard` 例外 |

`KimChatMsg.body` 对 agentCard 存 JSON：

```json
{
  "v": 1,
  "type": "action_required",
  "call_id": "toolu_01",
  "name": "send_message",
  "arguments": {"dest": "bob", "text": "hello"},
  "state": "pending"
}
```

`KimMessageRow` 在 `kind==agentCard` 时渲染 `AgentActionBubble`，不走 `sys` 灰字。

**确认卡 Dart 状态机（无 freezed）：** 稳定 `KimChatMsg.key = 'agent-card-${call_id}'`。同一 `call_id` upsert（`copyWith(body:)` 改 JSON `state`）。点按钮：立刻本地 `state=resolved` 灰掉，再 `respondPermission`；失败则回到 pending。两个卡片靠 `body.call_id` 区分。`copyWith` 不能改 `key`/`sender`——一开始就写对。进程死（PR7 前）pending 卡见协议「崩溃」条。PR8 前只有一个 live session：卡片上的按钮只对当前 `_session` 发；切 dest 后旧卡 Cancel。

---

## Data Model Changes

### Flutter persistence

| Key | Store | 内容 |
|---|---|---|
| `agent.profiles` | SharedPreferences | `List<AgentProfile>` JSON，无 key；**permissions 嵌在每行里**，不另开 `agent.permissions.*` |
| `agent.active_profile_id` | SharedPreferences | 设置页当前编辑项；聊天 dest 仍按联系人 |
| `agent.multi_profile` | SharedPreferences | 默认 `false`；true 才在通讯录展开多个 bot |
| `agent.llm_backend` / `base_url` / `model` / `enable_fs_tools` / `bash_enabled` | SharedPreferences | 旧键；见迁移表 |
| `agent.api_key` | secure storage | 旧默认 key；迁移复制到 `agent.api_key.goose`，旧键保留一个版本 |
| `agent.api_key.<id>` | secure storage | 每 profile 一把 key |

Profile JSON **不** 写到 `KimPaths.agentRoot/profiles.json`。会话文件（仅 PR7）才用 `agentSessions/`。

工具参数无独立 schema_version：`send_message` 等 inputSchema **冻结**；破坏性变更换工具名。agentCard JSON `"v": 1` 只描述气泡，不是工具 schema。

#### 谁读/写哪把键

| PR | 设置页写 | ChatAgent `session_open` 读 | 备注 |
|---|---|---|---|
| 0 | 旧 `AgentSettings` 键 + `agent.api_key` | 旧 `toOpts()` | 无 profile |
| 1 | 同上 + `toOpts` 填空新字段 | 旧 `toOpts()` → `from_legacy` | 聊天等价 |
| 2 | **双写** 旧键 + `agent.profiles` goose 行 + `agent.api_key.goose` | 仍旧 `toOpts()`（goose 行优先填 toOpts 的 model/url/backend，fallback 旧键） | 设置 UI 新控件 |
| 3 | 旧 `enable_fs_tools` 双写进 goose `tools.fs` | `toOpts.enable_fs_tools` | 打开 fs 才有工具机器 |
| 4 | goose 行 tools 读开关 | **`profile_json` = goose 行**（无则 from_legacy + `enable_kim_tools`） | 读 IM 工具 |
| 5 | goose 行 `send_message`/`read_clipboard` + permissions | `profile_json` | 写工具 |
| 7 | 不变 | `session_id` + 真路径 | ChatAgent 此 PR 才改 `sqlitePath` |
| 8 | 多行 profiles；可停写旧键 | 每 persona `profile_json` | 旧 `agent.api_key` 仍可读 |

AlwaysAllow 写进 **该 profile 的 `permissions` 字段**（同一 `agent.profiles` 行）。AllowOnce 只存在于 conversation 的 `ToolConfirmationResponse`，PR7 resume 由此重建，不设 host HashSet。

### Host 内存

无独立 permission HashSet。PR7：`KimPaths.agentSessions/<dest>__<profile_id>.json` 存 `{"v":1,"messages":[...]}`（goose `Message` serde）。

---

## KIM-native tool catalog

权限默认随 profile `GooseMode`：`Chat` = 不广告；`Auto` = AlwaysAllow（除非 NeverAllow 覆盖）；`Approve` = AskBefore；`SmartApprove` = 读类 AlwaysAllow、写/副作用 AskBefore。下表是 **SmartApprove + 助手 profile** 的默认。

### `send_message` — executor: Dart → **仅** `OutboxNotifier.sendText`

```json
{
  "name": "send_message",
  "description": "Send a text IM message to a KIM user who is already a friend. Never send to local agents (dest goose or agent:*).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "dest": { "type": "string", "description": "KIM account id" },
      "text": { "type": "string" }
    },
    "required": ["dest", "text"],
    "additionalProperties": false
  }
}
```

Permission default: **ask_before**。循环护栏：`isAgentDest(dest)` → `{"ok":false,"error":"refusing to message a local agent"}`。群 dest：Outbox `_assertCanQueue` 用 `ThreadKind.user` 走好友/account 校验，非好友账号失败；capability 若 `dest` 含 `/` 或已知 group id → 直接 `{"ok":false,"error":"group dest not supported"}`。未好友走 `Copy.notFriends`。成功 `{"ok":true,"client_id":"<call_id>"}`（`client_id` 绑 `call_id` 以便重试幂等）。**禁止**调用 `KimClientPort.sendMessage`。

### `search_contacts` — executor: Dart → `contactsProvider` + `searchUsers`

```json
{
  "name": "search_contacts",
  "description": "Search local friends and optionally the server user directory.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": { "type": "string" }
    },
    "required": ["query"],
    "additionalProperties": false
  }
}
```

Permission default: **always_allow**。实现：先 filter `ContactsState.friends`（排除 bot），空则 `clientPort.searchUsers`。返回 `{"people":[{"account":"...","nickname":"..."}]}`，上限 20。

### `search_messages` — executor: Dart → `ConversationStore.loadMessages`（本地）；可选 `history`

```json
{
  "name": "search_messages",
  "description": "Search cached messages in a thread (current dest if omitted).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "dest": { "type": "string" },
      "query": { "type": "string" },
      "limit": { "type": "integer", "minimum": 1, "maximum": 50 }
    },
    "required": ["query"],
    "additionalProperties": false
  }
}
```

Permission default: **always_allow**。无 FTS：对 `loadMessages(account, dest)` 做 case-insensitive `contains`，`limit` 默认 20。PR7 前不打服务端 `history`。返回 `{"hits":[{"sender":"...","body":"...","at":0}]}`，body 截断 500 字。

### `get_conversation_context` — executor: Dart（也可由 host 在 prompt 时注入）

```json
{
  "name": "get_conversation_context",
  "description": "Return the latest N text messages of a thread for the model to read.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "dest": { "type": "string" },
      "limit": { "type": "integer", "minimum": 1, "maximum": 50 }
    },
    "additionalProperties": false
  }
}
```

Permission default: **always_allow**。`@mention` 路径上 `ChatAgent._prompt` **额外** 传 `context_json`：最近 N 条 text，**总字节 ≤ 8 KiB**，构造为 `Message::user().with_text(...).with_visibility(false, true)`，不是 kickoff，不进 `ConversationStore` UI。

### `read_clipboard` — executor: Dart `Clipboard.getData(Clipboard.kTextPlain)`

```json
{
  "name": "read_clipboard",
  "description": "Read current text clipboard on this device.",
  "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
}
```

Permission default: **ask_before**（剪贴板敏感）。空剪贴板 `{"text":""}`。

### `list_profiles` — executor: **Dart only**（`AgentProfileStore`）

```json
{
  "name": "list_profiles",
  "description": "List enabled local agent profiles (id, display_name). Cannot switch the current session.",
  "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
}
```

Permission default: **always_allow**。由 `DeferredKimToolOp` 广告 schema，**永远没有** Rust `ToolProvider`（禁止 hybrid，以免 `machine.rs:59-61` 重复名）。`KimCapabilityHost` 读 `agent.profiles` 已启用行。Host 在 `session_open` 只看到当前一份 `profile_json`，列不出其它 persona。**不做 `switch_profile` 工具。**

### `read_file` / `list_dir` / `write_file` — executor: Rust `FsToolProvider`

根 = `project_root`（`KimPaths.agentWorkspace`）。路径规范化后 `starts_with(root)` 否则错误。`write_file` 仅 `tools.fs_write`。Permission：read **always_allow**（若 fs 开启），write **ask_before**。

### `bash` — executor: Rust `BashToolProvider`

```json
{
  "name": "bash",
  "description": "Run a process in the agent workspace. Arguments are argv, not a shell string.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "argv": { "type": "array", "items": { "type": "string" }, "minItems": 1 }
    },
    "required": ["argv"],
    "additionalProperties": false
  }
}
```

Permission default: **ask_before**；profile 默认关。超时 30s，stdout+stderr 合预览 8KiB。拒绝 `argv[0]` 含 `..` 或绝对路径出 workspace 之外的解释器调用仍允许（用户已批准）——产品文案必须标明这是高风险。

### MCP — executor: Rust `ToolProvider` wrapping `rmcp` client

Phase 6。工具名使用 goose 惯例 `extension__tool`（`ToolNameParts` 按 `__` 拆，`tool_request.rs:6-18`）。Permission 默认 **ask_before**。

---

## Built-in starter profiles

### `goose` / 助手 — 通用 IM 助手

- dest: `goose`（canonical）
- aliases: `助手`
- `max_turns`: **16**
- provider/model: 从当前 `AgentSettings` 迁移
- mode: `SmartApprove`（仅当 tools 非空；PR1–PR2 空 tools → 实际机器为 Chat）
- **PR1–PR4 模板 tools：** 仅读 IM（`search_contacts`, `search_messages`, `get_conversation_context`, `list_profiles`）。**`send_message` / `read_clipboard` = false。** `list_builtin_profiles` 与默认 goose 行必须如此，否则 PR4 会在无确认卡时广告代发。
- **PR5 同 commit：** 翻 `send_message` / `read_clipboard` = true，装 PermissionOp。prompt 改为：「你有 search_* / get_conversation_context / list_profiles；PR5 起还有 send_message 与剪贴板且需确认。没有 fs/shell。」
- 无 fs、无 bash
- Machine PR5+：SystemPrompt, MaxTurns(16), Permission, DeferredKim, UnknownTool, Inference

### `translator`

- dest: `agent:translator`（`enabled: false` 直到用户打开）
- prompt: 只做翻译，不闲聊、不声称能发消息
- mode: `Chat`
- tools: 全 false
- Machine: SystemPrompt, Inference

### `coder`

- dest: `agent:coder`
- prompt: 软件助手；工作区是 `project_root`；不要 send_message
- mode: `Approve`（fs write / bash 必问）
- tools: `fs=true`, `fs_write=true`, `bash=false` 默认（设置页可开），无 send_message
- Machine: SystemPrompt, MaxTurns(32), Permission, ToolOperation(fs), UnknownTool, Inference

### 用户自定义

设置页「复制为新 Agent」→ 新 uuid id，`enabled=true`。`agent.multi_profile=true` 后出现在通讯录。

---

## Phased Implementation

每一阶段独立可编译。阶段内按文件分组。不写工期。

### Phase 0 — Goose alpha.9，行为不变

**目的：** 锁依赖，修测试谎言，为后续 `ToolOperation` 准备 `rmcp 3.2`。

**`crates/kim-agent-host/Cargo.toml`**

- `goose-agent` / `goose-provider-types` / `goose-providers` → `0.1.0-alpha.9`。

**`Cargo.lock`（workspace）+ `sdk/mobile/rust_agent/Cargo.lock`**

- `cargo update -p goose-agent -p goose-provider-types -p goose-providers`。

**`crates/kim-agent-host/src/lib.rs`**

- 不改行为。若 alpha.9 编译失败，只修 import / 弃用。
- 测试增加：`ProviderKind::parse` 仍接受 `scripted`（别名保留）。

**`sdk/mobile/rust_agent/tests/session_scripted.rs`**

- 现状：空 `api_key` → `session_open` `MissingApiKey`；`assert!(sqlite.exists())` 为假；测试未 `#[ignore]`。
- PR0 **重写**（不 `#[ignore]`，不引入 ScriptedProvider，不改 `session_open` 允许无 key）：
  - `api_key: "sk-dummy".into()`（足以通过 `AgentHost::new`）。
  - **不调用 `prompt`**（dummy key 会打真 OpenAI 或挂 120s）。
  - 去掉 `sqlite.exists()`。
  - `assert!(!session.snapshot().unwrap().busy)`；`close()`。
  - 注释写明：scripted prompt 环路在 PR1 host / PR3 FFI。

**验收：** `cargo test -p kim-agent-host`；`cd sdk/mobile/rust_agent && cargo test` **必须绿**；Dart agent 测试仍过。聊天路径零差异。

### Phase 1 — AgentProfile + MachineFactory，默认 profile = 当前设置

**目的：** host 内部可按 profile 组装机器；对外仍是一个「助手」、无工具。

拆文件（`lib.rs` 现 470 行，继续堆会不可审）：

**新建 `crates/kim-agent-host/src/profile.rs`**

- 上文 `AgentProfile` 等类型。
- `AgentProfile::from_legacy(&SessionOpenOpts) -> AgentProfile`：按「session_open 映射」表。PR1 opts 工具 bit 全 false → 空 ToolSet → `mode=Chat`，`max_turns=Some(16)`。
- `builtin_templates()`：goose 行 **仅读 IM 工具**（PR5 前 send_message=false）；translator Chat+空 tools；coder 的 fs bits 在模板里但 PR1 factory 因 ChatAgent 不送该 JSON 不会用到。
- 单测：无 `mode` 的 JSON + tools 非空 → SmartApprove 不是 Auto；无 `api_key` 字段。

**新建 `crates/kim-agent-host/src/provider.rs`**

- 搬 `ProviderKind` / `ProviderConfig` / `build_openai` / `build_anthropic` / `split_openai_url` / `anthropic_host`。
- `ProviderKind` 保留；新 `ProviderSpec.kind: String` 是超集。
- `build_provider_from_spec(&ProviderSpec, &str) -> Result<Arc<dyn Provider>, HostError>` Phase 1 只走 openai/anthropic 两 builder；其它 kind → `UnknownProvider`（declarative 在 Phase 2 接线）。

**新建 `crates/kim-agent-host/src/machine.rs`**

- `MachineFactory::assemble`。PR1 ChatAgent 送空 tools → **恰好** SystemPrompt + Inference（2 步）。ChatGuard/Unknown 在 PR3；Permission 在 PR5。单测断言 factory 输出步数，不要用产品 coder / PR5+ 终态机器当 PR1 期望。

**新建 `crates/kim-agent-host/src/ops/system_prompt.rs`**

- 现 `SystemPromptOp` 原样搬迁。

**新建 `crates/kim-agent-host/src/events.rs`**

- `HostEvent` 先加 enum 变体（ToolRequest 等），Phase 1 `run_loop` 仍只发 TextDelta；FFI 不映射新变体。这让后续 PR 不改 enum 形状。

**改 `crates/kim-agent-host/src/lib.rs`**

- `mod profile, provider, machine, events, ops;`
- `AgentHost::new` 可从 `ResolvedProfile` 构建，保留 `From<ProviderConfig>`。
- **`prompt` 签名不变** `-> Result<String, HostError>`。
- `run_loop` 用 factory；**只发 TextDelta**。FFI 继续在返回后合成 `assistant_finished`。
- `HostEffect` 包 `ConversationEffect`；`apply_effects` 写全四变体（Replace 此时无调用者）。
- **不**在本 PR 修 reconfigure-保留-store（单独 commit/PR1b 可选；混进 factory 不利于回滚）。
- 单测：空 tools → 2 step；`from_legacy` + `enable_fs_tools` → 含 ToolOperation 的步数（即使 ChatAgent PR1 不置该 bit）。
- CI：`rg kim-client sdk/mobile/rust_agent/Cargo.toml` 必须空（隔离检查从 PR1 起）。

**改 `sdk/mobile/rust_agent/src/api/session.rs`**

- `SessionOpenOpts` 加 `profile_id` / `profile_json` / `thinking_effort` / `goose_mode` / `enable_kim_tools` / `enable_approvals`，Default 空串/false。
- `config_from_opts`：空 `profile_json` → `from_legacy`。
- **不新增 FFI 方法**。新字段必须 codegen（Dart required 构造函数、hashCode、SSE）。
- Checklist：`rg "SessionOpenOpts\\(" sdk/mobile` 更新每一个构造（含 `chat_agent_test` / `agent_settings_test`）。

**FRB：** `cd sdk/mobile && flutter_rust_bridge_codegen generate --config flutter_rust_bridge.agent.yaml`，提交生成物。

**改 `sdk/mobile/lib/state/agent_settings.dart`**

- `toOpts` 填 `profileId: 'goose'`，新 bool 为 false，字符串空。

**验收测试 X（聊天路径等价）：** `sdk/mobile/test/state/chat_agent_test.dart` 仍通过；补一条：fake session 只发一个 `assistant_finished` → 线程里 **恰好一条** 助手气泡。这是 PR1 对「无产品聊天变化」的定义。FRB 布局变化是编译期 break，与 Dart 同 PR 着陆，不叫聊天行为变化。

### Phase 2 — 设置 UI + `fetch_supported_models` + reasoning + 多 provider

**目的：** 用户能配模型下拉、thinking、openai_compatible / bundled declarative；仍无工具。

**`crates/kim-agent-host/src/provider.rs`**

- 接 `declarative::from_json` + `SessionKeyResolver`。
- `pub fn bundled_provider_summaries() -> Vec<(String, String)>` 调 `fixed_provider_config_entries()`。
- `pub async fn fetch_models(spec, key) -> Result<Vec<String>, HostError>`：临时 build provider，调 `fetch_supported_models`，drop。`Ok(vec![])`（trait 默认）与网络失败一律回退 `OPEN_AI_KNOWN_MODELS` / Anthropic 已知列表 / JSON 内 `models`，不得把空列表当「无模型」。

**`sdk/mobile/rust_agent/src/api/session.rs`**

- 新增 **顶层** `pub fn fetch_supported_models(opts: SessionOpenOpts) -> Result<Vec<String>, String>`（不需要已 open 的 session）。
- 新增 `pub fn list_builtin_profiles() -> Result<Vec<String>, String>` 返回 `builtin_templates()` 的 JSON。
- `model_config` 使用 `thinking_effort` 字段。
- **FRB codegen。**

**新建 `sdk/mobile/lib/state/agent_profiles.dart`**

- `AgentProfileStore`：load/save `agent.profiles`；secure key per `key_ref`。
- 迁移：从 `AgentSettings` 填 goose。
- `agentProfilesProvider`。
- 保存 goose 时 **双写** 旧 `AgentSettings` 键，ChatAgent Phase 2 仍读 `AgentSettings.toOpts()`。

**改 `sdk/mobile/lib/screens/agent/agent_settings_page.dart`**

- Provider 选择：OpenAI / Anthropic / OpenAI-compatible / 其它 bundled（搜索列表，数据来自 `list_builtin_profiles` 不强制；bundled 名来自新 FFI `list_bundled_providers` 或把 summaries 放进 `list_builtin_profiles` 旁）。
- 模型：`TextField` 保留；旁加「拉取模型」按钮调 `fetchSupportedModels`，结果 `Dropdown`。网络失败不阻塞手填。
- Reasoning：`SegmentedButton` Off/Low/Medium/High/Max，写入 `thinking_effort`。
- **仍不展示 fs/bash 开关**（或展示但 disabled + 文案「后续版本」），避免用户以为已接线。
- 多 profile UI：本阶段只编辑 goose；页面底部「更多 Agent（即将推出）」不开放创建。避免 Phase 2 就要做通讯录。

**改 `sdk/mobile/lib/l10n/app_*.arb`**

- 新字符串：拉取模型、reasoning、OpenAI-compatible、keychain 说明保持。

**测试：** `agent_settings_test.dart` 覆盖 thinking 写入 prefs；fake FFI 测下拉。host 单测：`SessionKeyResolver` 空 key 失败；bundled unknown name → `UnknownProvider`。

### Phase 3 — 进程内 ToolOperation + 工具气泡

**目的：** fs（可选）在 Rust 执行；UI 显示 tool 卡片；ChatAgent 仍可不执行 KIM 工具。

**新建 `crates/kim-agent-host/src/ops/`** `unknown_tool.rs` + `chat_guard.rs` + `fs.rs` + `max_turns.rs`

- `FsToolProvider`：`read_file`/`list_dir`；path canonicalize + prefix。禁止 panic。
- `UnknownToolOp` / **`ChatGuardOp` 本 PR 上船**（第一台可能看见 ToolRequest 的机器）。Chat-only hallucinated ToolRequest → 错误文本 ToolResponse，避免 `applies` false 导致空 Finished 被 ChatAgent 丢弃。
- Factory：`tools.has_any()` 才装工具步骤。`from_legacy`：`enable_fs_tools` → `tools.fs` + SmartApprove（映射表）。

**接线 `enable_fs_tools`：** 见 session_open 映射。设置页打开 fs 即 host 跑 FsToolProvider。**没有**第二把 `agent.enable_tools` 让 Dart 丢掉事件。`bash_enabled` 仍忽略到 PR5b。

**改 `run_loop` 事件泵**

- 解析 Text / ToolRequest / ToolResponse。进程内工具（fs）在 `run()` 内执行完毕，FFI 可在 Running 期间发 `tool_started`/`tool_finished`（不 yield）。**不要**在 Running 期间发 `tool_request`（那是 Dart 工具，PR4）。
- `project_root` 写入 Inner。

**改 `sdk/mobile/rust_agent/src/api/session.rs`**

- 映射 `tool_started` / `tool_finished`。`prompt` 仍 `Result<String>`。无 `complete_tool`。

**改 kind 全链路：** `models.dart` enum + `fromJson` + `ConversationStore._msg` + `_msgValues` + `kim_bubble` + `chat_chrome` no-copy。未知 kind 仍当 text，**除** `agentCard`。

**新建 `sdk/mobile/lib/widgets/agent_action_bubble.dart`**

- 只读 tool 卡片（name + preview + ok/spinner）。确认按钮 Phase 5 再加。

**改 `sdk/mobile/lib/widgets/kim_bubble.dart`**

- `kind == agentCard` 分支到新 widget。

**改 `sdk/mobile/lib/state/chat_agent.dart`**

- 处理 `tool_started` / `tool_finished`：`_upsertToolCard`（host 只有在 `tools.fs` 时才会发）。

**设置页：** fs 开关解除 hardcoded false，文案「仅限工作区只读」。写入 `enableFsTools` → `toOpts.enable_fs_tools`。默认仍 false。

**测试：** host：临时目录 `read_file`；`..` 逃逸失败。`tool_operation` 风格：用 `ScriptedProvider` 吐出 ToolRequest。

**新建 `crates/kim-agent-host/src/scripted.rs`（cfg(test) + `kind=scripted`）**

- 实现 `Provider`：`stream` 按队列吐 `Message`。供 host 测工具环，也让 `session_scripted.rs` 终于能 prompt 而不要 API key。
- `llm_backend=scripted` 生产也可用于 demo，不访问网络。

### Phase 4 — Dart KimCapabilityHost + `complete_tool`

**目的：** IM 工具闭环。两条 FFI 仍隔离。

**新建 `crates/kim-agent-host/src/ops/deferred_kim.rs`**

```rust
pub struct DeferredKimToolOp {
    pub names: Vec<String>,
}

#[async_trait]
impl Operation<HostSession, HostEffect> for DeferredKimToolOp {
    fn name(&self) -> &'static str { "kim_tools" }

    async fn inference_tools(&self, _s: &HostSession) -> Result<Vec<rmcp::model::Tool>> {
        Ok(self.names.iter().filter_map(|n| kim_tool_schema(n)).collect())
    }

    async fn run(&self, _s: &HostSession, conversation: &Conversation, _emit: &Emitter)
        -> Result<OperationResult<HostEffect>>
    {
        let pending = pending_unanswered_named(conversation, &self.names);
        if pending.is_empty() { return not_applicable(); }
        yielded() // 不在 run() 内发 UI 事件
    }
}
```

**事件在 `run()` 返回、`SessionPhase::Yielded` 之后** 由 FFI 从 conversation 重放（协议一页纸）。Inference 期间的 `Emitter::message(ToolRequest)` 可被 pump 忽略或只作日志。

**改 `AgentHost`：** `prompt` → `TurnOutcome`；`complete_tool` 按协议 append；pending 清空才 `run()` from 0。**本 PR 起 host 发 `Finished`，FFI 停止合成。** Dart 测试：恰好一条气泡。

**改 FFI：** `SessionPhase` + `complete_tool` + `snapshot.phase` + `pending_call_ids`；listen replay；删除 host `Store.busy`。见协议一页纸。

**ChatAgent 本 PR 开始送 `profile_json`**（goose 行；无则 `enable_kim_tools=true` 的 from_legacy）。依赖 PR2 的 `AgentProfileStore`。

**FRB codegen。**

**新建 `sdk/mobile/lib/agent/capability_host.dart`**

- 实现 catalog。`send_message` 调 `outboxProvider.notifier.sendText`（复用好友校验与本地草稿）。
- `search_messages` 用 `conversationStoreProvider.loadMessages`。
- `get_conversation_context` 同上。
- `read_clipboard`：`Clipboard.getData`。
- 错误一律 JSON `{"ok":false,"error":"..."}`，不让 Dart 异常冒泡成 `failed` 事件（工具失败应回模型）。

**改 `chat_agent.dart`**

- `tool_request` → capability host → 串行 `completeTool`（pending 空才续跑）。
- `_prompt` 对非 agent dest 附带 `context_json`（≤8KiB，`with_visibility(false,true)`）。`prompt_with_context` FFI。
- Direct DM 不贴其它线程。
- 两个 ChatPage 同一 dest：单例 `chatAgentProvider` 已串行，文档化即可。

**测试：**

- host：scripted `search_contacts` → Yielded；`snapshot.phase==yielded` 后才有事件；`complete_tool` → Finished。两个 ToolRequest 的 batch 测试。
- kickoff 测试：context 注入后 kickoff 仍是用户原句。
- Dart：`KimCapabilityHost` fake store；`send_message` 到 goose / 群 dest → error JSON。
- Cargo.toml 隔离 grep（PR1 已有，本 PR 保持）。

**`project_root` / context：** 本阶段也可把 `@mention` 的「粘贴会话」做实。

### Phase 5 — PermissionOp + 确认卡 + PermissionStore

**新建 `crates/kim-agent-host/src/ops/permission.rs`**

- `run`：扫描 kickoff 以来未确认 ToolRequest。
- 查 `PermissionConfig` + `GooseMode`：AlwaysAllow → NotApplicable（让后续 op 执行）；NeverAllow → 写错误 ToolResponse（applied，不 yield）；AskBefore 且尚无 `ToolConfirmationResponse` → append `ActionRequired`（user_visible true, agent_visible false）+ `yielded_with`。
- 已有 matching `ToolConfirmationResponse`：Allow* → NotApplicable；Deny* → 错误 ToolResponse。
- **禁止 `set_message_meta`。** 重复询问靠 conversation 里已有 `ActionRequired` 且尚无 Response。

**FFI `respond_permission`：** 解析 string → `Permission`。AlwaysAllow 由 Dart 写进 **该 profile 的 `permissions` 字段**（`agent.profiles` 同一行），下次 `profile_json` 带上。AllowOnce 只留在 conversation。

**Dart `AgentActionBubble`：** 三个按钮。pending 时可点；完成后 `state=resolved` 灰掉。

**改 `ChatAgent`：** `action_required` → `_appendActionCard`；按钮 → `respondPermission`。

**设置页：** 每工具权限覆盖（always / ask / never）。

**测试：** scripted 发 `send_message` ToolRequest + SmartApprove → Yielded ActionRequired；AllowOnce 后续 DeferredKim yield；Deny 后 Finished 带「用户拒绝」。

**GooseMode::Chat：** 空 ToolSet 时 factory 不装 Permission/Deferred/ToolOperation；ChatGuard 已在 PR3。本 PR 把 goose 模板的 `send_message`/`read_clipboard` 翻开。

### Phase 6 — MCP extensions

**新建 `crates/kim-agent-host/src/ops/mcp.rs`**

- 对每个 `ExtensionSpec`，stdio：`rmcp` client（goose-agent 已依赖 `rmcp` 3.x，features `client` 若默认没有则在 **kim-agent-host** 加 `rmcp` 依赖，不要改 goose-agent）。
- `ToolProvider`：`tools()` = MCP list；`call` = MCP call。名字 `"{ext}__{tool}"`。
- 生命周期：session_open 连、close 断。失败是 `HostError::Failed`，不 panic。

**设置页：** 高级「MCP 扩展」列表（command argv）。默认空。

**文档：** 仍禁止 agent FFI 依赖 kim-client。

### Phase 7 — 会话持久化 + compaction

**`Store`：** `load` 时若内存没有，读 `session_path` JSON。`apply_effects` 后原子写（tmp + rename）。

**FFI：** 新增 `SessionOpenOpts.session_id`（逻辑 id，建议 `"{threadDest}::{profileId}"`）。`sqlite_path` 真正是路径。`ChatAgent` **本 PR** 传 `KimPaths.agentSessions/<dest>__<profile_id>.json`。旧客户端：path 不含 `/` 且非绝对 → 仍当 session_id，不落盘。

**新建 `ops/compaction.rs`：** 估算字符/4 ≈ tokens，超过 `provider.get_context_limit(&model, None).await` 的 70% 则 `ReplaceConversation`。规则摘要即可。AllowOnce 随 JSON 消息复活。

**`resume_on_open`：** true 且文件存在 → load；false → 空 conversation 但仍可写同一路径。

### Phase 8 — 多 Agent 通讯录 + @mention

**续：产品闭环与厂商目录见 [multi-agent-vendor-catalog.md](./multi-agent-vendor-catalog.md)**（C-KD；桌面 `agent.multi_profile` 默认 true；`/agent` 列表与编辑器；`botDelete` 后去掉本机人设行）。下文是当时 Phase 8 的通讯录 / mention 切片，不要再把它当「即将推出」。

**改 `sdk/mobile/lib/agent/mention.dart`**

```dart
bool isAgentDest(String dest) =>
    dest == kGooseAgentId || dest.startsWith('agent:');

String canonicalAgentDest(String dest) {
  if (dest == 'agent:goose') return kGooseAgentId;
  return dest;
}

List<KimPerson> withLocalAgents(List<KimPerson> people, List<AgentProfile> enabled) { ... }

bool mentionsAgent(String text, List<AgentProfile> enabled) { ... }

AgentProfile? mentionedProfile(String text, List<AgentProfile> enabled) { ... }
```

保留 `isGooseAgentDest` 为 `dest==goose || dest==agent:goose` 的 typedef 包装，避免全仓瞬时爆炸；逐步替换。

**改 contacts / inbox / outbox / chat_session / chat_page / contacts_page**

- `withGooseAgent` → `withLocalAgents`。
- 通讯录「本地 Agent」节：对每个 enabled profile 一个 ListTile。
- `Outbox._assertCanQueue`：任何 `isAgentDest` 都拒绝。
- `ChatAgent`：`Map<(threadDest, profileId), _Live>` LRU 4。`onOutgoingText` 用 `mentionedProfile`。群/真人线程 Agent 回复仍 `_appendLocal`，其它成员不可见。
- `_appendLocal` `sender = profile.display_name`，account = canonical dest。
- `contacts.person` / `isFriend` / `chat_page` 好友门：全部改 `isAgentDest`（否则 `agent:translator` 会撞 not-friends）。
- `inbox.withGooseThread` → 每个 enabled profile 一条本地线程。
- flag `agent.multi_profile` 曾默认 false。桌面默认 true 与 `/agent` 路由见 catalog 设计 C-KD 6 / 15。

**设置 / 列表：** `/agent` 列表、`/agent/:id` 编辑器、`/agent/accounts`。复制、删除（不可删 goose；已注册则 `botDelete` 后去掉本机行）。

**测试：** mention 正则；同一 thread 两个 profile 两个 session；`agent:goose` 归一；气泡 sender 不是总是「助手」。

### Phase 9 — Subagent / Steer（可选）

**不承诺进主路径。** 若做：

- `SteerOp`：把 profile `steer: String` 追加进 `prompt_parts`（system 之后）。极小。
- `SubagentOp`：工具 `delegate` 在嵌套 `StateMachine` 上跑 **Chat-only 或更小 ToolSet** 的 child profile，同步等待 Finished，结果当 ToolResponse。禁止 child 再 `send_message`（硬编码 child ToolSet 关掉 IM 写）。无 OS sandbox。

对照 unpublished 管道仅抄模式，不引入 Recipe/Skill/Doctor。

---

## Architectural Notes

- **机器是纯函数于 conversation。** 权限状态只看 `ActionRequired` / `ToolConfirmationResponse`。禁止 `set_message_meta` 当持久化。PR7 resume 同一规则。
- **busy vs yield。** `run()` 返回 Yielded → 设 phase → **然后**发事件。`complete_tool` 仅 Yielded。见协议一页纸。
- **reconfigure 丢会话。** 现状换 Inner 丢 HashMap。不塞进 PR1 factory（利于回滚）。需要时单独 PR1b：`mem::replace` provider、move 出 `store`。
- **Finished 双发。** PR1–PR3：FFI 继续合成 `assistant_finished`，host 只发 TextDelta。PR4：host 发 `Finished`，FFI 停止合成。同 PR 测一条气泡。
- **TextDelta 累积。** ChatAgent PR1 仍只用 Finished 全文。
- **rmcp Tool schema。** `Tool::new(name, description, schema: Arc<JsonObject>)`。
- **日志：** `send_message` / clipboard 的 `arguments_json` 不打印；preview 截断 80 字打码。禁止 `Debug` 打印整个 `SessionOpenOpts`（含 `api_key`）。单测：log allowlist 不含 `sk-` 前缀。
- **FRB 生成物不可手改。**
- **rust_agent 不在 workspace members。**
- **`unwrap`：** 新代码生产路径禁止。
- **scripted `Provider::stream`：** `goose_providers::base::stream_from_single_message`（不在 provider-types）。
- **CompactionOp：** `provider.get_context_limit(&model, override).await`，70% 阈值。

---

## File Change Summary

按 crate / 路径字母序。`--` 后一句话。

- `Cargo.lock` -- PR0：goose alpha.9 校验和
- `crates/kim-agent-host/Cargo.toml` -- PR0 bump；PR6 或 PR3 加 `rmcp` 若 Tool schema 需要
- `crates/kim-agent-host/src/events.rs` -- **新** HostEvent（PR1 变体可先加）；TurnOutcome 在 PR4
- `crates/kim-agent-host/src/lib.rs` -- 改为模块根；AgentHost API 扩展；mention 多 profile
- `crates/kim-agent-host/src/machine.rs` -- **新** MachineFactory
- `crates/kim-agent-host/src/ops/bash.rs` -- **新** Phase 5/9 bash ToolProvider
- `crates/kim-agent-host/src/ops/compaction.rs` -- **新** Phase 7
- `crates/kim-agent-host/src/ops/deferred_kim.rs` -- **新** Phase 4
- `crates/kim-agent-host/src/ops/fs.rs` -- **新** Phase 3
- `crates/kim-agent-host/src/ops/max_turns.rs` -- **新** Phase 3/9
- `crates/kim-agent-host/src/ops/mcp.rs` -- **新** Phase 6
- `crates/kim-agent-host/src/ops/mod.rs` -- **新**
- `crates/kim-agent-host/src/ops/permission.rs` -- **新** Phase 5
- `crates/kim-agent-host/src/ops/system_prompt.rs` -- **新** 从 lib.rs 搬
- `crates/kim-agent-host/src/ops/unknown_tool.rs` -- **新** PR3
- `crates/kim-agent-host/src/ops/chat_guard.rs` -- **新** PR3
- `crates/kim-agent-host/src/profile.rs` -- **新** AgentProfile 及 builtin
- `crates/kim-agent-host/src/provider.rs` -- **新** builders + declarative + SessionKeyResolver
- `crates/kim-agent-host/src/scripted.rs` -- **新** 测试/demo Provider
- `docs/agent-goose.md` -- 更新架构图、两 FFI 约束、profile dest 约定
- `sdk/mobile/flutter_rust_bridge.agent.yaml` -- 不变（仍 `crate::api`）
- `sdk/mobile/hook/build.dart` -- 不变
- `sdk/mobile/lib/agent/capability_host.dart` -- **新** Phase 4
- `sdk/mobile/lib/agent/mention.dart` -- Phase 8 多 dest / 多 mention
- `sdk/mobile/lib/agent_bridge.dart` -- 透传新 FFI：fetchModels / completeTool
- `sdk/mobile/lib/core/paths.dart` -- 不变（目录已存在）；Phase 7 开始真正使用 `agentSessions`
- `sdk/mobile/lib/core/settings.dart` -- 不变（继续提供 secure storage 工厂）
- `sdk/mobile/lib/data/conversation_store.dart` -- PR3：`_msg` 识别 `agentCard`；不需要 FTS
- `sdk/mobile/lib/data/message_repository.dart` -- 不变
- `sdk/mobile/lib/kim_bridge.dart` -- 不变
- `sdk/mobile/lib/l10n/app_en.arb` / `app_zh.arb` -- 设置、确认卡、多 Agent 文案
- `sdk/mobile/lib/models/models.dart` -- `KimMsgKind.agentCard`
- `sdk/mobile/lib/router/app_router.dart` -- `/agent` 列表、`/agent/:id` 编辑器、`/agent/accounts`（catalog 设计 PR5）
- `sdk/mobile/lib/screens/agent/agent_settings_page.dart` -- Phase 2 UI；现为 per-profile 编辑器
- `sdk/mobile/lib/screens/chat/chat_page.dart` -- `isAgentDest`；itemBuilder 已走 KimMessageRow
- `sdk/mobile/lib/screens/home/contacts_page.dart` -- Phase 8 多 ListTile
- `sdk/mobile/lib/screens/home/me_page.dart` -- 进 `/agent` 列表
- `sdk/mobile/lib/src/rust_agent/**` -- FRB 生成（PR1 新字段；PR2/4/5 新方法）
- `sdk/mobile/lib/state/agent_profiles.dart` -- **新** Phase 2
- `sdk/mobile/lib/state/agent_settings.dart` -- 双写；toOpts 新字段
- `sdk/mobile/lib/state/chat_agent.dart` -- 事件、工具、确认、LRU、context
- `sdk/mobile/lib/state/chat_session.dart` -- `isAgent`
- `sdk/mobile/lib/state/contacts.dart` -- withLocalAgents
- `sdk/mobile/lib/state/inbox.dart` -- 多 bot 线程
- `sdk/mobile/lib/state/outbox.dart` -- 拒绝所有 agent dest
- `sdk/mobile/lib/widgets/agent_action_bubble.dart` -- **新**
- `sdk/mobile/lib/widgets/kim_bubble.dart` -- agentCard 分支
- `sdk/mobile/rust_agent/Cargo.lock` -- PR0
- `sdk/mobile/rust_agent/Cargo.toml` -- 不变（仍 path-dep host）；禁止加 kim-client
- `sdk/mobile/rust_agent/src/api/session.rs` -- opts、事件映射、complete_tool、respond_permission、fetch_supported_models
- `sdk/mobile/rust_agent/src/frb_generated.rs` -- 生成
- `sdk/mobile/rust_agent/tests/session_scripted.rs` -- scripted provider；yield/complete 集成
- `sdk/mobile/test/agent_mention_test.dart` -- 多 profile
- `sdk/mobile/test/agent_settings_test.dart` -- 迁移与 thinking
- `sdk/mobile/test/state/chat_agent_test.dart` -- tool/permission 用 fake session
- `sdk/mobile/test/agent/capability_host_test.dart` -- **新**

**不变且本设计不碰：** `crates/kim-client/**`、`sdk/mobile/rust/**`、`services/**`、`sdk/mobile/lib/src/rust/**`。

---

## Alternatives Considered

### A. 合并两条 FFI / host 依赖 kim-client

Agent 直接 `Client::send_message`。少一层 Dart。

- 优点：延迟低、无需 yield。
- 缺点：违反已写进 `docs/agent-goose.md` 的隔离；agent 静态库链上整份 IM 协议、JWT、supervisor；测试与权限面爆炸；移动端两个 native asset 的边界被毁掉。
- **拒绝。**

### B. Dart 只做 UI，所有工具在 Rust，IM 调用走 Unix socket / 内部 RPC

- 优点：机器内同步 ToolProvider。
- 缺点：等于自研第三条 RPC；还是要把好友校验、outbox 幂等、ConversationStore 复制一份。现有 Outbox 是 Dart 状态机（`outbox.dart`）。
- **拒绝**（工作量大于 yield 协议）。

### C. 单全局机器 + 工具开关全靠 prompt 文字

- 优点：改动小。
- 缺点：模型会 hallucinate 未给的工具；无法做到 translator 物理上无 send_message schema。
- **拒绝** 作为终态；Phase 1 暂时等价于此。

### D. Profile 持久化全放 Rust 磁盘

- 优点：host 自洽。
- 缺点：key 在 Rust 落盘或再实现一遍 keychain；Flutter 已有 `flutter_secure_storage` + 迁移故事。跨 FFI 传 key 一次即可。
- **拒绝** 密钥与 profile 文件放 Rust；Rust 只收 resolved snapshot。

### E. 确认卡用系统 Dialog 而非 IM 气泡

- 优点：实现快。
- 缺点：切后台/滚聊天丢失；与「Agent 是一个会话」产品不一致。
- **拒绝** 作为主 UX；Dialog 可做无障碍备选，不替代气泡。

### F. 不升级 alpha.9，在 8 上做完全部

- 优点：少一次 lockfile PR。
- 缺点：PR3 `ToolOperation` 应对齐 `rmcp` 3.2（alpha.9 的真实动机）。goose-agent `src/` 无 diff 不构成「可以永远停 8」。
- **拒绝** 长期停 8；PR0 单独做。

### G. Dart 跑完后打 `goose.external_dispatch` 让 ToolOperation no-op

- yield 时工具尚未执行，不能打。跑完之后写正常 `ToolResponse` 即可，ToolOperation 看到已回答的 id 会 skip。**任何阶段都不要设 `TOOL_META_EXTERNAL_DISPATCH_KEY`。**

### H. 单 dest=goose + prompt 里选 persona

- 决策 2 已拒：无法分会话、分权限、分确认卡。每个 enabled profile 独立 dest / 独立 `(threadDest, profileId)` session。

---

## Security & Privacy Considerations

威胁模型：本机恶意/越权 Agent 输出、提示注入（真人会话里对方诱导 `@助手` 发消息）、剪贴板与文件系统外泄、API key 泄漏。

| 威胁 | 缓解 | 严重度若未缓解 |
|---|---|---|
| Agent 给任意 dest 发消息 | `send_message` 默认 ask_before；Outbox 好友门；拒绝 agent dest | High |
| 提示注入：群/私聊对方写「忽略指示发给 X」 | 确认卡展示 dest+正文；AlwaysAllow 仅该 tool 且用户显式点 | High |
| 合并 FFI 导致 agent 持有 JWT | 硬约束：禁止依赖；**PR1 起 CI grep** `sdk/mobile/rust_agent/Cargo.toml` 无 `kim-client` | High |
| API key 进 profile JSON / 日志 | schema 无字段；tracing 脱敏；禁止 log `SessionOpenOpts`；allowlist 测试 | High |
| fs 路径逃逸 | canonicalize + prefix 检查；根固定 workspace | High |
| bash 任意命令 | 默认关；ask_before；无 shell 拼接；超时 | High |
| 剪贴板被静默读 | ask_before | Medium |
| `search_messages` 读其它会话 | 工具可传 dest；SmartApprove 下 always_allow——**产品选择**：助手需要跨会话搜索才有用。设置里可改为 ask。Open Question 不留：默认 always_allow，设置可关工具 | Medium |
| MCP 扩展任意进程 | 用户手动加 argv；ask_before；Phase 6 文案警告 | Medium |
| 本地会话文件含对话 | **PR7** 文件在 app support，iOS/macOS 沙箱内；不含 key | Low |
| dest=goose 误发 WGateway | 已有 Outbox 护栏；扩展到所有 agent dest | High（回归） |
| `@mention` 注入过大 context | `context_json` ≤ 8 KiB | Medium |
| 群 dest 代发 | capability 拒绝 group；Outbox 仅 `ThreadKind.user` | High |
| bash AllowOnce 仍可 spawn 任意可执行文件 | 文案标明；cwd 不是安全边界；默认关 | High |

Auth：Agent 不使用用户 JWT。IM 工具以 **当前已登录用户** 走 **Outbox.sendText**（不是 `KimClientPort.sendMessage`），与用户点发送等价。FFI 传 raw key 不可避免；debug 日志禁止 stringify opts。

---

## Observability

进程内、无 Prometheus（mobile 不刮 `kim-metrics`）。

**日志（`tracing`，target `kim::agent`）：**

- session_open：`profile_id`, `provider.kind`, `model`（无 key）
- turn start/end：`session_id`, `outcome`, `elapsed_ms`
- yield：`kind`, `tool_name`（无 args）
- tool 结束：`name`, `ok`, `elapsed_ms`
- provider 错误：`HostError` Display，已有 InferenceRunner 会 `tracing::error!("LLM provider error")`

**指标（host 原子计数，snapshot 可选暴露，不强制 FFI）：**

- `agent_turns_total{outcome=finished|yielded|failed|aborted}`
- `agent_tool_calls_total{name,executor=rust|dart,ok}`
- `agent_permission_prompts_total{decision}`

**告警：** 无服务端。客户端：连续 3 次 `failed` 在该会话插一条 sys 气泡「检查 API key / 网络」，不弹全局。

**现有缺口：** `kim-agent-host` 依赖了 `tracing` 但 `lib.rs` 零调用。PR1 在 `prompt` / `build_provider` 补 `info!`/`warn!`（无 key）。

**PR4 debug 面：** `SessionSnapshotDto.phase` = `idle|running|yielded`，`pending_call_ids`。这是唯一需要的运行时检查点。

---

## Rollout Plan

无服务端 flag。客户端用 SharedPreferences：

| Flag / 开关 | 默认 | 作用 |
|---|---|---|
| （无）PR0–PR1 | — | 聊天路径等价（测试 X：一条气泡） |
| 设置页 fs | false | **唯一** fs 开关 → `tools.fs` |
| 设置页 kim 读工具 | false 直到 PR4 稳定 | → `tools.search_*` 等 |
| 设置页 kim 写工具 | false 直到 PR5 | → `send_message` / clipboard |
| `agent.multi_profile` | false | PR8 通讯录展开 |
| ~~`agent.enable_approvals` 把 SmartApprove 当 Auto~~ | **不存在** | 禁止生产调试脚枪 |

权威是 `profile.tools` / `mode` / `permissions`。开关只决定写进这些字段，不在 ChatAgent 丢事件。

**回滚：** PR0 钉回 alpha.8；PR1 空 `profile_json`；设置页关工具 bit；PR8 `multi_profile=false`。旧 `agent.api_key` 保留到 PR8+1 版本。

**PR1 聊天保证：** 未改设置的用户：同一 provider/模型、无工具、同一 `助手` dest、FFI 合成的一条 `assistant_finished`。测试 X 锁这条。FRB 新字段是编译期变更，不是产品聊天变更。

---

## Risks

| Risk | Severity | Mitigation |
|---|---|---|
| yield 后 busy 死锁，Dart 无法 complete_tool | High | PR4 协议：事件在 Yielded 之后；10min Dart timer |
| 忘记 UnknownToolOp / ChatGuard → 无限 loop 烧 token | High | factory 必装 Unknown 或 Chat 模式空 tools；`max_turns` 默认 16（助手） |
| `complete_tool` 与 abort 竞态 | Medium | call_id 未知 → 错；cancel token 在 continue 时检查 |
| FRB 生成物漏提交导致 CI 红 | Medium | 该 PR checklist；codegen 与手改 session.rs 同 PR |
| alpha.9 `rmcp` 3.2 细微 breaking 在 PR3 才爆 | Medium | PR0 编译；PR3 前再 `cargo test` tool_operation 范本 |
| 确认卡 JSON 进 `KimChatMsg.body` 被当普通文本转发 | Medium | `kind=agentCard`；长按复制走空（`chat_chrome.dart:39` 对 sys/image 已空；把 agentCard 列入） |
| 多 session LRU 关 goose 会话丢 HashMap | Low | PR7 持久化后无感；此前文档「进程内」 |
| bash/MCP 被 AlwaysAllow | High | 内置 coder 默认 Approve；AlwaysAllow 不出现在 bash 的确认卡主按钮（只有 Allow once / Deny）；Always 藏在「高级」 |
| 设置页双写 goose 与旧键不一致 | Medium | 单一 `AgentProfileStore.save` 负责双写；ChatAgent PR8 前只读旧 `AgentSettings` 或只读 goose 行（选一，PR2 起 **读 goose 行优先，fallback 旧键**） |

---

## Key Decisions

1. **IM 工具 Dart 执行 + `complete_tool` 续跑**；Rust 执行 fs/bash/MCP；永不合并 FFI。`KimCapabilityHost` 不持有 `AgentSession`。
2. **Profile dest = `agent:<id>`，`goose` 为默认别名**；mention 扩展到 display_name。Live session 键 `(threadDest, profileId)`。
3. **`MachineFactory` 按 profile 出 `Vec<Step>`**。Chat-only = **空 ToolSet**；`GooseMode::Chat` 不得短路非空 tools。
4. **PR0 升 alpha.9**（动机：`rmcp` 3.2）；goose-agent 源码树无 diff。
5. **权限：Yielded 之后 `action_required` → IM 气泡 → `respond_permission`。**
6. **不做早期 OS sandbox**。
7. **Key 只在 Flutter secure storage**；profile JSON 仅 `key_ref`。
8. **OpenAI/Anthropic 原 builder；其余字面-URL `from_json` + SessionKeyResolver**。
9. **PR7 前 HashMap 会话**；`sqlite_path` 今日是 session_id。
10. **PR1–PR3 UI 仍等 FFI 合成的 `assistant_finished`**；host 发 Finished 只在 PR4。
11. **`list_profiles` 只读、Dart-only**；不提供 switch 工具；禁止 host ToolProvider 杂交。
12. **`send_message` 默认 ask_before**；仅 Outbox；拒绝 agent dest 与群 dest。
13. **ChatAgent PR8 改为 `(threadDest, profileId)` LRU**；此前单 session，`close`=abort 协议。
14. **同一时刻 Finished 只由一边发出**：PR1 FFI；PR4 host。
15. **Builtin：goose / translator / coder**。goose 写工具 PR5 才广告。
16. **事件在 `SessionPhase::Yielded` 之后发**；listen replay；`complete_tool` 仅 Yielded。
17. **直到 ChatAgent 送 `profile_json`（PR4），`from_legacy` 按 `enable_fs_tools` / `enable_kim_tools` / `enable_approvals` 填 ToolSet；非空 → SmartApprove。**
18. **一次只 append 一个 complete_tool；pending 空才 `run()`。** 并行 ToolRequest 串行 complete。
19. **`search_messages` 默认 always_allow，允许跨 dest 搜本机全部会话历史，不弹确认卡。**
20. **bash 设置页可见但默认关**（危险开关，默认 off）。
21. **PR7 会话持久化用 JSON 文件** `KimPaths.agentSessions/<id>.json`，不用 SQLite。
22. **iOS 杀进程不自动 resume。** 冷启动丢 in-flight yield；PR7 后 conversation 在，pending 卡变 Retry/Cancel，Dart 不自动 `complete_tool`。

---

## Resolved

原 Open Questions 已由产品拍板，与上文推荐默认一致，不再开放：

1. **`search_messages`：** always_allow 跨 dest。助手可以搜本机全部会话历史，不弹确认卡。
2. **bash UI：** 可见但默认关。设置页有危险开关，默认 off。
3. **PR7 persistence：** JSON 文件（`KimPaths.agentSessions/<id>.json`）。Not SQLite。
4. **iOS kill：** 不自动 resume。冷启动丢 in-flight；PR7 后 conversation 在，pending 卡变 Retry/Cancel，Dart 不自动 `complete_tool`。

---

## PR Plan

每个 PR 可单独 review、单独合并。后面的 PR 依赖前面已在 main 的接口，但可通过 feature flag 关闭行为。

### PR0 — chore(agent): bump goose crates to 0.1.0-alpha.9

- **Files:** `crates/kim-agent-host/Cargo.toml`, workspace `Cargo.lock`, `sdk/mobile/rust_agent/Cargo.lock`, `sdk/mobile/rust_agent/tests/session_scripted.rs`（dummy key + snapshot，不 prompt）
- **Deps:** none
- **Desc:** 无产品行为变化。`cargo test -p kim-agent-host` 与 `cd sdk/mobile/rust_agent && cargo test` **必须绿**。记下 `rmcp` 3.2。

### PR1 — feat(agent): AgentProfile + MachineFactory, legacy chat path

- **Files:** host 拆模块；`SessionOpenOpts` 新字段 + FRB；`toOpts`；`docs/agent-goose.md`；host 单测；CI grep 无 kim-client
- **Deps:** PR0
- **Desc:** **重构，聊天路径等价于测试 X**（一条 FFI 合成的 `assistant_finished` 气泡）。`prompt` 返回类型不变。host 不发 Finished。不修 reconfigure-store。`rg "SessionOpenOpts\\("` 更新所有构造。

### PR2 — feat(agent): settings UI for models, reasoning, declarative providers

- **Files:** `from_json`（仅字面 URL）；FFI `fetch_supported_models` / `list_builtin_profiles`；FRB；`agent_profiles.dart`；设置页；l10n
- **Deps:** PR1
- **Desc:** 编辑 goose。ChatAgent 仍 `toOpts()`（goose 行优先填字段）。无工具。builtin goose JSON **不含** send_message。

### PR3 — feat(agent): in-process ToolOperation (optional fs) + tool bubbles

- **Files:** `ops/{fs,unknown_tool,chat_guard,max_turns}.rs`；`scripted.rs`；kind 全链路；`agent_action_bubble.dart`；设置页 fs
- **Deps:** PR1。设置页 fs 开关视觉依赖 PR2，但 host 接线只靠 `enable_fs_tools`。
- **Desc:** 设置页 fs 默认关。`from_legacy` 置 `tools.fs` → SmartApprove。ChatGuard 本 PR。无 `complete_tool`。

### PR4 — feat(agent): Dart KimCapabilityHost + complete_tool

- **Files:** `deferred_kim.rs`；TurnOutcome；FFI phase/replay/`complete_tool`/`snapshot.phase`；FRB；`capability_host.dart`；ChatAgent 送 `profile_json` + context；host 发 Finished、FFI 停合成
- **Deps:** PR3（kinds）**且 PR2**（`AgentProfileStore` / `profile_json`）
- **Desc:** 读 IM 工具。**不**广告 `send_message` / `read_clipboard`。协议一页纸 + 双 ToolRequest 脚本测试。

### PR5 — feat(agent): PermissionOp + confirmation bubbles

- **Files:** `ops/permission.rs`；FFI `respond_permission`；FRB；气泡按钮；profile.permissions；goose 模板翻写工具
- **Deps:** PR4
- **Desc:** 广告 `send_message` / clipboard。无「SmartApprove 当 Auto」pref。

### PR6 — feat(agent): MCP extensions via ToolProvider

- **Files:** `ops/mcp.rs`；profile `extensions`；设置页高级列表；host Cargo.toml `rmcp` client 特征
- **Deps:** **PR5**（MCP 工具必须走确认卡；禁止 Auto 执行）
- **Desc:** 默认无扩展。

### PR7 — feat(agent): persist conversations + compaction

- **Files:** host store 文件 I/O；`SessionOpenOpts.session_id`；**本 PR 的 ChatAgent** 才把 `sqlitePath` 换成真路径（旧客户端：path 无 `/` 仍当 session_id）；`ops/compaction.rs`（`get_context_limit(...).await`）；修「SQLite preserved」注释
- **Deps:** PR1。可与 PR3–PR5 并行，但 `SessionOpenOpts.session_id` 与 ChatAgent `_ensureSession` 必须在本 PR 一起改，避免半套。
- **Desc:** 冷启动可续聊。compaction 默认关。AllowOnce 从 JSON conversation 重建。

### PR8 — feat(agent): multi-agent contacts and @mentions

- **Files:** `mention.dart`；contacts/inbox/outbox/chat_session/chat_page/contacts_page；ChatAgent `(threadDest, profileId)` LRU；sender=display_name；`isAgentDest` 好友门；设置页 CRUD
- **Deps:** PR2；建议 PR5
- **Desc:** `agent.multi_profile`。群 @mention = 本机 echo。

### PR9 — feat(agent): optional steer + subagent (optional)

- **Files:** `ops/steer.rs`、`ops/subagent.rs`；coder/助手可选 `max_turns`；bash provider 若未在 PR5 做完则本 PR 或独立 PR5b
- **Deps:** PR3, PR5
- **Desc:** 可砍。bash 更适合作为 **PR5b**（高风险，独立 review），不要和 subagent 捆。

**建议的 PR5b — feat(agent): optional bash tool**

- **Files:** `ops/bash.rs`；设置页危险开关；permission 强制 AskBefore
- **Deps:** PR5
- **Desc:** 默认关。独立安全 review。

---

## References

- `docs/agent-goose.md` — 两 FFI 隔离与产品路径
- `crates/kim-agent-host/src/lib.rs` — 当前 470 行 host
- `goose-agent` 0.1.0-alpha.8/9 README — Operation / ToolOperation / yield 语义
- `goose-agent-.../tests/tool_operation.rs` — 注册 SyncTool/AsyncTool/ToolProvider 的范本
- `goose-provider-types` `conversation/message.rs` — ActionRequired / dual visibility
- `goose-providers` `declarative.rs` — `from_json` / 46 bundled JSON
- unpublished 参考（只读，不依赖）：block/goose Agent 管道 EntryHook → … → Inference（用户研究笔记）；PR #11627 ToolOperation；PR #11685 tool approval in state machine
- `sdk/mobile/flutter_rust_bridge.agent.yaml` — Agent FRB
- `docs/production-gaps.md` H4 — unwrap_used 仍为待办，本设计新代码仍当 deny 执行

---

## 实现时对照清单（工程师可撕）

1. 不在 `sdk/mobile/rust_agent/Cargo.toml` 加 `kim-client`。
2. 不加 freezed `AgentUiEvent`。
3. 不把 key 写入 `agent.profiles`。
4. 不把 `dest=goose` / `agent:*` 送进 Outbox。
5. 生产路径无 `unwrap`/`expect`。
6. 每个新 FFI 方法同 PR 提交 codegen。
7. PR1 合入后：对助手说一句话，只出现 **一条** 回复（FFI 合成 Finished）。
8. `send_message` 无确认卡不上线（PR4 读工具 / PR5 写工具）。
9. bash / MCP 默认关。
10. 自实现 Operation，git 不依赖 `block/goose`。
11. PR1 起 CI grep `kim_agent_ffi` 无 `kim-client`。
12. `rg "SessionOpenOpts\\(" sdk/mobile` 与 FRB 同 PR。
13. `complete_tool` 事件必须在 `snapshot.phase==yielded` 之后。
14. 不设 `goose.external_dispatch`。
15. 不把 SmartApprove 当 Auto 的生产 pref。
