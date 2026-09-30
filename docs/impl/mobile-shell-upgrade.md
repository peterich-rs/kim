# 移动壳升级：Rust 下沉收尾 + 模块与大文件拆分 + 依赖收缩

| 字段 | 值 |
| --- | --- |
| 状态 | 本地已实现，尚未合入主干 |
| 分支 | `refactor/ffi-ownership`（在 ffi-ownership 两提交之后续切） |
| 对照 | [mobile-client.md](../mobile-client.md)；[flutter-layering.md](../flutter-layering.md)；[ffi-oo-contract.md](../ffi-oo-contract.md)；[impl/ffi-dead-surface-cleanup.md](./ffi-dead-surface-cleanup.md) |
| 前提 | `refactor/ffi-ownership`（闭集投影）先合入。本方案 Phase 3 起依赖 `ThreadKind` / `MediaKind` / `OutgoingContent` 已落地 |
| 参考 | `fire` 仓库（`/Users/fannnzhang/code/github.com/fire`）：平台自有 Keychain/Keystore、`verify-roadmap-architecture-constraints.sh` 架构 lint、`docs/architecture/2026-09-25-module-boundaries.md`「一文件答一问」 |

App 没有已发布安装包。遗留迁移代码（agent flags 的 prefs 搬家、`readLegacyDevicePrefs`）按死代码整删，不写兼容。

## Feasibility Assessment

依赖侧：`flutter_animate` 与 `cupertino_icons` 在 `lib/` 零引用，删除无风险。`uuid` 仅 2 个调用点（`chat_session.dart:315,332`、`kim_im_tools.dart:171`），而 ffi-ownership 分支的 `enqueue_message(client_id: Option<String>)` 已允许 `None`，Rust 侧生成 clientId 是收口不是新面。`flutter_secure_storage` 的全部生产用途是 `secret_store_executor.dart` 对 Rust `SecretRequest` 的 read/write/delete 执行，外加 `workspace_access.dart` 的 macOS 书签——书签不是秘密，本方案移进 Rust store；执行器换成本地 channel（iOS `SecItem` / Android Keystore AES-GCM wrap / macOS Data Protection），模式与 fire 的 `FireAuthCookieKeychainStore` / `FireCredentialStore` 相同，只是宿主从原生 App 换成 Flutter 插件壳。

边界侧：26 个 `features/**`、`design/**`、`core/**` 文件直接 import `package:kim_mobile/src/rust/**`（盘点见下），bridge 端口被绕过，但类型本身已闭集化，收口是改 import + bridge re-export，不改行为。`tools/check_page_size.sh` 已存在，只是没进 `.github/workflows/ci.yml`。

Fully feasible. 全部改动不碰协议、不碰 `kim-client` wire、不碰服务端。

## Current Surface Inventory

### (a) 依赖现状（pubspec.yaml）

| 包 | 用途证据 | 处置 |
| --- | --- | --- |
| `flutter_animate` ^4.5.2 | `lib/` 零 import | **删** |
| `cupertino_icons` ^1.0.8 | `lib/` 零 import（主题 Material 3） | **删** |
| `uuid` ^4.6.0 | `chat_session.dart:315,332`、`kim_im_tools.dart:171` 生成 `clientId` | **删**，Rust 生成 |
| `flutter_secure_storage` ^11.0.0 | `secret_store_executor.dart`（Rust SecretRequest 执行器）、`workspace_access.dart:75-93`（macOS 书签） | **删**，本地 channel 替代；书签下沉 Rust |
| `shared_preferences` ^2.5.5 | `core/settings.dart`（theme/dest/avatar/notificationsAsked）、`catalog.dart:90,108`（模型缓存）、`agent_profiles.dart:279,315`（flags 遗留迁移） | **收缩**：只留 `core/settings.dart` 的纯 UI 态 |
| `file_picker` ^12.3.0 | `workspace_access.dart:66`、`agent_plaza_page.dart:171`（桌面选目录） | 留（平台 UI，替换成本高于收益） |
| `connectivity_plus` / `package_info_plus` / `intl` / `flutter_displaymode` / `skeletonizer` 等 | 各有真实调用 | 留 |

### (b) Rust 下沉残留（应为 Rust 所有、现在在 Dart）

| 残留 | 位置 | 目标 |
| --- | --- | --- |
| `clientId` 生成 | `chat_session.dart`、`kim_im_tools.dart` 的 `Uuid().v4()` | `enqueue_message(client_id: None)`，Rust 生成并回填 receipt |
| 厂商模型缓存 | `catalog.dart:85-112` SharedPreferences JSON | `kim-sdk` settings 表（`catalog.models.<vendor>`） |
| agent flags 遗留迁移 | `agent_profiles.dart:270-330` `_loadFlags` / `_importPrefsProfiles` / `_kMultiMigrated` | 整删（无旧安装包） |
| macOS 工作区书签 | `workspace_access.dart` 存 secure storage `agent.workspace_bookmark.<id>` | 书签 bytes 存 Rust grant registry；channel 只做 start/stopAccessing |
| `readLegacyDevicePrefs` | `main.dart:176-181`，零调用 | 整删 |
| `features/chats/inbox.dart` | 兼容 re-export 一行文件 | 整删，改调用方 import |

### (c) FFI 边界违规

`grep -rln "package:kim_mobile/src/rust" lib | grep -v ^lib/src` = 26 个文件，其中 `features/**` 20 个、`design/conversation_tile.dart`、`core/{secret_store_executor,failures,errors}.dart`、`models/models.dart`、`main.dart`。端口（`lib/bridge/kim_ports.dart`）形同虚设。`src/rust_agent` 收口良好（仅 `goose_bridge.dart`、`catalog.dart`、`main.dart`）。

### (d) 超软上限文件（400 行，`flutter-layering.md`）

| 行数 | 文件 | 拆法 |
| --- | --- | --- |
| 897 | `features/agent/agent_capabilities_page.dart` | 按 im/fs/mcp/skills 四节拆 `views/sections/` |
| 758 | `features/agent/agent_settings_page.dart` | 编辑器分节拆 `views/widgets/`；`AgentSettingsPage` 别名类删 |
| 704 | `design/kim_bubble.dart` | 拆 `message_row/` 子件（内容/发送态/选择态/卡片） |
| 626+337+402 | `features/agent/agent_profiles.dart` + 两个 part | notifier 与持久化分离；`agent_capability_kinds.dart` 并入 catalog 域 |
| 544 | `bridge/kim_client_bridge.dart` | 闭集落地后自然缩小；再按域拆 part 文件 |
| 512 | `models/models.dart` | 按 Thread/Message/Link/Event 拆 4 文件 + barrel |
| 496 | `features/agent/provider_account_page.dart` | 表单分节 |
| 464 | `design/chat/chat_list.dart` | 控制器与滚动策略分离 |
| 424 | `design/kim_composer.dart` | 输入区/附件区/发送态拆件 |
| 408 | `features/agent/catalog.dart` | 缓存下沉后剩投影，预计 <300 |

### (e) CI 缺口

`tools/check_page_size.sh` 存在但 `ci.yml` 的 `sdk-mobile` job 没有跑它；也没有任何机制拦 (c) 的跨层 import。

## Design

### 关键决定

1. **`flutter_secure_storage` 换成本地 channel，不引入新三方库。** 生产用法只有 read/write/delete（+ iOS `first_unlock_this_device`、macOS Data Protection、Android RSA-OAEP+AES-GCM 选项）。fire 的终态证明这三类平台配置用 150 行原生代码（Swift `SecItemAdd/Update/CopyMatching/Delete` + Kotlin Keystore `Cipher`）即可覆盖。否决「继续用插件」：为 3 个 API 拖完整插件与传递依赖；否决 Rust 侧 `keyring` crate」：iOS entitlement / macOS Data-Protection / Android Keystore wrap 的平台语义在平台层最自然，且现有 SecretRequest 协议本就把执行放平台侧，Rust 仍是键名与生命周期唯一所有者。channel 名 `kim.keystore`，实现在 `Runner/AppDelegate` / `MainActivity` 内，不建独立插件包（体量不值得）。
2. **`clientId` 收归 Rust。** `enqueue_message` 已收 `Option<String>`。Dart 两处传 `None`，Rust 生成并经 `CommandReceipt.client_id` 回填（outbox 重试本就靠 receipt）。删 `uuid` 包。否决 Dart 留一个本地生成器：同一标识两处生成规则迟早分叉。
3. **缓存与书签下沉，prefs 只留 UI 态。** catalog 模型缓存进 `kim-sdk` settings 表（已有 get/patch 通道，不新增 FFI 面）；macOS 安全作用域书签的 bytes 存 Rust grant registry（ffi-ownership 已把 workspace grant 收进 Rust），channel `kim.workspace` 只保留 start/stopAccessing 两个原生调用。落定后 `shared_preferences` 的合法消费只剩 `core/settings.dart`（theme、dest、avatar、notificationsAsked）。
4. **FFI import 收口 + 架构 lint。** 只有 `lib/bridge/**`、`lib/core/secret_store_executor.dart`、`lib/main.dart` 允许 import `src/rust*`；端口类型经 `kim_ports.dart` re-export。新增 `tools/check_flutter_boundaries.sh`（rg 检查，仿 fire `verify-roadmap-architecture-constraints.sh`），与 `check_page_size.sh` 一起进 `sdk-mobile` CI job。否决只用 `analysis_options` import 规则：custom_lint 已有三个包在打架，不想再加 analyzer 插件。
5. **`features/agent/` 目录化，对齐 `chats/`。** 40 个平铺文件迁到 `views/`（页面）、`providers/`（notifier + 状态）、`widgets/`（分节组件）、`data/`（catalog / provider_accounts 投影）。`create/` 向导保持子目录。`session/` 仍是共享内核不动；`contacts/`、`profile/` 顺手补 `views/providers` 两层。纯 `git mv` + import 改写，无行为变更。
6. **大文件按「一文件答一问」拆，不按行数机械切。** fire `module-boundaries.md` 的原则：拆分单位是职责（一节表单、一种气泡状态、一个滚动策略），行数软上限只是触发器。`chat_session.dart`（378 行）不拆文件，但把 toast / redirect 从 notifier 移回 view 的 `ref.listen`（flutter-layering 已写明 ChatPage 只 listen）。
7. **死代码整删不整改。** `readLegacyDevicePrefs`、`inbox.dart` re-export、`AgentSettingsPage` 别名类（`agent_settings_page.dart:44-46`）、agent flags 的全部 prefs 迁移分支。理由同 ffi-ownership 前提：没有已发布安装包。
8. **不动。** Riverpod 3 / go_router 18 / flex_color_scheme / 三栏 `HomeShell` 骨架（否决 fluxdo 式可配置 NavEntryRegistry：kim 三栏固定，无配置需求）；`kim-agent-ffi` 桌面 catalog 边界；协议与 wire；`kim_media_picker`；`file_picker`。

### 调用形态（密钥路径，落定后）

```text
Rust kim-sdk                    Dart executor                 平台
SecretRequest {read/write/del} → kim.keystore MethodChannel  → iOS SecItem
键名 kim.jwt / account.<ref>     (secret_store_executor.dart) → Android Keystore AES-GCM wrap
生命周期/并发全部 Rust            无插件、无三方库              → macOS Data Protection keychain
```

## Phased Implementation

每相结束 `cd sdk/mobile && dart run custom_lint && flutter analyze && flutter test` 通过；涉及 Rust 的相加 `cargo check -p kim-sdk -p kim-client-ffi`。Phase 1–2 可在 ffi-ownership 合入前独立做，Phase 3 起依赖闭集。

### Phase 1: 死依赖与死代码

- **File: `sdk/mobile/pubspec.yaml`** — 删 `flutter_animate`、`cupertino_icons`。`flutter pub get` 后 analyze 验证零引用。
- **File: `sdk/mobile/lib/main.dart`** — 删 `readLegacyDevicePrefs`（176-181）。
- **File: `sdk/mobile/lib/features/chats/inbox.dart`** — 整删；调用方改 import `providers/inbox.dart`。
- **File: `sdk/mobile/lib/features/agent/agent_settings_page.dart`** — 删 `AgentSettingsPage` 别名类；调用方（router）直接 `AgentEditorPage(profileId: kGooseAgentId)`。

### Phase 2: kim.keystore channel 替换 flutter_secure_storage

- **File: `sdk/mobile/ios/Runner/AppDelegate.swift`** — 注册 `kim.keystore` channel：`read/write/delete/contains` 四方法，`kSecClassGenericPassword` + `kSecAttrAccessibleAfterFirstUnlockThisDevice`；macOS target 用 Data-Protection keychain 属性（对齐现 `secret_store_executor.dart:22-43` 的选项），无生物识别 UI。
- **File: `sdk/mobile/android/app/src/main/kotlin/.../MainActivity.kt`** — 同 channel：Keystore AES key + `Cipher` AES/GCM/NoPadding wrap 值存 SharedPreferences blob（对齐现 Android 语义 RSA-OAEP+AES-GCM 之上的加密偏好）。
- **File: `sdk/mobile/lib/core/secret_store_executor.dart`** — `FlutterSecureStorage` 换 `MethodChannel('kim.keystore')`，接口不变；保留现有订阅 / 应答 / 排队逻辑。
- **File: `sdk/mobile/macos/Runner/AppDelegate.swift`** — 若 macOS 走独立入口则同 iOS 实现；否则共用 swift 文件。
- **File: `sdk/mobile/pubspec.yaml`** — 删 `flutter_secure_storage`；`macos/Flutter/GeneratedPluginRegistrant.swift` 随 `pub get` 再生成。
- 测试：iOS 模拟器 / Android 设备各跑一次登录，确认 `kim.jwt` 落 Keychain / Keystore 且重启免登录。书签尚未迁移，本相 `workspace_access.dart` 暂走同一 channel（键名不变，值兼容：老值是插件写的 base64，channel 首版读时兼容两种编码，Phase 3 连数据一起删）。

### Phase 3: Rust 下沉收尾（依赖 ffi-ownership 合入）

- **File: `crates/kim-sdk/src/lib.rs`** — `enqueue_message` 的 `client_id: None` 分支生成 uuid v4（Rust 侧已有 uuid 依赖则复用）；`CommandReceipt` 回填。settings 表新增 catalog 缓存读写（复用现有 settings kv，键 `catalog.models.<vendor>`，`Vec<String>` JSON 存列）。
- **File: `crates/kim-client-ffi/src/api/client.rs`** — 暴露 `catalog_model_cache(vendor)` / `set_catalog_model_cache(vendor, models)`（或并进现有 fetch_models 的返回，避免新面：优先后者——`fetch_models` 直接带 5 分钟缓存，Dart 无感）。
- **File: `sdk/mobile/lib/features/agent/catalog.dart`** — 删 `loadCatalogModelCache` / `saveCatalogModelCache`（85-112），改走端口。
- **File: `sdk/mobile/lib/features/chats/providers/chat_session.dart`**、**`features/agent/kim_im_tools.dart`** — 删 `Uuid()`，`clientId: null`。
- **File: `sdk/mobile/pubspec.yaml`** — 删 `uuid`。
- **File: `crates/kim-sdk/src/store/agent.rs`** — workspace grant registry 增加 bookmark blob 列（bytes，非文本）。
- **File: `sdk/mobile/lib/features/agent/workspace_access.dart`** — 书签读写改走端口（grant registry）；`kim.workspace` channel 只留 start/stopAccessing；删 `agent.workspace_bookmark.*` 键与 Phase 2 的兼容读。
- **File: `sdk/mobile/lib/features/agent/agent_profiles.dart`** — 删 `_loadFlags` 的空分支迁移、`_importPrefsProfiles`、`_kMultiMigrated` / `_kIdentityMigrated` / `_kProfiles` 常量（270-330）。

### Phase 4: 边界收口 + CI 门

- **File: `tools/check_flutter_boundaries.sh`（新）** — rg 断言：`package:kim_mobile/src/rust` 仅出现在 `lib/bridge/**`、`lib/core/secret_store_executor.dart`、`lib/main.dart`、`lib/src/**`；`package:kim_mobile/src/rust_agent` 仅 `lib/bridge/goose_bridge.dart`、`lib/features/agent/catalog.dart`、`lib/main.dart`；`SharedPreferences` 仅 `lib/core/settings.dart`。退出码非零即失败。
- **File: `lib/bridge/kim_ports.dart`** — re-export 投影类型（`ThreadKind` / `MediaKind` / `Relation` / `SendStatus` / `Settings` 等），成为 features 的唯一入口。
- **File: 20 个 `features/**` 文件 + `design/conversation_tile.dart` + `core/{failures,errors}.dart` + `models/models.dart`** — `src/rust` import 改 `package:kim_mobile/bridge/kim_ports.dart`。
- **File: `.github/workflows/ci.yml`** — `sdk-mobile` job 加 `- run: tools/check_flutter_boundaries.sh` 与 `- run: tools/check_page_size.sh`（在 analyze 之前）。

### Phase 5: features/agent 目录化（纯移动）

- `git mv`：页面 `*_page.dart` → `features/agent/views/`；notifier / 状态（`agent_profiles.dart` 拆出的部分、`provider_accounts.dart`、`agent_capability_kinds.dart`）→ `providers/`；表单与分节组件 → `widgets/`；catalog 投影 → `data/`。`create/` 整目录不动。
- `contacts/`、`profile/` 补 `views/` + `providers/` 两层。
- 全仓 import 改写；`dart run custom_lint` + `flutter test` 验证零行为变更。

### Phase 6: 大文件拆分（每文件一个独立 PR 粒度）

- **`agent_capabilities_page.dart` 897 →** `views/agent_capabilities_page.dart`（壳 + GlobalKey 定位）+ `views/sections/{im,fs,mcp,skills}_section.dart` 四件；表单状态留 `agent_capabilities_form.dart`。
- **`agent_settings_page.dart` 758 →** `views/agent_editor_page.dart`（壳）+ `views/widgets/{basics,provider,model,persona}_section.dart`。
- **`design/kim_bubble.dart` 704 →** `design/message_row/` 目录：`message_row.dart`（组装）、`bubble_content.dart`、`send_state_icon.dart`、`selection_mode.dart`、`agent_card_bubble.dart`。
- **`agent_profiles.dart` 626 →** `providers/agent_profiles_notifier.dart`（纯状态）+ `data/agent_profile_codec.dart`（文档往返）；`agent_profile_model.dart`（337）随迁 `providers/`。
- **`models/models.dart` 512 →** `models/` 目录：`thread.dart`、`message.dart`、`link.dart`、`event.dart` + `models.dart` barrel（外部 import 不变）。
- **`design/chat/chat_list.dart` 464 →** `chat_list.dart`（widget）+ `chat_list_controller.dart` 已存在则只移滚动锚定策略。
- **`design/kim_composer.dart` 424 →** `composer/{input_field,attachment_tray,send_button}.dart`。
- **`provider_account_page.dart` 496、`catalog.dart` 408 →** 分节 / 下沉后复测行数，仍超再拆。
- **`chat_session.dart` 378 →** 不拆文件；toast / redirect 移到 `chats_page.dart` 的 `ref.listen`。

### Phase 7: 文档回写 + 验证

- **File: `docs/mobile-client.md`** — 依赖清单更新（无 flutter_secure_storage / uuid / flutter_animate；keystore channel 一段）；「SharedPreferences 只留 UI 态」落字。
- **File: `docs/flutter-layering.md`** — 写上 import 边界规则与两个 CI 门脚本。
- **File: `docs/impl/README.md`** — 删本行；本文件随合入删除。
- 验证：`cargo fmt --check && cargo clippy -p kim-sdk -p kim-client-ffi --all-targets -- -D warnings`；`cd sdk/mobile && dart run custom_lint && flutter analyze --fatal-infos --fatal-warnings && flutter test`；`tools/check_flutter_boundaries.sh && tools/check_page_size.sh`；桌面 `flutter build macos` 走登录 / 发消息 / agent 权限卡 / 人设保存；Android 构建 + OTA 打包脚本跑一遍（`.so` 未改名，OTA 面不变）；`rg -l "flutter_secure_storage|package:uuid|flutter_animate" sdk/mobile` 为空。

## Architectural Notes

- **无 semver 对象。** workspace 内消费；`libkim_client_ffi.so` 名不变，Android OTA 通道不受影响。
- **密钥归属不变。** Rust 定义键名与生命周期、并发与排队；平台 channel 只是执行器换了实现。SecretRequest 协议、`platformBootstrap` 顺序不动。
- **Keychain 内容兼容。** `kim.jwt` / `account.<keyRef>` 由 channel 原样读写同一 keychain item；新实现写入格式与插件不同没关系——没有发布版用户，开发机首次升级重新登录一次可接受（与 ffi-ownership「库文件删了重开」同一前提）。
- **不新增 FFI 面。** catalog 缓存优先并进 `fetch_models`；书签走 grant registry 现有方法。Phase 3 若必须加方法，先回本文件补一行再实现。
- **explicitly not changed：** `kim-agent-ffi` 桌面边界；`session/` 内核文件；`HomeShell` 三栏与 `kimPushPage` 转场；`kim_lints` 的 `avoid_set_state`；l10n 双语 ARB 与 `Copy` 查表模式（`Copy` 是 `AppLocalizations` 的 zh 快捷取用，不是第三套字符串源，不在此收敛）。
- **删除的跨依赖：** flutter_secure_storage（含其 Android BiometricPrompt / iOS LocalAuthentication 传递依赖）、uuid、flutter_animate、cupertino_icons。新增：0 个三方包，2 个原生 channel 实现（已有 `kim.workspace` / `ota` 先例）。

## File Change Summary

- `.github/workflows/ci.yml` — sdk-mobile job 增两个门脚本
- `crates/kim-client-ffi/src/api/client.rs` — clientId 生成回填；fetch_models 带缓存（或最小新方法）
- `crates/kim-sdk/src/lib.rs` — enqueue None 生成 uuid；settings 增 catalog 缓存
- `crates/kim-sdk/src/store/agent.rs` — grant registry 增 bookmark blob
- `docs/flutter-layering.md` — import 边界与 CI 门
- `docs/impl/README.md` — 未合入表增本行，合入时删
- `docs/mobile-client.md` — 依赖与 keystore channel 落字
- `sdk/mobile/android/app/src/main/kotlin/.../MainActivity.kt` — kim.keystore 实现
- `sdk/mobile/ios/Runner/AppDelegate.swift`、`macos/Runner/AppDelegate.swift` — kim.keystore 实现
- `sdk/mobile/lib/bridge/kim_ports.dart` — re-export 投影类型
- `sdk/mobile/lib/core/secret_store_executor.dart` — channel 替换插件
- `sdk/mobile/lib/features/agent/agent_profiles.dart` — 删迁移代码；拆 notifier/data
- `sdk/mobile/lib/features/agent/catalog.dart` — 删 prefs 缓存
- `sdk/mobile/lib/features/agent/workspace_access.dart` — 书签下沉 grant registry
- `sdk/mobile/lib/features/chats/inbox.dart` — 整删
- `sdk/mobile/lib/features/chats/providers/chat_session.dart` — clientId 传 null；toast/redirect 上移
- `sdk/mobile/lib/main.dart` — 删 readLegacyDevicePrefs
- `sdk/mobile/lib/models/**` — models.dart 拆目录 + barrel
- `sdk/mobile/lib/design/message_row/**`、`design/composer/**` — kim_bubble / kim_composer 拆件
- `sdk/mobile/lib/features/agent/{views,providers,widgets,data}/**` — 目录化 + 三页拆分
- `sdk/mobile/pubspec.yaml` — 删 4 个依赖
- `sdk/mobile/test/**` — import 路径跟随；fake_kim 不变
- `tools/check_flutter_boundaries.sh` — 新增
