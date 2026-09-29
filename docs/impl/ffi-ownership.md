# 把 Flutter 侧逻辑收归 Rust：platform bootstrap 与所有权反转

| 字段 | 值 |
| --- | --- |
| 状态 | 本地已实现，未合入 |
| 对照 | [ffi-oo-contract.md](../ffi-oo-contract.md)；[mobile-client.md](../mobile-client.md)；[ffi-workspace.md](./ffi-workspace.md) |
| 不变量 | Keychain 键名字符串不变（`kim.jwt` 等），升级不丢数据；`.so` 名不变；手机不链 agent host |

## Breaking Change Notice

FRB Dart 侧签名多处破坏性变更，App 内无外部消费者，同一 PR 改调用方：

1. `attachStore(dbPath)` → `attachStore()`；新增一次性 `platformBootstrap(...)`。
2. `startSession(url, token, userAgent, account)` → `startSession()`，无参。
3. `KimAuth(baseUrl, userAgent)` → `KimAuth.create()`，无参。
4. `watchTokenPersist` / `cacheAgentSecret` / `importDeviceSettings(wsUrl, httpOrigin, ...)` 删除或改形（见 Phase 3–5）。
5. `fetchSupportedModels(vendor, baseUrl, apiKey)` 的 `apiKey` → `keyRef`。
6. `skillAppCatalog(cacheRoot)` / `skillPortableList(userRoot, projectRoot)` 参数内化。

## Feasibility Assessment

Rust 已具备几乎全部承接件：settings 表（`kim-sdk/src/store/settings.rs`）已是迁移后的真源；`account_from_token`、`http_origin_from_ws`、`require_secure_auth_origin`、默认 URL 常量都在 `kim-client`；media 目录已示范「一个根目录派生全部布局」（`kim-sdk/src/lib.rs:219`）；`respond_agent_permission` 已验证 oneshot 请求-应答回调模式可行。Flutter 侧真正不可内化的事实只有：目录根（path_provider）、Keychain/Keystore 读写、版本号、模拟器标记、权限与连通性、workspace 目录选择器。**结论：Fully feasible**，风险集中在 secure-store 回调的时序（Phase 3 用超时兜底）与 web 构建（不适用 bootstrap，维持现状）。

## Current Surface Inventory

审计结论按「谁该拥有」分类：

**(a) 平台事实，留在 Flutter**：目录根（`core/paths.dart:33-54`）、Keychain 读写机制（`core/settings.dart:58-78`）、`package_info_plus`、通知权限、`connectivity_plus`、OTA `.so` 路径（`core/ota_info.dart`）、目录选择器 + 安全书签（`workspace_access.dart`）、纯 UI prefs（theme / dest / avatar / notificationsAsked）。

**(b) 逻辑 / 惯例，应收归 Rust**（当前由 Dart 计算后传入）：

| # | 泄漏点 | Dart 侧 | Rust 已有承接件 |
| --- | --- | --- | --- |
| 1 | `dbPath = support/kim-cache.db`，文件名惯例在 Dart | `main.dart:77` | media 目录已示范根目录派生（`kim-sdk/src/lib.rs:219-221`） |
| 2 | `wsUrl` 每次 `startSession` 传入 | `link.dart:193-197` | settings 表 `ws_url` + `DEFAULT_*_URL`（`kim-client/src/config.rs:6-12`） |
| 3 | `token` 每次 `startSession` 传入 | `link.dart:182` | `TokenPersist` 事件已是 Rust 主导；只差存储通道 |
| 4 | `userAgent` 每次现拼 | `core/user_agent.dart:9-19` | `ClientConfig` 已有默认 UA 与 `device_for_target_os`（`config.rs:25-30`） |
| 5 | `account` 参数恒为 `''`（死参数） | `kim_client_bridge.dart:46` | `account_from_token`（`kim-client/src/token.rs`） |
| 6 | `httpOrigin` 传入 `KimAuth` | `kim_bridge_base.dart:48-50` | `http_origin_from_ws`（`config.rs:110-134`） |
| 7 | HTTPS origin 校验 Dart 版 | `core/secure_origin.dart:5-32` | `require_secure_auth_origin`（`kim-client/src/auth.rs:34-58`，语义完全重复） |
| 8 | URL 常量 Dart 镜像 | `settings.dart:14-17` | `config.rs:6-12` 同值 |
| 9 | skill 缓存目录惯例在 Dart | `paths.dart:91-92` → `skills_catalog.dart:170` | 同 #1 模式 |
| 10 | agent 目录布局 / sandbox 清洗 / `AGENTS.md` 播种在 Dart | `paths.dart:74-144` | 无，Phase 1 新建 |
| 11 | 密钥种子循环：Dart 读 Keychain 逐个 `cacheAgentSecret` | `agent_host.dart:87-107` | 桌面 vault 已在 Rust（`kim-desktop-runtime/src/lib.rs:100-108`），缺读通道 |
| 12 | `apiKey` 明文过 FFI | `goose_bridge.dart:62-73` | ffi-oo-contract.md:30 明令禁止 |
| 13 | media 上传 Dart 先写临时文件再传路径 | `kim_media_bridge.dart:17-26` | Rust 可自持临时文件 |
| 14 | `previewAssembled(profileJson, ...)` Dart 拼 JSON | `agent_capabilities_page.dart:219-245` | ffi-workspace.md Phase 4 遗留，本就判死 |

**(c) 一次性数据迁移，保留 Dart 执行**：`importDeviceSettings` 读的是 SharedPreferences（平台存储），但值与去重逻辑归 Rust——改形见 Phase 5。

## Design

### 目标形状

```text
现在（半重构）                          目标（所有权反转）
┌──────────────────────────┐          ┌──────────────────────────┐
│ Flutter                   │          │ Flutter                   │
│  拼 dbPath / url / token  │          │  一次 platformBootstrap:   │
│  拼 UA / origin / 常量    │          │   目录根×4, 版本号, 模拟器  │
│  Keychain 读写 + 种子循环 │          │  Keychain 执行器(回调)     │
│  拼 skill/profile JSON    │          │  纯 UI prefs / 权限 / 连通 │
└──────────┬───────────────┘          └──────────┬───────────────┘
           │ 每次调用带参数                        │ 只有意图 + 事实
┌──────────▼───────────────┐          ┌──────────▼───────────────┐
│ Rust FFI                  │          │ Rust FFI                  │
│  被动接收一切              │          │  布局/设置/密钥/UA 全自持  │
└──────────────────────────┘          └──────────────────────────┘
```

判据一句话：**跨 FFI 的只有意图（intent）、投影（projection）、句柄（handle）、平台事实（fact）四类。任何「Dart 算好了喂给 Rust」的第四种以外内容都是泄漏。**

### 关键决定

1. **一次 `platform_bootstrap`，之后 FFI 无路径无版本参数。** 否决「每个函数继续带路径」：文件名与子目录惯例是业务约定，属于 Rust。否决 Rust 用 `dirs` crate 自行推导：iOS/Android 的标准目录必须走平台 API（path_provider 已解决），再加一份 `dirs` 只会桌面能推导、手机不能，出现两套真源。
2. **密钥用回调通道，不引 `keyring` crate。** Rust 定义 `SecretStore` trait；生产实现是「StreamSink 请求 + `secret_store_respond(id, value)` 应答」的 oneshot 模式——与 `respond_agent_permission` 同构，已被验证。Dart 侧执行器继续用 `flutter_secure_storage`（Keychain 的 accessibility 等**平台配置**本就属于平台层）。否决 `keyring`：Android Keystore 需要自建 JNI 管道，且 macOS 上无法复现现在的 Data Protection keychain 选项；两套平台实现必然漂移。**Keychain 键名字符串原样保留**（`kim.jwt`、`agent.api_key.goose`、`account.<keyRef>`），升级零迁移。
3. **`start_session()` 无参。** URL 从 settings 表读，token 从 SecretStore 读，account 用 `account_from_token` 推导（`client.rs:381-385` 的兜底逻辑转正），UA 用 bootstrap 传入的 version/build + `std::env::consts::OS` 自拼。Dart 只表达「登录了，连」这个意图。
4. **token 写入方向反转，`watchTokenPersist` 删除。** 现在 Rust 发事件、Dart 写 Keychain、下次再把 token 读出来传回去——绕一整圈。反转后 Rust 刷新 token 时直接经 SecretStore 写，`token_unusable` 过期丢弃逻辑也收进 Rust。
5. **`KimAuth::create()` 无参。** origin 从 settings 派生（`http_origin_from_ws`），UA 内部自拼，构造时调 `require_secure_auth_origin`，Dart 的 `core/secure_origin.dart` 预检删除。
6. **URL 常量与 env 切换单源化。** `settings.dart:14-17` 删除；DevPanel 改调 `settings_preset(Local|Prod)`，由 Rust 常量生成 patch。Dart 侧 `Settings` 只剩投影缓存（watch Rust）。
7. **`import_device_settings` 收窄为一次性数据移交。** 读 SharedPreferences（平台存储）合法保留在 Dart，但去重标记、默认值兜底在 Rust（已是如此），移交完成后 Dart 删掉 `dropImportedPrefs` 之外的 URL/origin 读写。
8. **workspace 授权模型：目录选择器产物是「授权」而非「参数」。** `projectRoot`（picker + 安全书签）是平台事实，但解析后的路径作为授权登记进 Rust（`workspace_grant_register(path)`），持久化在 DB，之后 `skill_portable_list` / `preview` 从授权表取，不再每调用带参。`$HOME/.agents/skills` Rust 自己读 `HOME`。
9. **`fetch_supported_models(vendor, base_url, key_ref)`。** key 经 vault/SecretStore 解析，明文不再过 FFI（补上 ffi-oo-contract.md:30 的欠账）。
10. **media 上传改 `media_upload_bytes`。** Dart 现在的 bytes→临时文件→传路径之舞删除，临时文件归 Rust 布局管理。大文件路径变体保留给原生侧已有文件的场景。
11. **`previewAssembled` 删除**（ffi-workspace Phase 4 遗留），能力预览改为 `preview_profile(profile_id)`：profile / overlay / account 本就在 KimSdk，Dart 无可拼。
12. **纯 UI prefs 不动。** theme / dest / avatar / notificationsAsked 留在 SharedPreferences——它们是 UI 状态不是业务真源；分界写进 mobile-client.md。

### 核心类型

```rust
// crates/kim-sdk/src/platform.rs（新）
#[derive(Debug, Clone)]
pub struct PlatformBootstrap {
    pub documents: PathBuf,
    pub support: PathBuf,
    pub cache: PathBuf,
    pub temp: PathBuf,
    pub app_version: String,
    pub build_number: String,
    pub simulator: bool,
}

pub struct Layout { /* 由 PlatformBootstrap 派生，OnceCell 全局单例 */ }

impl Layout {
    pub fn db_path(&self) -> PathBuf;        // support/kim-cache.db
    pub fn log_path(&self) -> PathBuf;       // support/kim.log
    pub fn media_dir(&self) -> PathBuf;      // support/kim-media
    pub fn agent_sessions(&self) -> PathBuf; // support/agent/sessions
    pub fn agent_workspaces(&self) -> PathBuf;
    pub fn app_skill_cache(&self) -> PathBuf; // support/agent/app-skills/cache
    pub fn sandbox_for(&self, profile_id: &str) -> Result<PathBuf, SdkError>; // 清洗规则从 paths.dart:107-115 移植
    pub fn session_file(&self, dest: &str, profile_id: &str) -> PathBuf;      // 清洗规则从 paths.dart:99-105 移植
}
```

```rust
// crates/kim-sdk/src/secrets.rs（新）
#[async_trait]
pub trait SecretStore: Send + Sync + 'static {
    async fn read(&self, key: &str) -> Result<String, SdkError>;
    async fn write(&self, key: &str, value: &str) -> Result<(), SdkError>;
    async fn delete(&self, key: &str) -> Result<(), SdkError>;
}

pub struct KeyringSecretStore {
    tx: mpsc::Sender<SecretOp>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<...>>>>,
}
// FFI: watch_secret_requests(StreamSink<SecretRequest>)  // Dart 订阅
// FFI: secret_store_respond(id: u64, ok: Option<String>) // Dart 应答
// 读超时 10s → 视为空值并 warn，不挂死 start_session
```

```rust
// crates/kim-client-ffi/src/api/bootstrap.rs（新）
pub async fn platform_bootstrap(
    documents: String, support: String, cache: String, temp: String,
    app_version: String, build_number: String, simulator: bool,
) -> Result<(), ApiFailure>;
// idempotent：二次调用返回 Ok 且不覆盖（测试除外，见 set_bootstrap_for_test）
```

### 调用形态对比

```text
现在 main.dart:
  attachStore('<support>/kim-cache.db')
  importDeviceSettings(wsUrl:…, httpOrigin:…)
  watchTokenPersist().listen(→ settings.saveToken)
  AgentHostController._seedSecrets()   // 读 Keychain 循环 cacheAgentSecret

之后 main.dart:
  platformBootstrap(documents:, support:, cache:, temp:,
                    appVersion:, buildNumber:, simulator: platformIsSimulator)
  attachStore()                         // 路径 Rust 自己派生
  secretStoreExecutor.attach(bridge)    // 订阅 SecretRequest → flutter_secure_storage → respond
  importLegacyPrefsOnce(wsUrl:…, httpOrigin:…)  // 仅存量设备，一次性

之后 link.dart:
  startSession()                        // 无 url / token / userAgent / account
```

## Phased Implementation

每相结束 `cargo check --workspace` 与 `flutter analyze` 通过；Phase 2 起 Flutter 手机构建、Phase 3 起桌面构建各验一次。

### Phase 1: `kim-sdk` 布局模块（不改 FFI）

- **File: `crates/kim-sdk/src/platform.rs`（新）** — `PlatformBootstrap` + `Layout` + 全局 `OnceCell`；移植 `paths.dart:99-115` 的段清洗（`[^A-Za-z0-9._-]`→`_`、80 截断、sandbox 拒 `/`、`\`、`..`）与 `ensure_sandbox` 的 `AGENTS.md`/`MEMORY.md`/`notes/` 播种。单测覆盖清洗边界（空串、超长、路径穿越）。
- **File: `crates/kim-sdk/src/lib.rs`** — `pub mod platform;`；`attach_store(db_path)` 增加无参姊妹 `attach_store_default()`（读 Layout）；media_dir 改从 Layout 取。
- **File: `crates/kim-sdk/src/error.rs`** — 增加 `BootstrapMissing` 变体（未 bootstrap 就要路径时报结构化错误，不是 panic）。

### Phase 2: `platform_bootstrap` 落地，路径参数消失

- **File: `crates/kim-client-ffi/src/api/bootstrap.rs`（新）** — 上文签名；写 `Layout` 单例。
- **File: `crates/kim-client-ffi/src/api/client.rs`** — `attach_store()` 无参化（内部 `attach_store_default`）；`media_upload` 的临时文件改写入 `Layout.temp`；新增 `media_upload_bytes(data: Vec<u8>, mime, width, height, byte_size)`。
- **File: `crates/kim-agent-ffi/src/api/catalog.rs`** — `skill_app_catalog()` 无参；`skill_portable_list(project_root)` 的 `user_root` 内部从 `HOME` 推导。
- **File: `sdk/mobile/lib/main.dart`** — bootstrap 调用移到最前；`attachStore()` 去参。
- **File: `sdk/mobile/lib/core/paths.dart`** — 删 `agentRoot`/`agentSessions`/`agentSessionFile`/`agentWorkspaces`/`appSkillCache`/`sandboxFor`/`ensureSandbox`（消费者改走 FFI）；`KimPaths.ensure` 保留（四个根仍是平台事实）。
- **File: `sdk/mobile/lib/bridge/kim_media_bridge.dart`** — 删 `Directory.systemTemp` 写文件之舞。
- **File: `sdk/mobile/lib/features/agent/skills_catalog.dart` / `workspace.dart` / `host_support.dart`** — 跟随改调用。

### Phase 3: SecretStore 回调通道，token 方向反转

- **File: `crates/kim-sdk/src/secrets.rs`（新）** — trait + `KeyringSecretStore` + 超时；键名常量 `KEY_JWT="kim.jwt"`、`KEY_ACCOUNT="kim.account"`、`KEY_AGENT_WRAP="kim.agent_wrap"`、provider 前缀 `account.<keyRef>`——与现有 Keychain 字符串逐字一致。
- **File: `crates/kim-client-ffi/src/api/secrets.rs`（新）** — `watch_secret_requests(StreamSink<SecretRequest>)`、`secret_store_respond(id, ok)`。
- **File: `crates/kim-sdk/src/lib.rs`** — token 刷新/登出路径改写 `SecretStore`；`subscribe_token_persist`（`lib.rs:1487`）与 `TokenPersistEvent` 删除；`token_unusable` 过期丢弃在 session 启动前执行。
- **File: `crates/kim-client-ffi/src/api/handles.rs`** — `cache_agent_secret` 删除（vault 读改为：桌面 `HostAgentRuntime` 经 `SecretStore` 按 keyRef 取）。
- **File: `sdk/mobile/lib/core/secret_store_executor.dart`（新）** — 订阅 `SecretRequest` → `SettingsStore.productionSecureStorage()` 执行 → respond。平台配置（macOS Data Protection、Android RSA-OAEP）只存在于此。
- **File: `sdk/mobile/lib/main.dart`** — `watchTokenPersist` 监听块删除；挂 executor。
- **File: `sdk/mobile/lib/bridge/agent_host.dart`** — `_seedSecrets`/`_readStoredKey` 删除。
- **File: `sdk/mobile/lib/core/settings.dart`** — `saveToken`/`_readToken`/`_writeSecret`/`_readSecret`/`readAgentWrapKey` 删除（token 不再过 Dart）；`productionSecureStorage` 移交 executor。

### Phase 4: `start_session` / `KimAuth` 无参化，Dart 镜像删除

- **File: `crates/kim-client-ffi/src/api/client.rs`** — `start_session()` 无参：settings 表取 URL、SecretStore 取 token、`account_from_token` 推 account、UA = `KIM/{app_version} ({os}; build {build_number})`（`app_version`/`build_number` 来自 bootstrap，OS 来自 `std::env::consts::OS`）。
- **File: `crates/kim-client-ffi/src/api/auth.rs`** — `KimAuth::create()` 无参：settings 派生 origin（缺省 `http_origin_from_ws`）、内部 UA、构造时 `require_secure_auth_origin`。
- **File: `sdk/mobile/lib/features/session/link.dart`** — `startSession()`；token 为空判断改读 Rust `session_snapshot`（`authState`）。
- **File: `sdk/mobile/lib/features/auth/providers/auth.dart`** — `KimAuth.create()`；删 `baseUrl`/`userAgent` 传参。
- **File: `sdk/mobile/lib/core/user_agent.dart`** — 删除。
- **File: `sdk/mobile/lib/core/secure_origin.dart`** — 删除（调用点 `auth.dart:47-49,123-125` 移除）。
- **File: `sdk/mobile/lib/core/settings.dart`** — `defaultUrl`/`localUrl`/`defaultHttp`/`localHttp`/`url`/`httpOrigin` 字段与读写删除；`Settings` 变为「UI prefs + Rust Settings 投影缓存」。
- **File: `sdk/mobile/lib/bridge/kim_client_bridge.dart` / `kim_bridge_base.dart` / `kim_auth_bridge.dart`** — 签名跟随。

### Phase 5: 设置单源 + 一次性迁移收窄

- **File: `crates/kim-sdk/src/lib.rs`** — 新增 `settings_preset(Preset::{Local, Prod})`：由 `kim-client` 常量生成 `DeviceSettings` patch（env 同步写 dev/prod）。
- **File: `crates/kim-client-ffi/src/api/client.rs`** — `settings_patch` 收窄为不接受裸 URL 常量以外的漂移来源（自定义 URL 输入框场景保留 `settings_set_urls(ws_url)`，origin 必须派生）；`import_device_settings` 更名 `import_legacy_prefs`，语义不变。
- **File: `sdk/mobile/lib/features/settings/dev_panel.dart`** — `useLocal()/useProd()` 改 `settingsPreset(...)`；URL 常量引用删除。
- **File: `sdk/mobile/lib/main.dart`** — `importLegacyPrefs` 仅在 Rust `imported_prefs=false` 时执行（Rust 返回是否已迁移，避免每次启动传值）。

### Phase 6: Agent 侧收尾（合并 ffi-workspace Phase 4/5 遗留）

- **File: `crates/kim-sdk/src/store/`（workspace 授权表）** — `workspace_grants(profile_id, path, created_at)`；`workspace_grant_register/list/revoke` FFI。
- **File: `crates/kim-agent-ffi/src/api/session.rs`** — `fetch_supported_models(llm_backend, base_url, key_ref)`（key 经 vault）；`preview_assembled(profile_json, ...)` 删除，换 `preview_profile(profile_id) -> CapabilityPreview`。
- **File: `sdk/mobile/lib/features/agent/workspace_access.dart`** — picker + 书签解析保留（平台），解析结果 `workspaceGrantRegister(path)`。
- **File: `sdk/mobile/lib/bridge/goose_bridge.dart`** — `fetchModels` 改 keyRef；`previewAssembled` 调用删除。
- **File: `sdk/mobile/lib/features/agent/agent_capabilities_page.dart`** — Dart 拼 JSON 块（`:219-245`）删除，读 `CapabilityPreview` 投影。

### Phase 7: 验证

- `cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets --all-features -- -D warnings`；`cargo test --workspace --all-features`。
- `cd sdk/mobile && dart run custom_lint && flutter analyze && flutter test`。
- 手机构建：无 `libkim_agent_ffi`；OTA 流程照旧。
- 桌面手动：登录 → 收 token（确认 Keychain 由 executor 写入，键名不变）→ 改 DevPanel env → 跑一轮 goose agent（密钥不再种子循环）→ 能力预览不再传 JSON。
- 存量升级路径手测：带旧 Keychain + 旧 prefs 的安装包升级 → token/账号/URL 全保留。
- web：`frb_generated.web.dart` 路径确认 bootstrap 不被 web 调用（web 仍走 `KIM_WS_URL`/浏览器 UA 现状）。

## Architectural Notes

- **不引 `keyring`/`dirs` 新依赖**；回调通道复用 FRB StreamSink + oneshot，模式与 `respond_agent_permission` 一致。
- **键名即数据格式**：Keychain 字符串是这个方案里唯一「Rust 定义、平台执行」的跨层契约，注释钉死在 `secrets.rs` 常量上，改键名 = 数据丢失。
- **SecretStore 时序**：respond 丢失/超时 → 按空值降级 + warn，不 panic 不挂死；executor 在 attach 前发起的请求排队（ChannelBuffer）。
- **不改**：WGateway 协议、ACK、SQLite schema（除新增 grants 表）、`.so` 命名、`kim.rustStore` 语义、web SDK。
- **`KimUiHandle` 方法去重与 `KimBridge` 三拆**（ffi-workspace Phase 6）不在本切片——本切片 Phase 4 已顺带走掉 `start_session`/auth 两个重复面，余下仍归 ffi-workspace.md 跟踪。
- **纯 UI prefs 分界**：theme / dest / avatar / notificationsAsked 永久留在 Dart，写回 mobile-client.md，防止下一轮「彻底化」矫枉过正。
- 语义化版本无意义（App 内消费），但 FRB 生成物与调用方必须同一 PR。

## File Change Summary

- `crates/kim-sdk/src/platform.rs` -- 新：PlatformBootstrap/Layout/路径清洗/沙盒播种
- `crates/kim-sdk/src/secrets.rs` -- 新：SecretStore trait + KeyringSecretStore 回调实现 + 键名常量
- `crates/kim-sdk/src/lib.rs` -- attach_store_default、token 写经 SecretStore、删 TokenPersist、settings_preset
- `crates/kim-sdk/src/store/settings.rs` -- preset 支持
- `crates/kim-sdk/src/store/grants.rs` -- 新：workspace 授权表
- `crates/kim-sdk/src/error.rs` -- BootstrapMissing
- `crates/kim-client-ffi/src/api/bootstrap.rs` -- 新：platform_bootstrap
- `crates/kim-client-ffi/src/api/secrets.rs` -- 新：watch_secret_requests / secret_store_respond
- `crates/kim-client-ffi/src/api/client.rs` -- attach_store 无参、start_session 无参、media_upload_bytes、settings 收窄
- `crates/kim-client-ffi/src/api/auth.rs` -- KimAuth::create 无参 + secure origin 强制
- `crates/kim-client-ffi/src/api/handles.rs` -- 删 cache_agent_secret
- `crates/kim-agent-ffi/src/api/session.rs` -- fetch_supported_models 改 keyRef、删 preview_assembled
- `crates/kim-agent-ffi/src/api/catalog.rs` -- 路径内化
- `sdk/mobile/lib/main.dart` -- bootstrap/executor/import once/删 token 监听
- `sdk/mobile/lib/core/secret_store_executor.dart` -- 新：Keychain 执行器
- `sdk/mobile/lib/core/paths.dart` -- 只留四个根
- `sdk/mobile/lib/core/settings.dart` -- 删 URL/token/secure 逻辑，留 UI prefs + 投影
- `sdk/mobile/lib/core/user_agent.dart` -- 删除
- `sdk/mobile/lib/core/secure_origin.dart` -- 删除
- `sdk/mobile/lib/bridge/agent_host.dart` -- 删种子循环
- `sdk/mobile/lib/bridge/goose_bridge.dart` / `kim_client_bridge.dart` / `kim_bridge_base.dart` / `kim_auth_bridge.dart` / `kim_media_bridge.dart` -- 签名跟随收窄
- `sdk/mobile/lib/features/session/link.dart` -- startSession() 无参
- `sdk/mobile/lib/features/auth/providers/auth.dart` -- KimAuth.create()
- `sdk/mobile/lib/features/settings/dev_panel.dart` -- settingsPreset
- `sdk/mobile/lib/features/agent/workspace_access.dart` -- 授权登记
- `sdk/mobile/lib/features/agent/agent_capabilities_page.dart` -- 投影替代拼 JSON
- `docs/mobile-client.md` / `docs/ffi-oo-contract.md` -- 所有权分界、四类跨界判据、UI prefs 边界写回
