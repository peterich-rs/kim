# 把 Flutter 侧 Rust 收进 workspace，并收束双底座

| 字段 | 值 |
| --- | --- |
| 状态 | Phase 1–5 已落地。Phase 6 的 `agent_flags` 已改成结构体；`KimUiHandle` 上与句柄重复的方法、以及把 `KimBridge` 拆成三个类型，还没做 |
| 对照 | `rust-best-practice`；[codex-embed.md](./codex-embed.md)；[ffi-oo-contract.md](../ffi-oo-contract.md) |
| 不变量 | Goose 与 Codex 都留。手机不链 Agent host。`.so` 名 `libkim_client_ffi.so` 不变 |

## Breaking Change Notice

Phase 1–2 不改 Dart 能调用的函数签名。Phase 4 起 Dart 不再传 `SessionOpenOpts` / `profile_json`。

App 内迁移（没有仓库外的 crate 消费者）：

1. 桌面打开会话改为「profile id + 已缓存的密钥引用」，不再组装 `SessionOpenOpts`。
2. `catalog_*` / `preview_assembled` / `skill_*` 的 `String` JSON 改成 FFI 结构体；Dart 模型只保留表单草稿。
3. `sdk/mobile/rust` 与 `sdk/mobile/rust_agent` 目录删除。`flutter_rust_bridge.yaml` 的 `rust_root` 改指向 `crates/`。

## Feasibility Assessment

业务已经在 workspace 里：`kim-client`、`kim-sdk`、`kim-agent-host`、`kim-desktop-runtime`。Flutter 工程里的两份 crate 各自带 `[workspace]`（`sdk/mobile/rust/Cargo.toml` 第 1 行，`sdk/mobile/rust_agent/Cargo.toml` 第 1 行），所以父 workspace 看不见它们，各有一份 `Cargo.lock`。`rust_agent` 还复制了根 `Cargo.toml` 第 97–102 行的 tungstenite patch。

根 CI 已经 `cargo clippy --workspace --all-features`（`.github/workflows/ci.yml`）。`kim-agent-host` 的 `codex` feature 在 `--all-features` 下会被编到。把 FFI crate 加进成员不会新引入 Codex 源码，只是让默认 `cargo check --workspace`（不带 `--all-features`）也编桌面 FFI。

`unsafe_code = "deny"` 不必把 FFI 留在外面。生成代码的 `unsafe` 用我们拥有的 `lib.rs` 上的模块级 `#[allow(unsafe_code)]` 盖住，codegen 改不掉这个属性。

Feasible with caveats：FRB 2.13 的 `rust_root` 和 native-assets hook 必须同时改；`[profile.dev] split-debuginfo` 不能升到根 profile；手机构建必须继续跳过 `kim-agent-ffi`。

## Current Surface Inventory

IM FFI，`sdk/mobile/rust`，包名 `kim_client_ffi`，crate-type `staticlib` + `cdylib`：

- `src/api/failure.rs` `ApiFailure` — 结构化错误，FRB 不能用名字以 `Error` 结尾的类型。
- `src/api/client.rs` `KimUiHandle` — 约 60 个方法：会话、好友、房间、bot、设置、agent profile CRUD、`agent_flags() -> Result<String, ApiFailure>`。
- `src/api/handles.rs` — `InboxHandle`、`ConversationHandle`、`ContactsHandle`、`MediaHandle`、`AgentCatalogHandle`，以及 `watch_agent_permission` / `watch_agent_ui`。`KimUiHandle` 上仍有同名方法，句柄是第二份入口。
- `src/api/types.rs` — UI 投影类型（`ThreadView`、`MessageView`、`SessionSnapshot` 等）。
- `src/api/auth.rs` `KimAuth` — 登录 / 注册 / 改密。
- `src/api/simple.rs` `greet`、`spec_json_to_blob`、`spec_blob_to_json`。
- `src/frb_generated.rs` — codegen，含 `unsafe`。
- `sdk/mobile/hook/build.dart` — `cratePath: 'rust'`，桌面再编 `rust_agent` 和 `kim-codex-helper`。

Agent FFI，`sdk/mobile/rust_agent`，包名 `kim_agent_ffi`，多一个 `rlib`：

- `src/api/session.rs` `SessionOpenOpts` — `profile_json`、`harness_json`、`api_key`、`goose_mode` 全是 `String`。
- 同文件 `AgentSession::{prompt, prompt_with_context, complete_tool, respond_permission, listen, park, abort, steer, resume, snapshot, reconfigure, close}`，返回 `Result<_, String>`。
- 同文件自由函数 `catalog_vendors`、`catalog_surface`、`catalog_validate`、`skill_portable_list`、`skill_app_catalog`、`preview_assembled`、`capability_catalog_json`、`fetch_supported_models`，返回 `Result<String, String>` 或 `Result<Vec<String>, String>`。
- `src/api/phase.rs` — `SessionPhase` 纯状态机，无 I/O。应离开 FFI。
- 直接依赖 `kim-agent-host` 且打开 `features = ["codex"]`（`Cargo.toml` 第 15 行）。

Dart 调用面：

- `sdk/mobile/lib/bridge/kim_bridge.dart` — `KimAuthPort` + `KimClientPort` + `KimMediaPort` 的唯一实现。
- `sdk/mobile/lib/bridge/goose_bridge.dart` — `AgentSessionPort`，`session_open(sqlitePath, projectRoot, opts)`。
- `sdk/mobile/lib/bridge/agent_host.dart` — `AgentHostController`，只灌密钥、订权限和 presence。
- `sdk/mobile/flutter_rust_bridge.yaml` — `rust_root: rust/`，`dart_output: lib/src/rust`。
- `sdk/mobile/flutter_rust_bridge.agent.yaml` — `rust_root: rust_agent/`，`dart_output: lib/src/rust_agent`。

已在 workspace、本设计不搬的实现：

- `crates/kim-sdk/src/agent/runtime.rs` `AgentRuntime::run_turn`。
- `crates/kim-desktop-runtime` — 进程内 runtime，今天不打开 `codex` feature。
- `crates/kim-agent-host/src/events.rs` `HostError` / `ProviderFail`。
- `crates/kim-agent-host/src/machine.rs` — Goose 状态机。
- `crates/kim-agent-host/src/codex_drive.rs` — `runtime = codex` 时的 `ThreadManager`。

## Design

目标形状：

```text
Dart
  KimAuth / KimUiHandle / 句柄          仅桌面：Agent 事件流
        |                                      |
crates/kim-client-ffi                  crates/kim-agent-ffi
  ApiFailure、投影类型、FRB glue              薄转发，无 profile JSON
        |                                      |
crates/kim-sdk  kim-client            crates/kim-desktop-runtime
                                      AgentRuntime::run_turn
                                               |
                                      crates/kim-agent-host
                                      profile.runtime
                                        goose -> StateMachine
                                        codex -> ThreadManager
```

手机 native-assets 只编 `kim-client-ffi`。桌面再编 `kim-agent-ffi` 和已有的 `kim-codex-helper`。两个 cdylib 不合并。

### 关键决定

1. **两份 FFI crate 成为根 workspace 成员，目录挪到 `crates/`。** 否决继续放在 `sdk/mobile/`：那里的 `[workspace]` 制造第二、第三份 lockfile，patch 会漂。否决只把「手写代码」挪进 workspace、生成 crate 留在 Flutter 工程：依赖版本仍然两套。
2. **`unsafe` 只在生成模块放行。** `lib.rs` 由我们写，`#[allow(unsafe_code)] mod frb_generated;`。crate 仍 `[lints] workspace = true`，因此 `unsafe_code = "deny"` 继续约束 `api/`。否决整个 FFI crate `unsafe_code = "allow"`：手写的 `session.rs` 会失去这条检查。
3. **不把 `[profile.dev] split-debuginfo` 写进根 profile。** 两个 FFI 包单独设 `unpacked`。macOS 上 `off` 会删掉 debug map 仍指向的 `.rcgu.o`，Flutter 把 cdylib 拷进 framework 之后 lldb 对不上断点。`packed` 的 `.dSYM` 不会跟着拷进 app。`unpacked` 把对象文件留在 cargo 输出目录，lldb 按绝对路径读取。根 profile 保持平台默认，避免改服务端二进制的调试信息。
4. **`kim-agent-ffi` 不实现第二套 host。** `SessionOpenOpts`、`profile_json`、`catalog_* -> String` 删掉。目录和技能查询进 `kim-agent-host`，返回结构体；FFI 只做 `From`。否决在 Dart 保留 JSON 再慢慢换：`Result<String, String>` 是现在质量问题的本体。
5. **Goose / Codex 的分发留在 `kim-agent-host`，入口是已有的 `AgentRuntime::run_turn`。** `profile.runtime` 只有 `goose` | `codex`。Dart 不传 `goose_mode` 字符串，也不 `session_open`。否决两条 FFI 各包一个底座：手机误链、两套事件、两套错误类型会回来。
6. **`KimUiHandle` 上与句柄重复的方法，Dart 改走句柄之后再删。** Phase 5 之前两套入口都留，避免一次改完全部 feature。
7. **`phase.rs` 搬进 `kim-agent-host`，改成 `pub` 的领域枚举，快照用枚举而不是 `"idle"` 字符串。** FFI 边界如果 FRB 需要字符串，在 FFI crate 里 `as_str`，不让 host 的公共状态是 `String`。
8. **根 CI `--workspace` 编这两个 crate。** 桌面 FFI 因此打开 `codex` feature，本地 `cargo check --workspace` 会变慢。接受。手机 Flutter job 继续不编 `kim-agent-ffi`。否决 `default-members` 把它藏起来：`--workspace` 仍然会编，藏起来只让人以为没进 workspace。
9. **包名和 `.so` 名不变。** `name = "kim_client_ffi"`，OTA 继续加载 `libkim_client_ffi.so`。路径变了，产物名不变。
10. **`greet` 删除。** 没有调用方的演示函数不跟着搬家。

### 错误

`kim-client-ffi` 继续用 `ApiFailure`。映射停在 FFI 边界，`kim-sdk` 仍返回 `SdkError`。

`kim-agent-host` 继续用 `HostError`。FFI 新增 `AgentFailure`，只在 `kim-agent-ffi`：

```rust
#[derive(Debug, thiserror::Error)]
pub enum AgentFailure {
    #[error("api key missing")]
    MissingApiKey,
    #[error("unknown provider {name}")]
    UnknownProvider { name: String },
    #[error("invalid provider url")]
    InvalidUrl,
    #[error("session busy")]
    Busy,
    #[error("unknown session")]
    UnknownSession,
    #[error("unknown tool call")]
    UnknownToolCall,
    #[error("rate limited")]
    RateLimited,
    #[error("context exceeded")]
    ContextExceeded,
    #[error("host poisoned")]
    Poisoned { recently_active: bool },
    #[error("{message}")]
    Failed { message: String },
}
```

`HostError::Failed` 和 `ProviderFail::Other` 只落进 `AgentFailure::Failed`。不要在 FFI 再包一层 `String` 错误。名字不要以 `Error` 结尾。

### runtime 选择

`AgentProfile.runtime` 缺省或空串视为 `goose`。其它值是 `AgentFailure::Failed`，调用方看到明确的配置错误，而不是静默走 Goose。

```rust
pub enum AgentHarness {
    Goose,
    Codex,
}

impl AgentHarness {
    pub fn parse(runtime: &str) -> Result<Self, HostError> {
        match runtime {
            "" | "goose" => Ok(Self::Goose),
            "codex" => Ok(Self::Codex),
            other => Err(HostError::Failed(format!("unknown runtime {other}"))),
        }
    }
}
```

`run_turn` 内部 `match`。Goose 路径不构造 `ThreadManager`。Codex 路径不装 `CompactionOp`。这条和 [codex-embed.md](./codex-embed.md) 一致，本设计不重写嵌入步骤。

### Dart 在 Phase 4 之后看到的打开方式

桌面 `attach_store` 已经安装 `HostAgentRuntime`。打开一轮不再经过 `goose_bridge`：

```text
用户发送
  -> KimUiHandle / ConversationHandle 发 IM
  -> kim-sdk 调 AgentRuntime::run_turn(dest, profile_id, text, ...)
  -> host 按 runtime 执行
  -> watch_agent_ui / watch_agent_permission 推投影
```

密钥仍是 `cache_agent_secret(key_ref, secret)` 一次。API key 不出现在每轮的 opts 里。

### 依赖

FFI 的 `Cargo.toml` 删掉自己的 `[workspace]`、`[patch]` 和版本号字面量：

```toml
[dependencies]
flutter_rust_bridge = { workspace = true }
kim-sdk = { workspace = true }
tokio = { workspace = true }
```

根 `[workspace.dependencies]` 增加精确 pin：`flutter_rust_bridge = "=2.13.0"`。tungstenite patch 只留根上那一份。

## Phased Implementation

每一相结束时，对应 crate `cargo check` 通过，Flutter 桌面和手机各能编一次 native asset。不要把后一相的 API 删除提前做掉。

### Phase 1: 搬迁，签名不动

**File: `crates/kim-client-ffi/`**（新，内容来自 `sdk/mobile/rust`，去掉 `target/` 和 `Cargo.lock`）

- 删除文件头的 `[workspace]`。
- `version` / `edition` / `license` / `rust-version` 改为 `workspace = true`。
- `[lints] workspace = true`。
- `src/lib.rs` 改为：

```rust
pub mod api;

#[allow(unsafe_code)]
#[cfg(not(frb_expand))]
mod frb_generated;
```

- 保留 `name = "kim_client_ffi"` 和 `crate-type = ["staticlib", "cdylib"]`。
- 把 `rust-toolchain.toml` 原样移来。Flutter 在这个目录里编时仍然看得到交叉 target。根目录的 toolchain 文件不变，仓库根的 `cargo` 不继承这些 target。

**File: `crates/kim-agent-ffi/`**

- 同样处理。`crate-type` 保持 `staticlib`、`cdylib`、`rlib`。
- `kim-agent-host` 仍 `features = ["codex"]`，这一相不改依赖图。
- 删除本文件里的 `[patch]` 和 `[patch."ssh://..."]`。解析改走根 patch。

**File: `Cargo.toml`**

- `members` 加上 `crates/kim-client-ffi`、`crates/kim-agent-ffi`。
- 增加：

```toml
[profile.dev.package.kim_client_ffi]
split-debuginfo = "unpacked"

[profile.dev.package.kim_agent_ffi]
split-debuginfo = "unpacked"
```

- 不要改根 `[profile.dev]`。

**File: `sdk/mobile/flutter_rust_bridge.yaml`**

- `rust_root: ../../crates/kim-client-ffi`。`dart_output` 仍是 `lib/src/rust`。

**File: `sdk/mobile/flutter_rust_bridge.agent.yaml`**

- `rust_root: ../../crates/kim-agent-ffi`。`dart_output` 仍是 `lib/src/rust_agent`。

**File: `sdk/mobile/hook/build.dart`**

- `cratePath` 从 `'rust'` / `'rust_agent'` 改为仓库根下的 `crates/kim-client-ffi` 与 `crates/kim-agent-ffi`。`_desktopAgentHost` 的跳过逻辑不变。
- `_buildCodexHelper` 仍调用根 `cargo`，不链进 FFI。

**File: `.github/workflows/ci.yml`**

- 删掉 `cargo fmt --manifest-path sdk/mobile/rust/...` 两行。根 `cargo fmt --all` 会覆盖新成员。
- `sdk-mobile` job 的 cargo cache 路径从 `sdk/mobile/rust/target` 改为 `crates/kim-client-ffi/target`。若 hook 把 target 放在 crate 目录，agent ffi 同样改。以 hook 实际 `CARGO_TARGET_DIR` 为准，两处一起改。

**File: `.github/workflows/logic-ota.yml`**

- cache workspace 键同样从 `sdk/mobile/rust` 改到新 target 目录。加载的文件名仍是 `libkim_client_ffi.so`。

**File: `sdk/mobile/ota/logic-ota.workflow.yml`**

- 同上，只改 cache 路径。

删除 `sdk/mobile/rust/` 与 `sdk/mobile/rust_agent/`（含各自的 `Cargo.lock`）。不要把 `target/` 提交进来。

这一相 Dart 源码不改。`flutter_rust_bridge_codegen` 若因路径重跑，提交生成结果；若生成物只含路径注释变化，仍然提交，避免下次 codegen 产生大 diff。

### Phase 2: 依赖收成 workspace 表

**File: `Cargo.toml`**

- `[workspace.dependencies]` 增加 `flutter_rust_bridge = "=2.13.0"`。
- 已有的 `tokio`、`serde`、`serde_json`、`tracing`、`futures`、`tokio-util`、`uuid`、`async-trait`、`thiserror`、`tempfile` 给 FFI 用 `workspace = true`。
- `kim-client`、`kim-sdk`、`kim-agent-codec`、`kim-log`、`kim-agent-host`、`kim-desktop-runtime` 补进 workspace 表（缺的那些）。FFI 不再写 `path = "../../../crates/..."`。

**File: `crates/kim-client-ffi/Cargo.toml`、`crates/kim-agent-ffi/Cargo.toml`**

- 每个依赖改成 `workspace = true`。桌面专属依赖保留 `target.'cfg(...)'`，但版本不再手写。
- `kim-desktop-runtime` 仍只在 macOS / Windows / Linux 上依赖。手机 target 链不到它，也链不到 Codex。

跑 `cargo metadata -p kim-client-ffi` 与 `-p kim-agent-ffi`，确认 patch 只有根上那一份，没有第二份 git rev。

### Phase 3: 状态机和会话驱动离开 FFI

**File: `crates/kim-agent-host/src/session_phase.rs`**（新，来自 `phase.rs`）

- `SessionPhase`、`PhaseInput`、`transition`、`stale` 改为 `pub`。
- 单测原样搬来。`unwrap` 只留在测试里。

**File: `crates/kim-agent-host/src/lib.rs`**

- `pub mod session_phase;` 并 re-export `SessionPhase`。

**File: `crates/kim-agent-host/src/drive.rs`**（新）

- 从 `kim-agent-ffi/src/api/session.rs` 搬出不带 `StreamSink` 的部分：解析 opts、创建 `AgentHost`、`prompt` / `abort` / `steer` / `resume`、把 `HostEvent` 收成一个内部枚举。
- 这一相对外仍接受今天的字段集合，结构体放在 host 里，命名 `OpenRequest`，字段类型先保持 `String`，避免和 Phase 4 缠在一起。FFI 的 `SessionOpenOpts` 只 `From` 到 `OpenRequest`。
- 返回 `Result<T, HostError>`，禁止 `Result<_, String>`。

**File: `crates/kim-agent-ffi/src/api/session.rs`**

- 留下 `#[frb]` 类型和函数。函数体调用 `kim_agent_host::drive`。
- `StreamSink<AgentUiEvent>` 的映射留在这里。host 不依赖 `flutter_rust_bridge`。
- 删除 `mod phase`。

这一相 Dart 仍调用 `session_open`。行为以 `rust_agent/tests/session_scripted.rs` 和搬进 host 的单测为准。

### Phase 4: runtime 内部分发，Dart 不再打开底座

**File: `crates/kim-agent-host/src/harness.rs`**（新）

- 上面的 `AgentHarness::parse`。
- `run_turn` 的实现按 profile 上的 `runtime` 调用现有 Goose 机器或 `codex_drive`。空串等于 `goose`。
- Codex 路径不注册 `CompactionOp`。Goose 路径不创建 `ThreadManager`。

**File: `crates/kim-desktop-runtime/src/drive.rs`**

- `run_turn` 只转给 host。不读 Dart 传来的 `harness_json`。

**File: `crates/kim-agent-ffi/src/api/session.rs`**

- 删除 `session_open`、`SessionOpenOpts`、`reconfigure(opts)`。
- 保留 `listen` 形态的事件订阅，改由 `kim-client-ffi` 的 `watch_agent_ui` 推出同一组事件后，本 crate 的订阅变为桌面内部用，Dart 不再直接 `import rust_agent` 的 session API。若 codegen 必须留一个入口，留 `init_app`，不留打开会话。

**File: `sdk/mobile/lib/bridge/goose_bridge.dart`**

- 删除 `session_open` 和 `SessionOpenOpts` 的生产调用。
- `AgentSessionPort` 若测试仍需要脚本化 host，测试双留在 `test/support/`，不要从生产 `ChatAgent` 调用。

**File: `sdk/mobile/lib/bridge/agent_host.dart`**

- 继续只做密钥、权限、presence。不要在这里 `match runtime`。

**File: `sdk/mobile/lib/features/agent/`**

- 创建 / 设置页写入 `AgentProfile.runtime`，取值 `goose` 或 `codex`。不再写 `goose_mode` 进打开参数。
- 具体页面拆分沿用 [flutter-layering.md](../flutter-layering.md)，不在这一相重做 UI 结构。

### Phase 5: 目录和技能不再用 JSON 字符串

**File: `crates/kim-agent-host/src/catalog.rs`**（已有则扩展，没有则新增）

- `vendors()`、`surface(vendor, model)`、`validate(...)`、`portable_skills(user_root, project_root)`、`app_catalog(cache_root)`、`preview(profile, project_root)` 返回结构体，不返回 `String`。
- 路径参数用 `impl AsRef<Path>`，函数内部拥有需要存下来的 `PathBuf`。

**File: `crates/kim-agent-ffi/src/api/session.rs`**

- 删除 `catalog_vendors -> Result<String, String>` 这一组和 `preview_assembled(profile_json)`、`capability_catalog_json`。
- 换成返回 `Vec<Vendor>` 等。投影类型定义在本 crate，`From<host::Vendor>`。
- `fetch_supported_models` 返回 `Result<Vec<String>, AgentFailure>`。模型 id 本身是字符串，这不是把错误或整份目录 stringly 化。

**File: Dart `lib/src/rust_agent/api/session.dart` 的调用方**（`features/agent/catalog.dart`、`agent_profiles.dart`、`agent_profile_model.dart`）

- 解码 JSON 的代码删掉，改读投影类型字段。
- `agent_profile_model.dart` 里只服务于「把 Dart 对象编成 host JSON」的函数，在调用点改为调 FFI 结构体之后删除。文件应明显变短；变短是结果，不要为了行数拆文件。

### Phase 6: IM 句柄收成唯一入口

**File: `crates/kim-client-ffi/src/api/client.rs`**

- `KimUiHandle` 保留生命周期和还没有句柄的命令：`create`、`attach_store`、`start_session`、`stop`、`command`、`watch_session_snapshot`、bot、设置、`cache_agent_secret`。
- 与 `handles.rs` 重复的好友、房间、媒体、时间线方法标成转发到句柄，Dart 改完后删除重复项。
- `agent_flags` / `set_agent_flags(String)` 改成结构体或现有的 `Settings` 字段。不要新增 JSON 协议。

**File: `sdk/mobile/lib/bridge/kim_bridge.dart`**

- 按已有的 `KimAuthPort`、`KimClientPort`、`KimMediaPort` 拆成三个类型，同一个 `KimUiHandle` 可以分给它们。不要再包一层 repository。
- 每个文件只实现一个 port。

**File: `crates/kim-client-ffi/src/api/simple.rs`**

- 删除 `greet`。
- `spec_json_to_blob` / `spec_blob_to_json` 若仍是 `AgentSpec` 的临时桥，留到 `from_spec` 落地后删；本设计不提前删存储格式。

### Phase 7: 验证

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo check -p kim-client-ffi --target aarch64-linux-android`，确认依赖里没有 `kim-desktop-runtime`、没有 `kim-agent-host`。
- 桌面：`cargo check -p kim-agent-ffi`。
- `cd sdk/mobile && dart run custom_lint` 与 `flutter analyze`。
- 桌面手动：一个 `runtime=goose` 人设和一个人设 `runtime=codex` 各跑一轮，权限卡仍走 `respond_agent_permission`。
- 手机：构建不含 `libkim_agent_ffi`。OTA 仍只替换 `libkim_client_ffi.so`。

## Architectural Notes

- 没有仓库外的 semver。Dart 生成代码在 Phase 4、5 会变，要同一 PR 里改调用方。
- `AgentRuntime` 保持 `async_trait` 对象安全。不要把 Goose 和 Codex 的类型写进 trait 方法。
- `run_turn` 跨 task 传拥有的 `String` 和 id。不要把 `SessionOpenOpts` 的引用存进 host。
- 密钥：`cache_agent_secret` 仍是唯一一次把 secret 交进 Rust。日志继续不打 key。`OpenRequest` 搬迁时不要把 `api_key` 放进 `Debug`。
- 不合并 `kim_client_ffi` 与 `kim_agent_ffi`。不把 `kim-agent-host` 并进 `kim-client`。不把 Codex workspace 并进根 `Cargo.toml`。
- 不改 WGateway、`chat.bot.*`、ACK、SQLite schema。
- 根 `[patch.crates-io]` 仍服务 Codex pin。FFI 不再声明第二份。
- `flutter_rust_bridge` 生成文件继续不手改。允许 `unsafe` 的属性写在 `lib.rs`。
- Phase 1 之后 `docs/mobile-client.md` 里「FFI crate 在 workspace 外」那句改成新路径。那是专题文档，和代码同一 PR，不另开说明。

## File Change Summary

- `.github/workflows/ci.yml` -- fmt 不再指向 `sdk/mobile/rust`；cache 路径跟随 crate。
- `.github/workflows/logic-ota.yml` -- cargo cache 路径。
- `Cargo.toml` -- 两个 FFI 成员、workspace 依赖、仅这两个包的 `split-debuginfo = "unpacked"`。
- `Cargo.lock` -- 吞掉两份 FFI lock；patch 只剩一份。
- `crates/kim-agent-ffi/**` -- 从 `sdk/mobile/rust_agent` 迁入；Phase 3 起变薄。
- `crates/kim-agent-host/src/drive.rs` -- 会话驱动，返回 `HostError`。
- `crates/kim-agent-host/src/harness.rs` -- `goose` | `codex`。
- `crates/kim-agent-host/src/session_phase.rs` -- 从 FFI 搬来的状态机。
- `crates/kim-agent-host/src/lib.rs` -- 导出上述模块。
- `crates/kim-agent-host/src/catalog.rs` -- 结构体查询，替换 JSON 字符串 API。
- `crates/kim-client-ffi/**` -- 从 `sdk/mobile/rust` 迁入；`ApiFailure` 留在这里。
- `crates/kim-desktop-runtime/src/drive.rs` -- `run_turn` 不再读 Dart harness JSON。
- `docs/mobile-client.md` -- FFI 路径改到 `crates/`。
- `sdk/mobile/flutter_rust_bridge.yaml` -- `rust_root`。
- `sdk/mobile/flutter_rust_bridge.agent.yaml` -- `rust_root`。
- `sdk/mobile/hook/build.dart` -- `cratePath`。
- `sdk/mobile/lib/bridge/agent_host.dart` -- 不增加 runtime 分支。
- `sdk/mobile/lib/bridge/goose_bridge.dart` -- 生产路径删除 `session_open`。
- `sdk/mobile/lib/bridge/kim_bridge.dart` -- 按三个 port 拆开。
- `sdk/mobile/lib/features/agent/agent_profile_model.dart` -- 停止把 host JSON 当 FFI 协议。
- `sdk/mobile/lib/features/agent/agent_profiles.dart` -- 同上。
- `sdk/mobile/lib/features/agent/catalog.dart` -- 读投影类型。
- `sdk/mobile/ota/logic-ota.workflow.yml` -- cache 路径，不改 `.so` 名。
- `sdk/mobile/rust/**` -- 删除。
- `sdk/mobile/rust_agent/**` -- 删除。
