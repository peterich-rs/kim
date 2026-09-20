# Codex Agent Harness 嵌入（替换 Goose 运行时）

| Field | Value |
|---|---|
| Author | KIM Agent Working Group |
| Date | 2026-09-19 |
| Status | Draft |
| Audience | 实现 `kim-agent-host` + `sdk/mobile/rust_agent` 的资深工程师 |
| Codex pin | 本地检出 `/Users/zhangfan/develop/github.com/codex`，`origin` = `https://github.com/openai/codex.git`，**stable tag `rust-v0.155.1`**（`be2951ea3`，2026-09-18）。本文行号与核对项以该 pin 为准。比它新的 `0.156.0-alpha.*` 不跟。 |
| 仓库 | `/Users/zhangfan/develop/github.com/im` |
| 扩展（不替换产品模型） | [agent-config-as-data.md](./agent-config-as-data.md)（A-KD）、[agent-provider-persona.md](./agent-provider-persona.md)（P-KD）、[agent-capability-blocks.md](./agent-capability-blocks.md)（B-KD）、[agent-productivity.md](./agent-productivity.md)（S-KD）、[human-agent-im-parity.md](./human-agent-im-parity.md)（HA-KD） |
| 取代的运行时 | [goose-agent-harness.md](./goose-agent-harness.md)（H-KD）里「自实现 CompactionOp / 自实现推理环」不再是目标。H-KD 的 idle / hard / yield / 可见性仍留在适配层。Goose crate 在切流完成前不删。 |
| 编号 | 本文决策写作 **CX-KD *n***。 |

---

## Overview

Goose `0.1.0-alpha.9` 没有已发布的 conversation summary。KIM 自写的 `CompactionOp` 用 `chars/4` 估 token、只压一轮、摘要是截断片段（`crates/kim-agent-host/src/ops/compaction.rs`）。上下文一长就顶死。这是换运行时的原因，不是再补一层 Operation。

Codex 的压缩、多 profile、外部 provider、技能、MCP、审批、rollout 都在 `codex-core` 里，并且已经有库入口：`codex-core-api` 的 `ThreadManager`。官方样例是 `codex-rs/thread-manager-sample`，它明确写了「只依赖 `codex-core-api`，新表面加到 core-api，不要再依赖别的 codex crate」。

做法：**不把 CLI / TUI 当产品**；**不删 Codex 里用不到的 crate**；**不把 KIM workspace 和 Codex workspace 并成一个**。`AgentProfile` 仍是产品配置（A-KD）。会话打开时把它投影成一份**内存里的** Codex `Config`（`ConfigOverrides` + 点分覆盖），再交给 `ThreadManager`。磁盘上只共用一个 app 内的 `codex_home`（rollout / state db / helper 路径），**不**按 agent 拆 home。密钥走 API key，不走 ChatGPT OAuth。

---

## Feasibility Assessment

**结论：Feasible with caveats。** 库嵌入、配置注入、API key、本地 compaction 都有已核对的公开表面。不能承诺的是「任意 Goose Chat Completions 厂商原样能打」。

已核对、可直接依赖：

1. **库入口是 `ThreadManager`，不是 CLI。** `codex-rs/core-api/src/lib.rs` 再导出 `ThreadManager`、`CodexThread`、`Config`、`AuthManager`、`StartThreadOptions`、`EventMsg`、`DynamicToolSpec`。样例 `codex-rs/thread-manager-sample/src/main.rs` 用 `ThreadManager::new` → `start_thread` → `start_turn_if_idle` → `next_event` 跑完一轮。
2. **配置加载器就是注入点。** 正确入口是 `ConfigBuilder`（`codex-rs/core/src/config/mod.rs`）：`.codex_home(shared)` + `.cli_overrides(...)` + `.harness_overrides(ConfigOverrides { ... })` + `.loader_overrides(LoaderOverrides { ignore_user_config: true, ignore_project_config: true, ... })`。注意：`Config::load_default_with_cli_overrides_for_codex_home` **内部写死** `ConfigOverrides::default()`，装不进 `cwd` / `developer_instructions` / `codex_self_exe`，**不要单独用它当注入路径**。`ConfigOverrides` 有 `cwd`、`developer_instructions`、`base_instructions`、`compact_prompt`、`workspace_roots`、`approval_policy`、`codex_self_exe`。不要手写样例里那份上百字段的 `Config { ... }`。
3. **多 profile 是 `config.toml` 一等字段。** `ConfigToml.profile` + `profiles: HashMap<String, ConfigProfile>`（`codex-rs/config/src/config_toml.rs`）。`ConfigProfile` 覆盖 model、`model_provider`、`approval_policy`、`model_reasoning_effort`、sandbox（`codex-rs/config/src/profile_toml.rs`）。人设、窗口、压缩阈值、MCP、skills 不在 profile 里，在根配置或 `ConfigOverrides`。
4. **外部模型是 `model_providers`。** `ModelProviderInfo` 有 `base_url`、`env_key`、`experimental_bearer_token`（注释写明：程序化嵌入用这个，不要为了方便把 key 写进文件）、`requires_openai_auth`（`codex-rs/model-provider-info/src/lib.rs`）。`requires_openai_auth = false` 时不走登录屏，key 来自 `env_key` 或 bearer。
5. **API key 是正式登录模式，OAuth 是另一条。** `AuthMode::ApiKey` + `login_with_api_key` 写 `auth.json` 的 `openai_api_key`，不写 `tokens`（`codex-rs/login/src/auth/manager.rs` `login_with_api_key`）。`ForcedLoginMethod::{Chatgpt, Api}`（`codex-rs/protocol/src/config_types.rs`）。`forced_login_method = api` 时若磁盘上是 ChatGPT 登录，Codex 会登出。`AuthManager::shared_from_config(config, enable_codex_api_key_env)` 的第二个参数我们传 `false`，避免吸进进程环境里的 `OPENAI_API_KEY`。
6. **压缩在 turn 里，不在 TUI 里。** 自动路径 `run_auto_compact`（`codex-rs/core/src/session/turn.rs`）看 `model_auto_compact_token_limit` 与模型目录的 `auto_compact_token_limit`。手动路径 `Op::Compact` → `CompactTask`（`codex-rs/core/src/session/handlers.rs`、`codex-rs/core/src/tasks/compact.rs`）。远端压缩 V2 只给 OpenAI、Azure Responses URL、Bedrock（`codex-rs/model-provider/src/provider.rs` `configured_provider_remote_compaction_matches_provider_support`）。其它 provider 是 `RemoteCompactionSupport::Unsupported`，落到本地 `compact::run_compact_task`，提示词是 `compact_prompt` 或 `SUMMARIZATION_PROMPT`。事件是 `EventMsg::ContextCompacted`。
7. **IM 工具有一等让出。** `StartThreadOptions.dynamic_tools`（`codex-rs/core/src/thread_manager.rs`）。模型调用后发 `EventMsg::DynamicToolCallRequest`；宿主回 `Op::DynamicToolResponse`（`handlers.rs` `dynamic_tool_response`）。这就是今天 `SessionPhase::Yielded` 的对应物。
8. **Steer / 审批 / MCP 刷新已经是 Op。** `TurnInputRequest` 可 start-or-steer（`codex-rs/protocol/src/turn_input.rs`）。审批是 `EventMsg::ExecApprovalRequest`。`Op::RefreshMcpServers`、`Op::ReloadUserConfig` 已存在。
9. **桌面门已经在。** `sdk/mobile/hook/build.dart` `_desktopAgentHost` 只在 macOS / Windows / Linux 编 `rust_agent`。手机不编 agent host。这条不变。

Caveats（必须写进实现，不是口头说明）：

| Caveat | 证据 | 处理 |
|---|---|---|
| `wire_api = "chat"` 已删除 | `WireApi` 只剩 `Responses`；反序列化 `"chat"` 报 `CHAT_WIRE_API_REMOVED_ERROR`（`model-provider-info/src/lib.rs`） | 注入器拒绝只讲 Chat Completions 的厂商，返回类型化错误。不在 KIM 里写 chat→responses 代理。Goose 路径留到这些厂商有 Responses 端点，或用户改 `base_url` |
| 远端压缩不是「任意 base_url」 | 自定义 `https://example.test/v1` 测下来是 `Unsupported`；OpenAI / Azure / Bedrock 才是 V2 | 自定义端点用本地摘要压缩。这仍然比 `CompactionOp` 深：真 token、可多次、有 `ContextCompacted`。不要对自定义厂商承诺 remote compact |
| `ThreadManager` 构造需要 `codex_self_exe` | `ExecServerRuntimePaths::from_optional_paths` 在路径为空时返回 `Codex executable path is not configured`（`exec-server/src/runtime_paths.rs`） | 桌面另编 **`kim-codex-helper`**。它不是 `codex` CLI。core 用绝对路径再 exec，argv 命中 helper 模式才干活，否则退出 |
| 两个 workspace 不能合并 | KIM `edition = "2021"`、`unsafe_code = "deny"`（根 `Cargo.toml`）。Codex `edition = "2024"`（`codex-rs/Cargo.toml`） | path 依赖，Codex 保持自己的 workspace。KIM lint 不套到 Codex 源码上 |
| `AuthManager` / 密钥与共享 `codex_home` | 共享 home 时磁盘 `auth.json` 只有一份；多 agent 不同 key 会互相覆盖 | **配置按会话隔离，密钥不落共享文件。** 官方 OpenAI：`CodexAuth::from_api_key` / `AuthCredentialsStoreMode::Ephemeral`。自定义：`experimental_bearer_token` 只在本次 `Config` 内存里。一个打开中的 dest×profile 一个 `AuthManager`+`ThreadManager` |
| 崩溃域与 Goose 相同 | 进程内嵌入，panic 与 `kim_agent_ffi` 同命运 | 不假装比 ACP 子进程更隔离。毒化 = drop manager + 按 rollout 重建 |
| 样例 `Config { ... }` 字段极宽 | `thread-manager-sample/src/main.rs` `new_config` | 禁止抄这份结构体字面量。只用 `ConfigBuilder` |
| `StartThreadOptions` 字段比旧 pin 多 | `0.155.1` 多了 `inherited_environments` / `user_instructions` / `disabled_plugin_ids` | 用 `StartThreadOptions::new(config)` 再改 `dynamic_tools`；不要手填整结构体 |
| pin 会漂移 | 本地钉 `rust-v0.155.1` | 升级 pin 必须重跑：Responses-only、`ConfigBuilder`+`LoaderOverrides`、`RemoteCompactionSupport` 分档 |

**Fully feasible：** PR0–PR2（钉版本、编译、配置投影、API key、不联网）。

**Feasible with caveats：** PR3 起的线程、动态工具、压缩、权限。阻塞项是 Responses 线协议和 helper 二进制，不是「Codex 能不能嵌」。

---

## Goals & Non-Goals

### Goals

- `kim-agent-host` 的运行时从 `goose-agent` `StateMachine` 换成 `codex_core_api::ThreadManager`。对外仍是 `prompt` / `complete_tool` / `respond_permission` / `abort` / `steer`。
- `AgentProfile`（以及以后的 `AgentSpec`）是唯一产品配置。Codex `config.toml` 是打开会话时的投影，不是用户编辑的第二真相，也不上云。
- 官方 OpenAI 与自定义 Responses provider 都用 API key。`forced_login_method = "api"`。不实现、不调用 ChatGPT OAuth。
- 压缩交给 Codex：`model_context_window` + `model_auto_compact_token_limit` 从 `ModelSpec.context_tokens` 注入。删掉热路径上的 `CompactionOp`（Goose 代码文件先留着，flag 默认切过去之后再删）。
- IM 工具继续让给 Dart。注册 `dynamic_tools` 命名空间 `kim`，`DynamicToolCallRequest` ↔ 今天的 yield。
- 不读、不写用户真实的 `~/.codex`。进程内共用一个 app 目录下的 `codex_home`；每个会话的 `Config` / `ConfigOverrides` / 密钥彼此隔离。
- 两条 FFI 仍分开。手机仍不编 agent host。

### Non-Goals

- 不把 `codex-tui` / `codex-cli` 的交互界面链进 App。不提供 `codex` REPL。
- 不删 Codex 树里未使用的 crate（TUI、voice、realtime、cloud-tasks 都留在 submodule 里，只是 KIM 不依赖它们）。
- 不把 Codex workspace 成员并进 KIM `Cargo.toml`。
- 不做 Chat Completions 适配层。
- 不做 ChatGPT / Codex OAuth、device code、`AuthMode::Chatgpt`。
- 不改 WGateway、不改 `chat.bot.*`、不合并 `kim_agent_ffi` 与 `kim_client_ffi`。
- 不在本切片重做 A-KD 的 protobuf 存储。投影读今天的 `AgentProfile` JSON；`from_spec` 落地后只换读入口。
- 不在本切片做手机运行时。
- 不在 PR0–PR7 删除 `goose-agent` 依赖。切流 flag 默认仍是 goose，直到压缩与 IM 工具在桌面走通。

---

## 决策

**CX-KD 1 — Codex 以 git submodule 进仓库，path 依赖，不并 workspace。**

- 路径：`third_party/codex`，上游 `https://github.com/openai/codex.git`，pin **`rust-v0.155.1`**（`be2951ea3`）。
- `crates/kim-agent-host` 只依赖 `codex-core-api`：

```toml
codex-core-api = { path = "../../third_party/codex/codex-rs/core-api" }
```

- 不把 `codex-rs/cli`、`codex-rs/tui` 写进 KIM 的 `members`。Cargo 不会因为 submodule 在磁盘上就编译它们。
- 不复制 crate 进 `crates/`。升级 = 移动 submodule pin + 重跑 CX-KD 核对项。
- `rustc 1.95` 支持 edition 2024（1.85 起）。KIM 自己的 crate 继续 edition 2021。

**CX-KD 2 — 产品入口是 `ThreadManager`，对照 `thread-manager-sample`，不对照 `codex app-server`，不对照 TUI。**

每个打开的 dest×profile：

```text
共享 codex_home = {KimPaths.support}/agent/codex
        │  只放 state db / rollout / helper 路径约定；不写用户 ~/.codex
        │
AgentProfile + KeyVault
        │  ConfigInjector（每会话一份，不落盘）
        ▼
cli overrides（内存） + ConfigOverrides（cwd / 人设 / workspace / approval / self_exe）
        │  ConfigBuilder::default()
        │    .codex_home(shared)
        │    .cli_overrides(...)
        │    .harness_overrides(...)
        │    .loader_overrides(ignore_user_config + ignore_project_config)
        │    .build()
        ▼
AuthManager（每会话；密钥 ephemeral / bearer，不写共享 auth.json）
ThreadManager::new(...)
        │  start_thread(StartThreadOptions { dynamic_tools, .. })
        ▼
CodexThread::{start_turn_if_idle, next_event, shutdown_and_wait}
        │  EventMap
        ▼
现有 HostEvent / SessionPhase::Yielded / kim-sdk 队列
```

`ThreadManager` 的其余构造参数（`EnvironmentManager`、`thread_store_from_config`、`init_state_db`、`ExtensionRegistryBuilder`）按样例接，但：

- `analytics_events_client = None`，`analytics_enabled = false`。
- `install_image_generation_extension` 先不装。KIM 没有这条产品。
- `SessionSource::Exec`（库嵌入，不是 TUI）。
- `ephemeral` 由注入器按「要不要 rollout」决定，见 CX-KD 11。不要无脑抄样例的 `ephemeral: true`，否则压缩历史无处可 resume。
- **进程可以共享一个 `ThreadManager` 工厂所需的 `EnvironmentManager` / state db 句柄**（都挂在同一 `codex_home` 上），但每个打开会话仍持有自己的 `Config` 快照与 `AuthManager`。不要进程级单例 `Config`。

**CX-KD 3 — 共享 `codex_home`，隔离 `Config`。AgentProfile 不改成 Codex 的配置文件格式。**

`ConfigInjector` 放在 `crates/kim-agent-host/src/codex_inject.rs`（名字实施时可以改，职责不能拆进 Dart）。

**磁盘（共用一份）：**

| 路径 | 内容 |
|---|---|
| `{KimPaths.support}/agent/codex` | 唯一 `codex_home`。与现有 `agent/workspaces`、`agent/sessions` 并列，不碰 `~/.codex` |
| 其下的 state db / sessions rollout / log | Codex 自己写。thread 用 `ThreadId` 区分，**不**按 profile 再拆 home |
| `codex_self_exe`（helper） | 安装到 app 包或 support 下固定路径；所有会话的 `ConfigOverrides.codex_self_exe` 指向同一文件 |

**内存（每个打开的 dest×profile 一份，禁止写成共享 `config.toml` 当真值）：**

| 产出 | 用哪个 API | 内容 |
|---|---|---|
| 点分覆盖 | `ConfigBuilder.cli_overrides` | 本次会话的 `profile` / `model_providers.*` / `profiles.*` / `skills.*` / `mcp_servers.*` / 窗口与压缩阈值 / `forced_login_method`。bearer 只在这里，**不**写进共享 home 的 toml |
| harness 覆盖 | `ConfigBuilder.harness_overrides`（`ConfigOverrides`） | `cwd`、`workspace_roots`、`developer_instructions`、`compact_prompt`、`approval_policy`、`permission_profile`、`codex_self_exe` |
| 层过滤 | `ConfigBuilder.loader_overrides` | `ignore_user_config = true`、`ignore_project_config = true`（以及忽略 managed/system 登录要求，避免吸到机器上别的 Codex 安装）。共享 home 里即使以后落了文件，也不得成为「当前 agent」的权威 |

选中的 Codex profile 名就是这个 agent id（只存在于本次 overrides，不是全局 active profile 文件）：

```toml
profile = "<agent_id>"

[profiles.<agent_id>]
model = "<ModelSpec.name>"
model_provider = "<provider id>"
model_reasoning_effort = "<mapped>"
approval_policy = "<mapped>"
```

`ConfigProfile` 装不下的字段放本次 cli 覆盖的根上：`model_context_window`、`model_auto_compact_token_limit`、`model_auto_compact_token_limit_scope = "total"`、`forced_login_method = "api"`、`mcp_servers`、`skills`。

人设进 `ConfigOverrides.developer_instructions`（system prompt + steer）。**不要**把人设写进 `base_instructions`。`base_instructions` 是模型协议提示；换掉它会拆掉 Codex 自己的工具说明和压缩约定。`display_name` / `aliases` / IM dest 仍是 KIM 身份，不进 Codex。

**禁止：** 为了省事把多 agent 的 profile 合并写进共享 home 的一份 `config.toml`，再靠切换 `profile =` 字段当运行时真相。运行时真相永远是这次 `ConfigBuilder.build()` 得到的 `Config` 值。

**CX-KD 4 — 字段映射（注入器的合同）。**

| AgentProfile | 注入到 | 说明 |
|---|---|---|
| `id` | 本次 overrides 的 `profile` 与 `profiles.<id>` | 一个 agent 一次投影；不往共享 home 写全局 toml |
| `provider.kind` + `base_url` | 本次 `model_providers.<id>` | `name` 用 kind；`base_url` 用账号上的 URL；`wire_api = "responses"`。kind 为官方 `openai` 且无自定义 base_url 时用内置 `OPENAI_PROVIDER_ID`，不重复定义保留 id（`validate_model_providers` 会拒保留 id） |
| `provider.key_ref` | 见 CX-KD 5 | 值从 KeyVault 来，不在 profile JSON 里 |
| `model.name` | `profiles.<id>.model` | |
| `reasoning` / `thinking_effort` | `profiles.<id>.model_reasoning_effort` | 对不上的枚举在注入时失败，不静默丢 |
| `model.context_tokens` | `model_context_window` | 缺省沿用现目录默认，不发明新数字 |
| `model.context_tokens` | `model_auto_compact_token_limit` | **窗口的 70%**。与今天 `CompactionOp` 的 70% 阈值对齐，避免切流后手感突变。scope = `Total` |
| `model.temperature` / `max_tokens` / `extra_params` | 不进 Codex 根配置，除非 pin 上有对应键 | 没有的键不要塞进 `deny_unknown_fields` 的 toml。PR1 列一张「丢弃并打 debug 日志」的字段表 |
| `system_prompt` | `developer_instructions` | 空则用现 `DEFAULT_IDENTITY_PROMPT` |
| `steer` | 拼进 `developer_instructions` 尾部，单独一节 | 中途 steer 走 `TurnInput` steer，不重写这份静态提示 |
| `mode` | `approval_policy` + permission profile | 见 CX-KD 8 |
| `sandbox` / `workspace` | `ConfigOverrides.cwd` + `workspace_roots` | sandbox kind → 应用沙箱目录；repo kind → 已有 bookmark 解析出的绝对路径。路径仍不进会同步的 Spec（A-KD 12） |
| `capabilities` 里的 IM 工具 | `StartThreadOptions.dynamic_tools` 命名空间 `kim` | 见 CX-KD 7。不做成 MCP |
| `capabilities` 里的 fs / bash | 不注册动态工具 | 交给 Codex 自带 shell / apply_patch，由审批档控制。没有这些 capability 时用只读 profile，并在 developer 提示里写明没有写权限 |
| `extensions` | `mcp_servers.<name>` | 只映射今天已经有的 command/url。OAuth MCP 不配置（`mcp_oauth_*` 保持默认，不填 callback） |
| `skills` / `portable_denylist` | `skills.config[]` | denylist 写成 `enabled = false` 的 name/path 规则（`SkillsConfig` / `SkillConfig`）。`include_instructions` 在 skills 非空时为 true |
| `user_agents_skills` | 本次 skills 扫描根 / 规则 | 空字符串仍表示关闭，host 不猜测 `$HOME`（S-KD 23）。路径可指向共享 support 下的用户技能目录，但启用列表仍按 agent 进本次 overrides |
| `max_turns` | 留在 KIM 适配层计数 | Codex 配置无此字段。到顶 abort，`stop_reason` 用现有稳定字符串，不新造扫错误文本的协议 |
| `harness` 超时 | 留在 KIM，包住 `next_event` | 不翻译进 toml。Keepalive 仍不复位 idle（H-KD） |
| `permissions` / `permission_rules` | IM 确认卡仍走动态工具 yield | 不翻译成 Codex execpolicy。fs/bash 的确认才用 `ExecApprovalRequest` |

**CX-KD 5 — 鉴权只用 API key。共享 home 下密钥不落盘。OAuth 代码留在 submodule 里，KIM 不调用。**

两条键，都从现有 KeyVault 取，不从环境变量取，**也不写入共享 `codex_home/auth.json`**（多 agent 不同 key 会互相覆盖）：

1. **官方 OpenAI**（内置 provider，`requires_openai_auth = true`）：用 `CodexAuth::from_api_key` / `AuthManager` 的 ephemeral 路径持有本次 key；`forced_login_method = "api"`；`enable_codex_api_key_env = false`。禁止 `AuthMode::Chatgpt`。不要调用会把 key 持久化进共享 home 的 `login_with_api_key(..., File, ...)`。
2. **自定义 Responses provider**（`requires_openai_auth = false`）：key 放进本次 `model_providers.<id>.experimental_bearer_token`。这是 `ModelProviderInfo` 文档写明的程序化路径。不写 toml，不写 auth.json。

多 agent 不同 key：共享 `codex_home`，**不同** `AuthManager`（跟会话同寿命）。关会话时 drop manager。日志禁止打印 key（现有 host 规则不变）。

不读取 `OPENAI_API_KEY`。不实现 `ExternalAuth` refresh。Bedrock API key 本切片不接；目录里如果出现 Bedrock，注入器返回未支持，直到单独切片。那不是 OAuth，但也不是今天的 KeyVault 形状。

**CX-KD 6 — 压缩完全用 Codex，KIM 不再摘要。**

- 阈值：`model_auto_compact_token_limit = context_tokens * 70 / 100`。`context_tokens` 缺失时不设 KIM 侧猜测值，让模型目录的 `auto_compact_token_limit()` 生效（`session/context_window.rs` 已有 `or_else`）。
- 官方 / Azure / Bedrock：`RemoteCompactionSupport::V2`。不要关 `Feature::RemoteCompactionV2`，除非该 feature 默认关闭且我们显式打开后有测试。PR5 先读 feature 默认值再决定是否 `features.set`。
- 其它 Responses 端点：本地 `run_compact_task`。`compact_prompt` 用短的 KIM 附加句（保留「用户意图、决定、未完成工具」），没有就用 Codex 的 `SUMMARIZATION_PROMPT`。
- 适配层把 `EventMsg::ContextCompacted` 映射成现有 usage / 系统事件，供 UI 显示「已压缩」。不再发 KIM 自己的 `kim.compaction.v1` 标记。
- `CompactionOp` 在 `runtime=codex` 时不装进机器。文件保留到 goose flag 删除切片。

这是本迁移的验收点：长会话必须出现 Codex 的压缩检查点（rollout 里的 compaction item，或 `ContextCompacted`），而不是 `CompactionOp` 的 80 字片段。

**CX-KD 7 — IM 工具 = 动态工具，不是 MCP，也不是 Codex 内置工具。**

`StartThreadOptions.dynamic_tools` 放一个 namespace：

- name: `kim`
- tools: 由 `capabilities` 投影，名字保持今天的 `send_message` / `search_contacts` / `search_messages` / `get_conversation_context` / `read_clipboard` / `list_profiles`
- schema: 与现在交给模型的 JSON schema 相同

事件：

- `DynamicToolCallRequest` → `HostEvent::ToolRequest` 或 `ActionRequired`（需要确认的工具走后者），FFI 仍 `Yielded`。
- Dart `complete_tool` → 适配层提交 `Op::DynamicToolResponse`。
- 禁止在 Codex 进程里调 `kim-client`。两条 FFI 的隔离不变。

fs / bash / MCP 不走这条 yield。它们在 Codex 内部跑完，适配层只把 `ExecCommandBegin` / `McpToolCallBegin` 转成 UI 事件。

**CX-KD 8 — 审批分两套，不要揉成一个模式枚举。**

| GooseMode | Codex 侧（fs/bash） | KIM 侧（IM 工具） |
|---|---|---|
| `Chat` | `AskForApproval::Never` + `PermissionProfile::read_only()`。developer 提示声明没有 shell 写权限 | 确认卡逻辑不变 |
| `SmartApprove` / `Approve` | `AskForApproval::OnRequest`。写工作区用 Codex 对应的 workspace-write profile；只读 sandbox 用 `read_only()` | 确认卡逻辑不变 |
| `Auto` | `AskForApproval::Never` + 可写 profile | 仍按 `permission_rules` 决定要不要确认。Auto 不表示 IM 代发免确认 |

`ExecApprovalRequest` → 现有 `respond_permission`。不要为了省事把 IM 确认也改成 exec approval。

**CX-KD 9 — 沙箱 helper 是我们自己的 bin，不是 submodule 里的 `codex`。**

`EnvironmentManager::from_codex_home` 在默认 `include_local = true` 时要求 `ExecServerRuntimePaths`。路径为空，`from_optional_paths` 直接失败。exe 不是 agent 循环：`process_sandbox.rs` 把命令改写成 `codex_self_exe --codex-run-as-arg0-exec-helper …`，`fs_sandbox.rs` 拉起同一个文件加 `--codex-run-as-fs-helper`。macOS seatbelt / Linux bwrap 作用在子进程上，不能收进 Flutter 进程。

不编 `codex-cli --bin codex`。那个包带 TUI 和子命令。桌面只编 `kim-codex-helper`（见下节）。`Config.codex_self_exe` 指向它的绝对路径。

约束：

- App 不提供打开该二进制的 UI，不放进 `PATH`，不叫 `codex`。
- KIM / Flutter 进程不调用 `arg0_dispatch` 或 `arg0_dispatch_or_else`。只有 helper 进程在被再 exec 时调用。
- Linux 额外放一个名为 `codex-linux-sandbox` 的 symlink，指向同一文件，写入 `codex_linux_sandbox_exe`。macOS / Windows 该字段为 `None`。
- 构建进 `sdk/mobile/hook/build.dart` 的桌面分支。手机分支不编，手机也不起带本地 shell 的 Codex 线程。

**CX-KD 10 — 事件映射保持现有 FFI。不把 `EventMsg` 泄漏进 Dart。**

| Codex | 现有 HostEvent / 结局 |
|---|---|
| 助手正文增量 | `TextDelta`。优先 `EventMsg::AgentMessageContentDelta`；没有 delta 时再吃 `AgentMessage` 并在适配层切块。不把 `item_event_to_server_notification` 滤掉的内部事件往上抛 |
| `AgentReasoning` | 丢弃，除非设置里已经有「显示推理」。默认 `hide_agent_reasoning = true` |
| `TokenCount` | `Usage`。仍不复位 idle |
| `DynamicToolCallRequest` | yield |
| `ExecApprovalRequest` | `ActionRequired` |
| `ExecCommandBegin` / `McpToolCallBegin` | `ToolRequest`（已在 Codex 内执行，Dart 不接） |
| 对应 End | `ToolResult` |
| `ContextCompacted` | UI 标记，不当成一条聊天正文 |
| `TurnComplete` | `TurnOutcome::Finished`，`replied` / `visible` 算法不变 |
| `Error` | `HostError`，禁止把正文塞进空 Finished |
| 流在 idle/hard 内无上述活动 | `TimedOut`。时钟在适配层，不要求 Codex 提供同名超时 |

`stop_reason` 字符串保持 `events.rs` 里已有的 `completed` / `side_effect` / `empty`。

**CX-KD 11 — 转录权威在 Codex rollout，不在 `SessionDisk` JSON。**

- `ephemeral = false`。rollout 写在**共享** `codex_home` 下，由 Codex 用 `ThreadId` 分文件/分库条目。
- 打开已有 dest×profile：用 KIM 侧存的 thread id 调 `resume_thread_*`。thread id 存在现有 session 键旁边（与 `KimPaths.agentSessionFile(dest, profileId)` 同级元数据即可），不写进 `AgentProfile`。
- IM 气泡仍由 kim-sdk / `chat.bot.reply` 持久化。rollout 是模型上下文，不是聊天气泡的第二副本。
- goose 的 `SessionDisk` 不迁移进 rollout。切流后的会话是新线程。旧 JSON 只读到 flag 关掉为止，不做自动摘要导入。
- 卸载整个 app 数据时才清共享 `codex_home`。卸载单个 agent 只删该 agent 的 thread 记录与 KIM 元数据，不删整个 home。

**CX-KD 12 — 切流 flag，Goose 留到最后。**

- `SessionOpenOpts` 增加 `runtime = goose | codex`，默认 `goose`。
- `codex` 时 `AgentHost::from_resolved` 走注入器 + `ThreadManager`。`goose` 时现有 `MachineFactory` 原样。
- 不在同一会话里混跑。切 flag = 新会话。
- 删除 goose 依赖、删除 `CompactionOp`、删除未使用的 Codex 源码，都不在本规划的 PR 里。

**CX-KD 13 — 本规划不改 A-KD 的存储形状。**

投影的输入是今天 `resolved_from_opts` 已经拿到的 `AgentProfile` + api key。A-KD 的 `from_spec` 落地后，注入器的入参从 JSON 换成 `AgentSpec`，映射表不变。不要把 Codex `config.toml` 同步到 chat。

---

## 不变量

切到 `runtime=codex` 之后仍然成立：

1. 密钥不进 profile 正文、不进日志、不进会同步的 Spec。
2. 不访问用户 `~/.codex`。共享 `codex_home` 只在 `{KimPaths.support}/agent/codex`。会话配置与密钥只在内存 `Config` / `AuthManager` 里，不靠共享 `config.toml` / `auth.json` 当多 agent 真相。
3. 每个 `tool_use` 在取消后仍有结果。动态工具由适配层在 abort 时回一条取消结果；Codex 内部工具由线程 shutdown 负责。PR4 用测试锁住动态工具这一侧。
4. Yield 期间 idle/hard 时钟暂停（FFI yield-wait 已有）。`TokenCount` / keepalive 不复位 idle。
5. 空助手结束不是失败。`visible` 语义不变。
6. 压缩失败不得变成空 Finished。`CompactTask` / `run_auto_compact` 的错误走 `HostError`。
7. `wire_api` 不是 `responses` 的投影不得调用 `start_thread`。

---

## 实现设计：第一刀只嵌入

上文 CX-KD 1–13 仍是目标形状。第一刀（`run_turn`、helper、不进 `run_loop`）已经落地。当前又接上了 `runtime=codex`：默认仍是 Goose，桌面能力页可以切到 Codex。配置在内存里投影，IM 工具走 `dynamic_tools`，压缩阈值是上下文的 70%。MCP 用 `project_extensions` 写进 `Config.mcp_servers`（stdio，或带 url 的 streamable HTTP；不配 OAuth）。技能 denylist 写在本次 `SessionFlags` 层的 `skills.config` 里。`user_agents_skills` 只有在它是绝对路径且目录名是 `skills` 时才加一层 user config，空字符串不会去扫 `$HOME`。KIM 自己的 app skill 引用仍然不落成 Codex 的 `SKILL.md`。跨进程续聊用会话旁边的 transcript 文件，不用 `resume_thread_*`，那个入口会丢掉动态工具。

**E-KD 1 — 只依赖一个 crate：`codex-core-api`。**

`kim-agent-host` 增加：

```toml
codex-core-api = { path = "../../third_party/codex/codex-rs/core-api" }
```

这个 facade 已经把 `codex-core`、login、exec-server、protocol、model-provider 整包拉进来。不再依赖 `codex-cli`、`codex-tui`、`codex-login`、`codex-core`。不把 Codex 成员写进 KIM workspace。不删、不改 submodule 里的源码（含「用不到」的 TUI）。pin = `rust-v0.155.1`（`be2951ea3`）。

`codex-core-api` **没有**再导出 `ConfigBuilder`、`LoaderOverrides`、`login_with_api_key`。要它们就得改 facade 或再依赖别的 crate。第一刀两者都不做。引导方式以 `codex-rs/thread-manager-sample` 为准：它就是上游给库嵌入准备的整包用法。

**E-KD 2 — 新模块照抄样例的 `ThreadManager::new`，不替换 Goose。**

文件：`crates/kim-agent-host/src/codex_rt.rs`。`lib.rs` 只加 `mod codex_rt;`。

构造顺序与样例 `run_main` 相同（`thread-manager-sample/src/main.rs`，`0.155.1` 已含 `passthrough_image_store()`）：

```text
init_state_db
AuthManager          ← E-KD 4
ExecServerRuntimePaths::from_optional_paths(config.codex_self_exe, …)
thread_store_from_config
EnvironmentManager::from_codex_home
resolve_installation_id
CodexHomeUserInstructionsProvider
ExtensionRegistryBuilder   ← 第一刀不装 image generation
ThreadManager::new(..., analytics = None, attestation = None, external_time = None)
start_thread(StartThreadOptions::new(config))
thread.start_turn_if_idle(TurnInputRequest::user_input(...))
thread.next_event() 直到 TurnComplete / Error
thread.shutdown_and_wait()
```

公开函数先收成一个，不进 FFI：

```rust
pub async fn run_turn(opts: CodexEmbedOpts) -> Result<String, HostError>
```

`CodexEmbedOpts` 只有：`codex_home`、`codex_self_exe`、`cwd`、`model`、`api_key`、`prompt`。没有 profile、skills、MCP、dynamic tools、idle 时钟。

Goose 的 `prompt` / `MachineFactory` / harness 本刀 diff 必须为空。共存就是两条函数，聊天仍走 Goose。

**E-KD 3 — `Config` 用样例的结构体字面量，只改必须换的字段。**

把样例 `new_config` 抄进 `codex_rt.rs`，然后只覆盖：

| 字段 | 第一刀的值 |
|---|---|
| `codex_home` | 调用方传入。产品路径以后是 `{KimPaths.support}/agent/codex`。本刀测试用 `tempfile`。**不**调用 `find_codex_home()`，因此不碰 `~/.codex` |
| `cwd` / `workspace_roots` | `opts.cwd` |
| `model` | `opts.model`；空则保持样例的 `None`（用 Codex 默认模型） |
| `model_provider_id` / `model_provider` / `model_providers` | 保持样例：内置 `OPENAI_PROVIDER_ID` + `built_in_model_providers` |
| `codex_self_exe` | `opts.codex_self_exe`。没有它 `ExecServerRuntimePaths` 直接失败，本刀不伪造 |
| `analytics_enabled` | `Some(false)`，与样例后半一致 |
| 其余 | 与样例逐字段相同，包括 `PermissionProfile::read_only()`、`AskForApproval::Never`、`ephemeral: true` |

`ephemeral: true` 是故意的：第一刀不接 rollout resume。压缩若因模型目录里的 `auto_compact_token_limit` 自己发生，让它发生，不在这里设 70% 阈值，也不关。

人设、`developer_instructions`、多 profile toml、skills、MCP、审批档，全部留到嵌入跑通之后。那是初步文档里的 CX-KD 3–8，不是本刀。

pin 升级时，用 `git diff rust-v0.155.1..NEW -- codex-rs/thread-manager-sample/src/main.rs` 看 `new_config` / `ThreadManager::new` 是否加了字段，再改我们这份拷贝。不跟踪 `codex-core` 私有模块。

**E-KD 4 — API key 只用 facade 上已有的内存构造。**

`CodexAuth::from_api_key` 与 `AuthManager::from_auth_for_testing` 都由 `codex-core-api` 导出。后者的名字是上游的测试入口，第一刀照用，不改名、不包一层新登录。这样 key 不写入共享 `auth.json`，两个调用两把 key 不会互相覆盖。

不设 `OPENAI_API_KEY`。不调用未导出的 `login_with_api_key`。空 key 返回现有 `HostError::MissingApiKey`，不起线程。

**E-KD 5 — helper 是 `kim-codex-helper`，不链进 cdylib。**

按下一节「沙箱 helper」落地。`kim-agent-host` 只收路径，不依赖 `codex-arg0` / `codex-cli` / `codex-tui`。桌面 hook 编的是 `cargo build -p kim-codex-helper`，不是 submodule 里的 `codex-cli`。

本刀单测仍不要求 bin 已存在：空路径时 `run_turn` 返回错误。helper 自己的单测只断言「直接启动退出码 2」。带 shell 的 live 一轮放 `#[ignore]`，`KIM_CODEX_LIVE=1` 加上 helper 路径和 key 才跑。

**E-KD 6 — 本刀完成的定义。**

- `cargo check -p kim-agent-host` 通过，依赖树有 `codex-core-api`，没有 `codex-tui`、没有 `codex-cli`。
- `cargo test -p kim-codex-helper` 通过。直接启动退出码 2，且该测试不创建 `~/.codex`。
- `kim-agent-host` 单测：空 key、空 exe 都不 panic，且不创建 `~/.codex`。
- Goose 现有测试无需改断言。
- 可选：ignore 的 live 测试打出一句助手文本。
- 聊天 UI、FFI、`runtime` flag、IM 工具、压缩阈值，都不在本刀。

做完再回到 CX-KD 3 的 `Config` 隔离投影。那时候如果仍然拒绝改 Codex，就继续在拷贝的 `Config { ... }` 上改字段；只有字段拷贝维护不住，才单开一次「是否给 `codex-core-api` 加 `ConfigBuilder` 再导出」的决定。

---

## 沙箱 helper（第二档，和第一刀一起做）

不把沙箱改成进程内调用。改 `process_sandbox.rs` / `fs_sandbox.rs` 的 spawn 会动 Codex，而且 seatbelt / bwrap 仍然要一个子进程。第一档（`local_runtime_paths = None`）能对话，但 shell、沙箱读写、apply_patch 都起不来，不方便测。

**HX-KD 1 — 新 crate，只出一个 bin。**

- 路径：`crates/kim-codex-helper`。workspace member。edition 跟 KIM，仍是 2021。
- `[[bin]] name = "kim-codex-helper"`。禁止叫 `codex`。
- 依赖只走 path，指向 submodule，且只用 helper 相关 crate：

```toml
codex-arg0 = { path = "../../third_party/codex/codex-rs/arg0" }
codex-exec-server = { path = "../../third_party/codex/codex-rs/exec-server" }
codex-apply-patch = { path = "../../third_party/codex/codex-rs/apply-patch" }
codex-linux-sandbox = { path = "../../third_party/codex/codex-rs/linux-sandbox" } # cfg(target_os = "linux")
codex-windows-sandbox = { path = "../../third_party/codex/codex-rs/windows-sandbox-rs" } # cfg(windows)
```

`codex-arg0` 的 `workspace = true` 由 **Codex 自己的** workspace 解析，不并进 KIM workspace。不依赖 `codex-cli`、`codex-tui`、`codex-core-api`。`kim-agent-host` 也不依赖本 crate，避免 helper 链进 `rust_agent`。

**HX-KD 2 — `main` 先认参数，再进上游分发。不改 Codex。**

上游 `arg0_dispatch`（`codex-rs/arg0/src/lib.rs`）在 helper 分支里 `process::exit`，不会返回。非 helper 分支会 `load_dotenv()`，读 `~/.codex/.env`，还会往临时目录丢 PATH 别名。所以不能无条件调用它，也不能用 `arg0_dispatch_or_else` 去跑一段空的 CLI main。

顺序：

1. 用公开常量判断这次是不是 helper。命中才调用 `codex_arg0::arg0_dispatch()`。那些分支不会返回。
2. 没有命中：立刻 `exit(2)`。不调 `arg0_dispatch`，不读 `~/.codex`，不打印用法，不进 TUI。

要认的入口（与 `0.155.1` 的 `arg0_dispatch` 前半一致）：

| 触发 | 上游行为 | 我们测到的功能 |
|---|---|---|
| argv1 `--codex-run-as-arg0-exec-helper` | `run_arg0_exec_helper_main` | shell / 进程沙箱 |
| argv1 `--codex-run-as-fs-helper` | `run_fs_helper_main` | 沙箱读写 |
| argv1 `CODEX_CORE_APPLY_PATCH_ARG1` | core apply_patch | 补丁 |
| argv0 文件名 `apply_patch` / `applypatch` | `codex_apply_patch::main` | 补丁别名 |
| argv0 文件名 `codex-linux-sandbox` | `codex_linux_sandbox::run_main` | Linux bwrap / landlock |
| argv0 文件名 `codex-execve-wrapper` | shell escalation | 提权 exec |
| Windows argv1 `CODEX_WINDOWS_SANDBOX_ARG1` | windows sandbox wrapper | Windows 沙箱 |

常量都从对应 crate 再导出，不手写字符串。pin 升级时 diff `codex-rs/arg0/src/lib.rs` 里 `arg0_dispatch` 的前半；多了分支就补进这张表。不跟踪 TUI。

**HX-KD 3 — 安装布局。**

| 平台 | `codex_self_exe` | `codex_linux_sandbox_exe` |
|---|---|---|
| macOS | `kim-codex-helper` 的绝对路径 | `None` |
| Windows | 同上 | `None` |
| Linux | 同上 | 旁边的 symlink，文件名必须是 `codex-linux-sandbox`，指向同一个 helper |

桌面 hook（`sdk/mobile/hook/build.dart`，仅 macOS / Windows / Linux）在编完 `rust_agent` 之后 `cargo build -p kim-codex-helper`，debug / release 与 Flutter 模式一致。macOS 的 `macos/Scripts/run_macos_assemble.sh` 在 embed 阶段把该二进制拷进 `.app/Contents/MacOS/`，和主程序放在一起，不需要先手工 `cargo build` 再启动。运行时优先用可执行文件旁边的这份，再回退到仓库 `target/`。手机 hook 提前 return，不编。

开发时 live 测试用刚编出来的绝对路径。不复制到 `~/.codex`，不要求用户自己装官方 CLI。

**HX-KD 4 — 这样能测什么，不能测什么。**

能测：线程能起来；官方 API key 一轮对话；只读 profile 下的 shell 被 seatbelt / bwrap 拦住或放行；沙箱读文件；apply_patch。这些都走 core 现成工具，不经过 Dart。

不能测，也不在这档做：TUI、`codex` 子命令、ChatGPT 登录、把沙箱收进 Flutter 进程、手机上的本地 shell。IM 动态工具、压缩阈值、profile 投影仍是后面的 PR，不因为 helper 齐了就提前做。

**HX-KD 5 — 体积。**

`codex-core-api` 已经把 exec-server 链进 cdylib（父进程负责 spawn）。helper 再链一份 sandbox / apply-patch（子进程负责实施）。两份都要，不能为了体积去改 Codex。PR0 记录 macOS release 下 cdylib 与 helper 各自的体积。

---

## PR 切片（嵌入跑通之后，现在不执行）

每片可单独编译。默认 flag 保持 goose，直到下面 PR7 之前的桌面验收写进本文件状态栏。第一刀见上一节，不占用这些编号。

| PR | 内容 | 验收 |
|---|---|---|
| 0 | submodule 钉 `rust-v0.155.1`、`kim-agent-host` path 依赖 `codex-core-api`、`kim-codex-helper` 可编。不改 `run_loop` | `cargo check -p kim-agent-host` 通过，依赖树没有 `codex-tui` / `codex-cli`。`cargo build -p kim-codex-helper` 通过。手机 hook 仍提前 return |
| 1 | `ConfigInjector` + 映射单测。共享 home + 每会话 overrides。不创建线程，不联网 | 给定两个 profile fixture，断言各自 `Config` 不同、共享同一 `codex_home` 路径、70% 阈值、`forced_login_method=api`、未知 chat 厂商被拒、人设在 `developer_instructions` 不在 `base_instructions`、磁盘上**没有**写入 agent 专用 `config.toml` / `auth.json` |
| 2 | API key 注入。官方走 ephemeral / `from_api_key`；自定义走 bearer 覆盖。`enable_codex_api_key_env=false` | 单测：共享 home 无持久化 key；两会话两把不同 key 互不覆盖；日志夹具不含 key |
| 3 | `runtime=codex` 适配层：起线程、`next_event` 映射、abort、idle/hard。helper 路径接上 | 无 key 的单测只走到注入失败。有 key 的手动桌面一轮能出气泡。scripted goose 测试仍绿 |
| 4 | `dynamic_tools` ↔ Dart yield。`send_message` 确认卡仍在 | 现有 `kim_im_tools` / permission 测试在 flag 打开的 host 测试里复用。MCP 不拿来冒充 IM 工具 |
| 5 | 压缩接线与观测。`ContextCompacted` 到达 UI | 映射单测覆盖阈值。桌面长会话能在 rollout 里看到 compaction，且热路径没有 `CompactionOp` |
| 6 | workspace、MCP、skills denylist、审批档 | repo cwd 落在 `ConfigOverrides.cwd`。denylist 的 skill 不出现在指令块里（以 Codex 的 skills 加载结果为准，不字符串匹配提示词凑合） |
| 7 | 文档把默认改成 `codex` 的条件写清，代码默认仍先不动 | 另一次产品决定再翻默认。本规划不授权翻默认 |

PR0 之前不要改 Flutter 聊天路径。

---

## 与现有文档的关系

| 文档 | 关系 |
|---|---|
| H-KD `goose-agent-harness.md` | 监督语义（idle、hard、yield 暂停、单一终端事件、可见性）留在适配层。其中「自写 CompactionOp、自写 400 环内摘要」在 `runtime=codex` 时作废。不删除那份文档，文首加一句「运行时迁移见 codex-embed」即可，本切片不动那份正文 |
| A-KD | 配置仍是数据。Codex toml 是投影 |
| B-KD | capability 仍是汇编输入。输出从 `MachineFactory` step 改成动态工具列表 + Codex 内置工具开关 |
| S-KD | skill 包仍由 KIM 安装到磁盘。启用/禁用交给 `skills.config`。不把 Skill 再实现成 Operation |
| P-KD / C-KD | 厂商目录多一个字段：`wire = responses \| chat`。`chat` 在 Codex 运行时不可选。目录 JSON 的改动放在 PR1，不单开产品改版 |
| HA-KD / bot-KD | 不改。气泡、输入中、bot 身份仍在 IM 层 |

---

## 风险（实现时不要假装已解决）

1. **二进制体积。** `codex-core` 会把 exec-server、sandbox、协议栈链进 `rust_agent` cdylib。PR0 记录 macOS release 体积。不为此去删 Codex 源码。
2. **helper 与 App Sandbox。** `kim-codex-helper` 让线程和本地 shell 在 `cargo` 直接跑时能测。macOS 打进 Flutter App Sandbox / hardened runtime 之后，子进程 seatbelt 仍可能被系统拒绝。PR3 在桌面包里再跑一次真实 shell。失败则包内先关 fs/bash，IM 工具仍可用。这不是改回进程内沙箱的理由，也不是回退 Goose 的理由。
3. **Responses 覆盖面。** 今天 Goose declarative provider 里大量是 Chat Completions。切流后它们不可用，直到端点支持 Responses。设置页必须能解释「此厂商在 Codex 运行时不可用」，而不是发一个 400。
4. **本地压缩质量。** 自定义厂商没有 remote compact。本地摘要仍依赖该厂商把 Responses 摘要调用跑通。PR5 至少用一个非 OpenAI 的 Responses 端点手测一次；测不了就在状态栏写明只验证了官方 API key。
5. **pin 漂移。** `Config` 字段和 `EventMsg` variant 在上游变动快。适配层只依赖 `codex-core-api` 再导出的类型。core 内部路径（`session/turn.rs`）只作行为依据，不在 KIM 里引用私有模块。
