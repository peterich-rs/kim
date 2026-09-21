# Goose / Codex 运行时问题定位

日期：2026-09-21。范围：定位与复现，不修改产品实现。

## 1. Goose 最终结果不显示、持续 typing：已复现收尾阻塞

```text
Goose harness.turn_end
        │
        ▼
Rust finish_turn → assistant_finished
        │
        ▼
FRB RustStreamSink → Dart driveSession 收到终止事件
        │
        ▼
finally: await sub.cancel()    ✗ 等待 native stream 下一条事件
        │
        ▼
session.close()                ← 本可发出 aborted，但尚未执行
        │
        ▼
submitAgentRun → bot reply / typing off   ← 无法到达
```

阻塞位置是 `sdk/mobile/lib/bridge/agent_bridge.dart:384`。`driveSession` 在 371 行准备返回最终结果，但必须先完成 `finally`；它先等待流取消，390 行才关闭 session。不是遗漏 `completed` 分支：`DriveResult.quiet` 已包含 `completed`。

当前锁定的 flutter_rust_bridge 2.13.0 中，`lib/src/stream/stream_sink.dart` 使用 `async*` + `await for (receivePort)`，再调用 `listenAndBuffer()`。最终事件转发给 Dart 后，底层生成器可能已经恢复到等待下一条 native 消息的位置。此时订阅的 `cancel()` Future 不会立即完成，要等生成器再次前进。

Rust 的 `AgentSession.listen` 在 `sdk/mobile/rust_agent/src/api/session.rs:549` 中持续等待 broadcast 事件，发完终止事件不会主动结束监听。`finish_turn` 已经把 phase 设为 Idle；Goose keepalive 也在 `crates/kim-agent-host/src/harness/mod.rs:396` 停止。Dart 又等取消完成才调用 `session.close()`，形成收尾依赖环。

后果：

- `driveSession` 返回不了，外层 `sink.finish` 和 `submitAgentRun` 都执行不到（`agent_bridge.dart:148`）。
- `FfiAgentRuntime::run_turn` 一直等待 `rx.await`（`crates/kim-sdk/src/agent/runtime.rs`）。
- SDK 的 bot typing 心跳继续每 2 秒发一次，回复也未进入 outbox（`crates/kim-sdk/src/agent/mod.rs:139`）。
- 这条收尾路径由 Goose / Codex 共用，Codex 正常完成时也可能受影响；它本身是异步悬挂，不能解释整个窗口无响应。

### 可独立运行的复现

[agent-frb-stream-cancel.dart](./agent-frb-stream-cancel.dart) 直接使用本项目依赖中的真实 `RustStreamSink`，通过 `NativeApi.postCObject` 投递合成的最终事件。没有加载 KIM 动态库，也没有调用模型。

```sh
dart --packages=sdk/mobile/.dart_tool/package_config.json research/agent-frb-stream-cancel.dart
```

观察到：

```text
current order:
  received assistant_finished
  before cancel
  TIMEOUT: no completion after 800ms; wake native stream
  after cancel (约 800ms)
close signal before cancellation:
  received assistant_finished
  before cancel
  after cancel (0ms)
```

800ms 后的合成 `aborted` 只是让探针退出，证明取消在等新的 native 消息，不是正常收尾所需的时间。生产 `close()` → `abort()` 也会发出 `aborted`（`session.rs:583`、`session.rs:610`），但当前顺序下执行不到。

修复边界：session 的关闭/监听终止不能依赖先取消 Dart 的事件订阅；最终结果提交也不应被无界资源清理扣住。应验证正常完成、失败、主动 abort、yield 恢复后的完成，以及关闭失败时的终止语义。只改 typing 状态会掩盖丢回复的问题。

现有 `sdk/mobile/test/agent/drive_session_stop_reason_test.dart` 使用普通 broadcast `StreamController`；其取消行为不同，无法暴露这个缺陷。Rust session 单测也没有跨越真实 Dart native stream 这一层。

## 2. Codex 发消息后整个窗口无响应：根因尚未确认

用户确认是整个窗口无响应，且发生在另一台机器。本机能采样到的 App 于 9 月 18 日启动，采样时主线程在系统事件循环中，未见 agent 动态库。该样本不能说明故障机器的状态，也不能据此排除主线程阻塞。

2026-09-21 续：用户确认卡住发生在实现该分支的另一台机器上，并认为同样的代码在这台 Mac 上也会有。当前仓库状态支持“问题在分支代码里、不依赖那台机器特有的环境”这一判断，但**这台 Mac 现在仍然无法复现整窗无响应**：

- `kim_mobile` PID 13302 仍是 9 月 18 日 16:16 启动的 Debug 包，主二进制时间戳 9 月 15 日，未加载 agent 动态库。
- `third_party/codex` 仍为空目录，本机编不出带 Codex 的 `kim_agent_ffi`。
- 发送入口本身是异步的：`chat_page.dart` `unawaited(_send)` → `enqueueMessage`（`executeNormal`）→ `AgentRunLoop` → `session_open` / `prompt`（同样 `executeNormal`）。Goose 走同一条 Dart 发送链，窗口不会卡死，所以整窗无响应必须落在 Codex 独有的 native 路径，或落在这条路径触发的平台主线程/同步锁上。

代码可确认的线程边界：

- `session_open`、`prompt`、`resume` 走 FRB `wrap_async` / Dart `executeNormal`。
- `start_prompt` 把实际推理交给 `rt().spawn`，先返回 operation ID（`sdk/mobile/rust_agent/src/api/session.rs:385`）。
- `AgentSession.listen` 是同步 FFI，会取得 opaque 的同步读锁、replay mutex 并注册监听，随后返回；没有在这个函数中调用 Codex 启动。当前没有证据说明它在故障时等待了哪个锁。

因此，不能仅因为看到 Rust 文件读写或 `await ensure_live()` 就断言它在 Flutter 主线程上运行。

`rust_agent` 里有两套独立的 `Runtime::new()`：FRB `SimpleAsyncRuntime` 跑 `session_open` / `prompt` 的 wrapper；应用自己的 `rt()` 跑 `listen` 的转发任务和 `start_prompt` 里真正的 `host.prompt_with_context`（含 `ensure_live` / `start_thread`）。`prompt()` 在拿到 operation ID 后就返回 Dart，**Dart isolate 并不等待 Codex 线程启动**。Codex 启动若卡住，先卡住的是 `rt()` 上的那条任务和持有 `CodexSlot` 的查询，不是 Flutter 的 `await prompt()`。

`listen()` 是这条链上唯一的同步 FFI。生成代码会对 `AgentSession` opaque 做 `blocking_read()`，调用方是 Dart UI isolate。首次 `driveSession` 先 `listen` 再 `resume`/`prompt`，这一次读锁通常是空闲的。若之后有方法以写锁长时间占住 opaque，下一次 `listen` 会把 UI isolate 堵死。当前 `prompt`/`resume`/`close` 都是 `&self` 读锁，还没有看到首次发送必然命中的写锁。

### 分支内、跨机器都可能触发的 Codex 挂起点

这些会卡住 **agent 回合**，需要额外证据才能升格成“整窗无响应”：

| 位置 | 行为 | 为何跨机器 |
|---|---|---|
| `codex_drive.rs` `ensure_live` + `start_turn_if_idle` 在 `arm_deadlines` 之前 | 第一次消息会新建 `ThreadManager`、state DB、environment、skills/MCP；这段没有 idle/hard/cancel | 每条消息 `session_open` 都走一遍 |
| `codex_inject.rs` `official_openai` 分支 | 官方 OpenAI 保留内置 `supports_websockets = true`。自定义厂商已经关掉，注释写明 WS 会先挂在 `wss://…/responses` | 官方账号路径 |
| `embed_config` 样例残留 | `agents_enabled: true`、`agent_max_threads: Some(6)`、`file_opener: VsCode`、`AuthCredentialsStoreMode::File`。`apply_profile` 只改了 `ephemeral = false` 和 analytics | 任意 Codex 会话 |
| `project_extensions` → `config.mcp_servers` | MCP 在 `start_thread` / `Session::spawn` 里拉起，不走 Goose 的 `connect_extensions`（Codex 开会话时显式跳过） | 配置了 MCP 的 profile |
| `close()` | 不调用 Codex `shutdown_and_wait` / `remove_thread` | 多轮发送后资源残留；不能单独解释第一次发送就卡死 |

整窗无响应若真是主线程/UI isolate 卡死，现场 `sample` 仍是最短路径。没有栈之前，优先查：`listen` 的 `blocking_read` 是否等到了写锁、平台线程是否停在 helper 启动或 Keychain/TCC、以及官方 OpenAI 的 websocket 是否把某个同步路径堵住。

### 已确认的 Codex 生命周期缺口

| 位置 | 代码行为 | 可确定的影响 |
|---|---|---|
| `crates/kim-agent-host/src/codex_drive.rs:213`、223、265 | 先锁 `CodexSlot`，执行 `ensure_live` 与 `start_turn_if_idle`，之后才启动 idle/hard deadline | Codex 启动阶段不在现有超时/取消监督范围内；启动不返回就到不了 `pump` 中的 cancellation select |
| `codex_drive.rs:376`；`session.rs:721` | `codex_pending` / snapshot 等同一把锁；prompt 持锁直到完成或 yield | 运行期间状态查询排队；Codex 初始化卡住时这些查询也不返回。这是异步阻塞，不足以证明 UI 主线程卡死 |
| `agent_bridge.dart:287`；`codex_rt.rs:170` | 每条消息新开 session，每次 `StartThreadOptions::new` 创建新 thread | 重复初始化 state DB、environment、manager、skills/MCP；上一轮的运行时和压缩上下文没有原生复用 |
| `codex_drive.rs:225` | 将 `.codex-transcript` 拼进新一轮 user input | 只保留问答纯文本，丢失原生工具历史、角色结构和 compaction 状态 |
| `session.rs:762`；`codex_drive.rs` | 产品 `close()` 只 abort + disconnect Goose MCP，没有 Codex `shutdown_and_wait` / manager remove | 缺少明确的 Codex 资源回收；当前不能证明实际泄漏数量或它就是窗口冻结原因 |

另一个配置更新缺口：`session.rs:735` 的 `reconfigure` 快路径只改 KIM profile，未更新已经启动的 Codex Config；慢路径新建 Goose host，没有调用 `attach_codex`。当前聊天入口每轮重新 open，降低了这条路径的实际触发概率，但不能把它作为 Codex 原生热更新使用。

### 故障现场还需补的证据

在发生无响应的机器上，保留卡住的进程，采集 5 秒 `sample`，同时记录 App 的 commit、构建模式和事件日志。无需先重启 App。

```sh
pgrep -x kim_mobile
sample <上一步的PID> 5 -file /tmp/kim-codex-hang.sample.txt
```

先判定主线程栈是在 FRB 同步锁、动态库加载、原生 MethodChannel/Keychain，还是 Dart 持续执行；再对照 Tokio 线程停在 Codex 的哪一段。排查用日志应覆盖 `session_open`、`listen`、`resume`、`ensure_live`、`start_thread`、首个 Codex event 和 `pump` 的进入/退出；不记录 API key。当前结论保留为“主线程卡死待现场定位”，不把上述异步缺口冒充已确认根因。

## 3. AgentProfile 如何对齐 Codex 现有结构

```text
AgentProfile + ProviderAccount + 设备路径 / 内存密钥
        │
        ▼
ConfigBuilder + ConfigOverrides + 有类型的 Codex 配置
        │
        ▼
StartThreadOptions { config, dynamic_tools, user_instructions, initial_history }
        │
        ▼
持有的 CodexThread → TurnInputRequest → EventMsg → KIM UI 事件
```

以上是建议的注入边界。产品 `AgentProfile` 继续作为配置来源；Codex 的 CLI config profile 不等同于 KIM 联系人/人设。

### 已经接对的字段

`crates/kim-agent-host/src/codex_inject.rs:44` 已把模型、推理强度、context window、70% compaction 阈值、provider、MCP、portable skill denylist 投影到 Codex Config。人设和静态 steer 写入 `developer_instructions`（226 行），没有覆盖 `base_instructions`；KIM 工具通过 `DynamicToolSpec::Namespace` 注入 `StartThreadOptions.dynamic_tools`。这些方向应保留。

### 应收敛的地方

| KIM 输入 | 建议使用的 Codex 表面 | 当前差距 |
|---|---|---|
| 人设、KIM 行为规则、静态 steer | `developer_instructions` | 只注入 identity + steer，Goose capability fragments 没有随之进入；应组装 KIM 需要的规则，不复制 Goose 整套推理提示词 |
| 工作目录、工作区、helper 路径 | `ConfigOverrides.cwd/workspace_roots/codex_self_exe` | 当前通过一份很大的 `Config { ... }` 字面量设值，容易遗漏上游默认/派生字段 |
| model、provider、reasoning、上下文、MCP、skills | Config loader + Codex 有类型配置 | 现在先手填 Config 再修改部分字段；`project_skills` 另外替换 layer stack，可能和将来其余配置层产生分歧 |
| 工具能力与审批 | 动态工具注册、内置工具配置、`Permissions`、审批 Op | 当前把 `bash || fs_write` 合成一个 writable 位；read-only sandbox 不等于禁止 shell。提示词不能作为工具开关的唯一实现 |
| KIM app skills | 将 SkillRef 解析为实际技能内容/目录，接入 pin 支持的 skills / 明确 instruction 输入 | `apply_profile` 对 `profile.skills` 只输出 debug 提示；当前没有接入 KIM app skills 内容 |
| 对话历史 | 保持同一 `CodexThread`；恢复时使用结构化 history / rollout | 现在每条消息重建 thread，拼接纯文本 transcript |
| 热更新/中途 steer | Codex 对应 turn input / 配置更新接口 | 不应只更新 Goose profile 或 steer inbox |

`embed_config` 中的 `include_environment_context`、`include_permissions_instructions` 等还是从样例带来的 `false`（`codex_rt.rs:251`）。需在统一 loader 投影中逐项确认产品语义，不能认为设置了 cwd / 权限对象就一定保留了全部 Codex 原生提示上下文。

### 必须按当前 pin 处理的 API 事实

本仓库 pin 为 `be2951ea34f0d295ed0becf97079f92fa5f6950e`（`rust-v0.155.1`）。本机 `third_party/codex` 为空，因此本次从该确切 commit 读取对应源文件核验，没有用另一份本地 Codex checkout 代替。

- `ConfigBuilder` 和 `ConfigOverrides` 存在于 [core config](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core/src/config/mod.rs#L1382)，但当前 [core-api facade](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-api/src/lib.rs#L58) **未导出它们**。所以“直接改成 `codex_core_api::ConfigBuilder`”现在不可编译。要使用完整 builder，需补一层很小的 facade 导出或明确引入该公开模块所在 crate。
- `Config::load_default_with_cli_overrides_for_codex_home` 是已可调用的方法，也明确不读取 user/project/system 配置；但它固定使用 `ConfigOverrides::default()` 和空 layer stack。cwd / helper 之类还需处理，不能把它当作完整 builder 的无损替换。[对应实现](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core/src/config/mod.rs#L1907)
- `resume_thread_with_history` 的快捷入口确实会从 `StartThreadOptions::new(config)` 重建 options，未传 dynamic tools；但 `StartThreadOptions` 本身同时公开 `initial_history` 和 `dynamic_tools`。应研究保留这两者的原生 start/resume 路径，不能据此认定 Codex 只能恢复纯文本。[对应结构与实现](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core/src/thread_manager.rs#L231)
- 该 pin 的 `ConfigToml` 仍有 `profile/profiles` 字段；当前线上 [OpenAI 配置文档](https://learn.chatgpt.com/docs/config-file/config-advanced) 描述的 CLI profile 机制已经不同。实现应以 pin 为准，避免把最新文档的形状硬套到这个嵌入版本。

## 验证范围

- 真实 FRB `RustStreamSink` 独立探针：已复现取消等待；提前发送关闭产生的事件后立即完成。
- Flutter 现有 stop-reason 测试：尝试运行，但 native-assets 构建因缺少 `third_party/codex/codex-rs/config/Cargo.toml` 失败，未进入测试断言。不能报告为通过。
- Codex 整窗无响应：未在本机复现；另一台机器的故障堆栈仍缺失。续查确认问题可以存在于任何跑这个分支的机器，但本机当前进程/submodule 仍不能当作故障现场。
- 本次只新增本文和诊断脚本，未修改产品代码或执行模型请求。
