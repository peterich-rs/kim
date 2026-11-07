# 按第一版重切 Flutter FFI 边界

| 字段 | 值 |
| --- | --- |
| 状态 | 本地已实现，尚未合入主干 |
| 分支 | `refactor/ffi-ownership` |
| 对照 | [ffi-oo-contract.md](../ffi-oo-contract.md)；[mobile-client.md](../mobile-client.md)；[flutter-layering.md](../flutter-layering.md) |
| 前提 | App 没有已发布安装包。本地 SQLite、FFI 签名、Dart 模型、Keychain 里尚不存在的键，都按第一版重来。不写升级、不写 `import_legacy_prefs`、不清旧行 |

协议线不动。Chat 与 Web 仍用 `kim-protocol` 的整型：`MESSAGE_TYPE_*`、`INBOX_KIND_*`，以及线上的 `extra` JSON。这些整型只出现在 `kim-client` 编解码处。`kim-sdk` 与 FFI 用闭集，入库时再编码。

## Breaking Change Notice

没有仓库外的 FFI 消费者。同一改动里改完全部调用方：

1. 删除 `KimUiHandle.command`、`UiCommand`、`watchAgentRun`、`submitAgentRun`、`botReply` / `botPending` / `botTyping`、`friendIncoming`、`agentSessionFile`、`markRead`（转发）、路径版 `mediaUpload`、`httpOriginFromWs`、`specJsonToBlob` / `specBlobToJson`、`importLegacyPrefs`。
2. 删除 `handles.rs` 的五个句柄结构体。Dart 只有 `KimUiHandle`。`ConversationPort` 留在 Dart，作为带 `ThreadKind` 的便利包装。
3. 删除 `kim-agent-ffi` 的 `session.rs` 与 Dart `AgentSessionPort`。桌面 catalog crate 保留（见决定 2）。
4. `enqueueMessage` 以及所有带线程种类的方法改为 `ThreadKind`。发送内容改为 `OutgoingContent`。`MessageView.kind: i32` 改为 `MediaKind`。`ThreadView` 增加结构化 `preview`。`Person.relation` 改为 `Relation`。
5. `SessionUpdate::TokenRenew { token, exp }` 从 FFI 与 `kim-sdk` 投影中删除。续签写入 Keychain 发生在投影之前，token 字符串不出 FFI。
6. `AgentProfile` 不再带 `body_blob` / `body_json`。FFI 只收发已经过 `kim-agent-codec` 校验的 JSON 文档。本地表只留 blob 列。`ProviderAccount.models_json` 改为 `Vec<String>`，`deleted_at` 不出 FFI。
7. 开发机上已有的 `kim-cache.db` 直接删掉重开。`migrate.rs` 不再为旧版本补列。

## Feasibility Assessment

生产 turn 不经过这些死面。桌面在 `KimUiHandle::attach_store` 里调用 `kim_desktop_runtime::install`（`crates/kim-client-ffi/src/api/client.rs`）。手机不编 `kim-agent-ffi`（`sdk/mobile/hook/build.dart` 的 `_desktopAgentHost`）。`watch_agent_run` / `submit_agent_run` / `command` 的调用只在 `sdk/mobile/test/support/legacy_agent_drive.dart` 与 `fake_kim.dart`。`FfiAgentRuntime` 只被这对 FFI 使用（`crates/kim-sdk/src/agent/runtime.rs`）。

气泡路径已经不跑 `kindFromWire`。`kimChatFrom` 只把 `MessageView.kind` 的 `2` / `4` 映回图片 / 视频（`sdk/mobile/lib/features/session/kim_session.dart`）。列表预览仍对 `lastBody` 做 `previewSnippet`（`sdk/mobile/lib/design/conversation_tile.dart`）。时间在入库时已经过 `store::send_time_ms`（`crates/kim-sdk/src/store/mod.rs`）；Dart `sendTimeMs` 是第二遍。`TokenRenew` 的 Keychain 写入在 `spawn_session_bridge` 里匹配该变体（`crates/kim-sdk/src/lib.rs`），不能随变体一起删。

`kim-agent-ffi` 依赖 `kim-agent-host` 且打开 `codex`。把它并进 `kim-client-ffi` 会让手机 `.so` 链上 host。catalog 留在桌面 crate。

Fully feasible. 工作量在投影闭集、人设文档改由 codec 校验，以及测试一次迁到 `KimClientPort` fake。不存在要保留的外部 ABI。

## Current Surface Inventory

### (a) 整段删除

| 面 | 位置 | 现状 |
| --- | --- | --- |
| `watch_agent_run` / `submit_agent_run` | `crates/kim-client-ffi/src/api/client.rs` | 仅测试驱动 |
| `UiCommand` + `command()` | `client.rs`、`types.rs` | `AgentRespondPermission` / `AgentAbortTurn` 返回 `InvalidArgument`；`SendMedia` 把线程 `kind == 0` 当成内容种类 |
| `FfiAgentRuntime`、`install_mobile_agent`、`subscribe_agent_run`、`submit_agent_run` | `crates/kim-sdk/src/agent/runtime.rs`、`lib.rs` | 无生产触发 |
| `bot_reply` / `bot_pending` / `bot_typing`、`KimTalkResult`、`KimBotPendingItem` | `client.rs` | 仅 fake |
| `friend_incoming` | `client.rs` | 通讯录读 snapshot 的 `relation` |
| `agent_session_file`、`mark_read`、路径版 `media_upload` | `client.rs` | `lib/` 无调用；`MediaHandle.upload` 还在调用路径版，句柄一并删 |
| `http_origin_from_ws` 导出 | `crates/kim-client-ffi/src/api/auth.rs` | 无调用。`KimAuth::create` 内部自己派生 origin |
| `Bot`、`MessagePage`、`CommandAck` | `types.rs` | `CommandAck` 只服务 `command()` |
| 五个 Handle 及 `inbox()` / `conversation()` 等工厂 | `handles.rs` | Dart 用 `ConversationPort`，且该端口把进房 `kind` 写成 `0` |
| `spec_json_to_blob` / `spec_blob_to_json` | `simple.rs` | `agent_profiles.dart` 往返存储格式 |
| `import_legacy_prefs` | `client.rs`、`main.dart` | 为不存在的旧安装包准备的一次性移交 |
| `session.rs`（含 `session_open`、`AgentSession`、`AgentUiEvent`） | `crates/kim-agent-ffi/src/api/session.rs` | 5 把 `Mutex`、`Result<_, String>`、宽 struct。生产不打开。`list_builtin_profiles` / `list_bundled_providers` 在文件末尾，迁到 `catalog.rs` |
| Dart `AgentSessionPort` | `sdk/mobile/lib/bridge/goose_bridge.dart` | 测试与 `agent_permission.dart` 的空挂钩 |

### (b) 第一版要收成闭集的投影

| 泄漏 | 现在 | 目标 |
| --- | --- | --- |
| 线程种类 | FFI `kind: i32` 散布在 enqueue、已读、可见性、进房、离开、typing；Dart `? 1 : 0` 与 `ConversationPort` 的 `kind: 0` | `ThreadKind` |
| 消息种类 | 库内字符串，`kind_from_name` 压成 `i32`，未知（含语音 `3`）变成文本；Dart 再映回 enum | `MediaKind`，替换 `kind` 字段，不并列第二份 |
| 列表预览 | `previewSnippet` 嗅探 URL | `ThreadPreview` |
| 时间 | `store::send_time_ms` 已在 `apply_talk` / `load_threads` 使用；Dart `format.dart` 再跑一遍；`search_messages` 把 `kind` 写成 `1` | 只在吃进协议 `send_time` 时归一；投影字段已是毫秒 |
| 关系 | `Person.relation` 字符串 `'incoming'` / `'outgoing'` / `'friend'` | `Relation` |
| 发送状态 | FRB 已是 `SendStatus`，桥接层翻回字符串 | 端口直接用枚举 |
| 资料种类 | `_profileKind` 接受 `'bot'`、`'2'` 与数字 | `ProfileKind`，投影已是枚举 |
| 续签 | 变体携带 token，FFI 序列化后 Dart 忽略 | 投影前 `write_global(KEY_JWT)`，变体消失 |
| 人设 | Dart `toJson` 是 schema；FFI 同时看见 `body_json` 与 `body_blob` | codec 校验，表里只留 blob，FFI 只见 JSON 文档 |
| 供应商模型 | `models_json: String` | `Vec<String>` |
| flags | FFI 手解 JSON | `kim-sdk` 结构体 |
| 技能目录 | `skill_portable_list(project_root)` | 读 workspace grant，调用不再带路径 |
| 工具卡片 | `AgentToolCard.parse` 对消息正文 `jsonDecode`；`SessionUpdate::AgentCard` 的 `card_type` / `state` 仍是字符串 | 入库时识别信封，`MessageView` 带结构化 `AgentCard` |
| 进房结果 | 端口返回 `List<Map<String, dynamic>>` | `Vec<RoomMember>` |

### (c) 已经在 Rust、不要再抄一份

- `store::send_time_ms`：阈值与 Dart `sendTimeMs` 相同。不在 `timeline.rs` 新增 `normalize_ms`。
- `parse_image_extra`：入库已拆宽高。读侧不再解析 `{"w","h"}`。
- `kim_client::OutgoingContent`：线上已有 Text / Image / Voice / Video。`kim-sdk` 的 `OutgoingPayload` 缺 Voice，第一版补上。

## Design

### 关键决定

1. **死面整删，不整改。** `FfiAgentRuntime` 的 oneshot、`session.rs` 的五把锁、`UiCommand` 与直连方法重复。桌面 turn 已在 `kim-desktop-runtime`。否决「留着改锁」：没有生产调用方。
2. **保留桌面 `kim-agent-ffi`，只留 catalog。** 否决并进 `kim-client-ffi`：该 crate 依赖带 `codex` feature 的 `kim-agent-host`，手机钩子今天靠不编第二个 crate 来避开 host。第一版继续：手机只链 `libkim_client_ffi`；桌面再链 catalog crate。`session.rs` 删除。`kim_agent_ffi` 仍然不依赖 `kim-client`。
3. **一个 `KimUiHandle`。** 五个句柄不拥有订阅，`ConversationHandle` 把 `kind` 写成 `0`。否决补全句柄，也否决把 `KimBridge` 拆成三个类型。合入时把本文件的闭集写回 `ffi-oo-contract.md`。
4. **闭集定义在 `kim-sdk`，FFI 复用同一类型。** 协议整型只在 `kim-client` 的 wire 编解码和写入 SQLite 时出现。否决「FFI 用 enum、sdk 继续 `i32`」：`SendMedia` 把线程种类当成内容种类，就是两套哨兵。否决为了「零迁移」保留读侧 URL 嗅探。开发库删文件重开；`CREATE TABLE IF NOT EXISTS` 不会改旧文件，所以 `migrate.rs` 见到低于当前版本的库就删掉客户端文件再按新 schema 建表。这不是用户数据迁移。
5. **媒体种类在入库时写定，读侧信任列。** 分类输入是协议 `msg_type`、`extra`、正文 URL。结果写入 `messages.kind` 文本（`text` / `image` / `video` / `voice` / `card`）。`kind_from_name` 删除，不再把未知收成文本。语音没有发送入口也可以；收到 `MESSAGE_TYPE_VOICE` 投影为 `Voice`，气泡用 `Copy` 占位。否决在 `MessageView` 上同时留 `kind: i32` 和 `media`。
6. **列表预览是结构化的。** `ThreadPreview::Text { snippet }`、`Media { kind }`、`System { text }`。Rust 截断文本、不把媒体 URL 放进预览。图片 / 视频 / 语音的展示文案留在 Dart `Copy`。系统消息沿用现有「原样显示」规则。否决 `preview: String` 再让 Dart 按种类分支。
7. **时间只在协议边界归一一次。** `apply_talk` 与收件箱投影继续调用现有 `store::send_time_ms`，因为对端和 Web 仍可能给出秒或纳秒。本端写出的 `at` 一律是毫秒。删除 Dart `sendTimeMs`。`search_messages` 填真实 `MediaKind`，不再写死文本。
8. **续签不进入投影。** `spawn_session_bridge` 在 `SessionEvent::TokenRenew` 上直接 `write_global(KEY_JWT, token)`，然后丢弃该事件。删除 `SessionUpdate::TokenRenew`。FFI 流里不再出现 token。
9. **人设 schema 只有 `kim-agent-codec`。** 本地 `agent_profiles` 去掉 `body_json`。`upsert` 收 JSON 字符串，codec 失败则 `ApiFailure`，成功则只写 blob。`list` 在返回前 `blob_to_json`。服务端 spec 同步继续传 blob，那是协议载荷，不出 FFI。否决把 host 的整棵 profile struct 再镜像成一套 FRB 类型：表单字段多，文档格式已经是 codec 的 JSON。Dart `agent_profile_model.dart` 是表单状态，往返必须被 codec 接受，另有一轮测试钉住。`list_builtin_profiles` 同样返回这份 JSON，不再手写第二套 schema。
10. **工具卡片在入库时变成 `AgentCard`。** 正文里的私有 JSON 不再交给 Dart `jsonDecode`。`card_type` 与 `state` 用枚举。实时 `SessionUpdate::AgentCard` 用同一结构。
11. **Keychain 键名不改。** `kim.jwt`、`account.<keyRef>` 没有第二套存储要兼容，改名没有产品收益。删除的是 `importLegacyPrefs` 这条迁移入口，不是键名。
12. **纯 UI 状态留在 Dart。** theme、dest、avatar、`notificationsAsked` 仍在 SharedPreferences。

### 核心类型

```rust
// crates/kim-sdk/src/model.rs（新；FFI pub use 这些类型）
pub enum ThreadKind { User, Group }

pub enum MediaKind { Text, Image, Video, Voice, Card }

pub enum Relation { Friend, Incoming, Outgoing }

pub enum ProfileKind { User, Bot }

pub enum ThreadPreview {
    Text { snippet: String },
    Media { kind: MediaKind },
    System { text: String },
}

pub enum OutgoingContent {
    Text { body: String },
    Image { path: String, mime: String, width: i32, height: i32, byte_size: i64 },
    Video { path: String, byte_size: i64 },
    Voice { path: String, byte_size: i64 },
}
```

`MessageView.kind: MediaKind` 替换 `i32`。`ThreadView.kind: ThreadKind`，并增加 `preview: ThreadPreview`。`last_body` 仍是原文，供搜索与调试；列表 UI 只读 `preview`。

`Person.relation: Relation`，`Person.kind: ProfileKind`。`RoomMember` 原样返回给 Dart，删除 `List<Map>`。

```rust
// crates/kim-sdk/src/timeline.rs 的 AgentCard 收紧
pub enum AgentCardType { Tool, ActionRequired }
pub enum AgentCardState { Pending, Ok, Error }

pub struct AgentCard {
    pub call_id: String,
    pub name: String,
    pub card_type: AgentCardType,
    pub state: AgentCardState,
    pub preview: String,
    pub ok: bool,
}
```

`MessageView` 在 `MediaKind::Card` 时带 `card: Option<AgentCard>`。其他种类 `card` 为 `None`。

```rust
// KimUiHandle::enqueue_message
pub async fn enqueue_message(
    &self,
    dest: String,
    kind: ThreadKind,
    content: OutgoingContent,
    client_id: Option<String>,
) -> Result<CommandReceipt, ApiFailure>;
```

`mark_thread_read`、`mark_conversation_read`、`set_conversation_visibility`、`room_enter`、`room_leave`、`send_typing` 的 `kind` 全部改为 `ThreadKind`。内部匹配到 `INBOX_KIND_USER` / `INBOX_KIND_GROUP` 只发生在调用 `kim-client` 的那一层。

```dart
// 端口不再把 SendStatus 翻成字符串，不再接受平铺的 localPath/mime/width
await client.enqueueMessage(
  dest: dest,
  kind: ThreadKind.group,
  content: OutgoingContent.image(path: p, mime: mime, width: w, height: h, byteSize: n),
  clientId: id,
);

final ConversationPort room = client.conversation(dest, ThreadKind.group);
await room.enter(); // 使用构造时的 ThreadKind，返回 List<RoomMember>
```

人设：

```rust
pub async fn upsert_agent_profile(&self, document_json: String) -> Result<(), ApiFailure>;
pub async fn list_agent_profiles(&self) -> Result<Vec<AgentProfileRow>, ApiFailure>;
// AgentProfileRow 含 profile_id、nickname、server_account、document_json、placement、updated_at
// 无 blob、无 deleted_at
```

### 调用形态

```text
Flutter
  platformBootstrap 一次（目录根、版本、模拟器）
  Keychain 执行器
  KimUiHandle：意图 + 订阅投影
  桌面另订阅 kim-agent-ffi catalog（厂商、技能、内置人设 JSON）
        |
kim-sdk
  ThreadKind / MediaKind / Relation / ThreadPreview / AgentCard
  入库时分类、send_time_ms、codec json↔blob
  TokenRenew 在投影前写入 SecretStore
        |
kim-client
  仅在 wire 上把闭集编成协议整型
        |
桌面 kim-desktop-runtime
  HostAgentRuntime 进程内跑 turn
  手机不链这条，也不链 kim-agent-ffi
```

## Phased Implementation

每相结束 `cargo check -p kim-sdk -p kim-client-ffi -p kim-agent-ffi` 与 `cd sdk/mobile && flutter analyze` 通过。测试迁移集中在 Phase 2 完成之后再跑 `flutter test`。FRB：`flutter_rust_bridge generate --config flutter_rust_bridge.yaml`；catalog 变更后再跑 `--config flutter_rust_bridge.agent.yaml`。

### Phase 1: 删除死面，并拦住 token

- **File: `crates/kim-sdk/src/lib.rs`** — 删除 `install_mobile_agent`、`subscribe_agent_run`、`submit_agent_run`、`inner.ffi_runtime`。`spawn_session_bridge` 对 `SessionEvent::TokenRenew` 直接 `write_global(KEY_JWT, token)`，不再构造 `SessionUpdate`。`session_update_from_event` 对该事件返回 `None`。
- **File: `crates/kim-sdk/src/timeline.rs`** — 删除 `SessionUpdate::TokenRenew`。
- **File: `crates/kim-sdk/src/agent/runtime.rs`** — 删除 `FfiAgentRuntime`。保留 `ScriptedRuntime`。
- **File: `crates/kim-sdk/src/agent/mod.rs`** — re-export 去掉 `FfiAgentRuntime`。`MobileAgent::new` 签名不变。
- **File: `crates/kim-client-ffi/src/api/client.rs`** — 删除 `command`、`watch_agent_run`、`submit_agent_run`、`bot_reply`、`bot_pending`、`bot_typing`、`friend_incoming`、`agent_session_file`、`mark_read`、路径版 `media_upload`、`import_legacy_prefs`、`KimTalkResult`、`KimBotPendingItem`、`empty_ack`、`ack_from_receipt`。
- **File: `crates/kim-client-ffi/src/api/types.rs`** — 删除 `UiCommand`、`Bot`、`MessagePage`、`CommandAck`、`AgentRunRequest`、`AgentRunResult`、`SessionUpdate::TokenRenew` 镜像及对应 `From`。
- **File: `crates/kim-client-ffi/src/api/handles.rs`** — 删除五个句柄与工厂。保留 `store_agent_secret`、`fetch_models`、`preview_profile`、`respond_agent_permission`、`watch_agent_permission`、`watch_agent_ui`，改为 `KimUiHandle` 的方法（若已在 `client.rs`，这里只留权限 / UI 扩展，不留空壳类型）。
- **File: `crates/kim-client-ffi/src/api/auth.rs`** — 删除 `http_origin_from_ws` 导出。内部派生保留。
- **File: `crates/kim-client-ffi/src/api/simple.rs`** — 删除两个 spec 转换。`init_app` 保留。
- **File: `sdk/mobile/lib/bridge/kim_ports.dart`**、**`kim_client_bridge.dart`**、**`test/support/fake_kim.dart`** — 同步删除上述方法。`link.dart` 去掉 `SessionUpdate_TokenRenew` 分支。
- **File: `sdk/mobile/lib/main.dart`** — 删除 `importLegacyPrefs` 调用。
- **File: `sdk/mobile/test/support/legacy_agent_drive.dart`** — 整文件删除。仍引用 `AgentSessionPort` 的测试留到 Phase 2，本相只保证不再调用已删的 client FFI。
- 重生成 client FRB。

### Phase 2: 删除 agent session，测试只迁一次

- **File: `crates/kim-agent-ffi/src/api/session.rs`** — 整文件删除。`list_builtin_profiles`、`list_bundled_providers` 移到 `catalog.rs`，仍返回 host 模板的 JSON 字符串（Phase 4 起这些字符串必须过 codec）。
- **File: `crates/kim-agent-ffi/src/api/mod.rs`** — 去掉 `session` 模块。
- **File: `crates/kim-agent-ffi/tests/`** — 删除或改写只覆盖 `session_open` 的集成测试。host 行为仍由 `kim-agent-host` 与 `kim-desktop-runtime` 的测试覆盖。
- **File: `sdk/mobile/lib/bridge/goose_bridge.dart`** — 删除 `AgentSessionPort` 与 session 的 export。保留 catalog 封装。
- **File: `sdk/mobile/lib/features/agent/agent_permission.dart`** — 删除 `_sessions` / `attach` / `detach`。`respond` 只走 `respondAgentPermission`。
- **File: `sdk/mobile/test/support/fake_kim.dart`** — 可注入 `SessionUpdate_AgentTurn` 与 timeline。
- **File: `sdk/mobile/test/state/chat_agent_test.dart`**、**`test/agent/drive_session_stop_reason_test.dart`**、**`test/agent/kim_im_tools_test.dart`**、**`test/agent/skill_host_paths_test.dart`**、**`test/agent/agent_permission_test.dart`**、**`test/widgets/agent_action_bubble_test.dart`** — 改为 fake 注入。断言 stop reason、typing、权限应答、气泡渲染。不再 import `rust_agent` session。
- 重生成 agent FRB。此后 `flutter test` 必须通过。

### Phase 3: 闭集替换哨兵

- **File: `crates/kim-sdk/src/model.rs`** — 新增决定里的枚举。`ThreadKind` ↔ `INBOX_KIND_*`、`MediaKind` ↔ `MESSAGE_TYPE_*` 的函数放在这里，供 wire 与 store 使用。
- **File: `crates/kim-sdk/src/command.rs`** — `SendMessageCommand.kind` 改为 `ThreadKind`。`OutgoingPayload` 与 `OutgoingContent` 对齐，补上 `Voice`。
- **File: `crates/kim-sdk/src/timeline.rs`** — `MessageView.kind` 改为 `MediaKind`。`ThreadView.kind` 改为 `ThreadKind`，增加 `preview: ThreadPreview`。删除 `kind_from_name` 的「其余都是文本」。
- **File: `crates/kim-sdk/src/store/messages.rs`** — `apply_talk` 用单一 `classify`（`msg_type` + extra + URL）写入 kind 文本，并继续 `send_time_ms`。宽高只来自 `parse_image_extra`。读行直接映射 `MediaKind`，不二次嗅探。
- **File: `crates/kim-sdk/src/store/threads.rs`** — `load_threads` 填 `ThreadPreview`。系统消息走 `System`，媒体走 `Media`，文本截断。
- **File: `crates/kim-sdk/src/lib.rs`** — `search_messages` 构造 `MessageView` 时带上真实 `MediaKind`，`at` 使用已经归一的列。
- **File: `crates/kim-sdk/src/contacts.rs`**、**`store/contacts.rs`** — 关系用 `Relation`。SQLite 仍可存 `friend` / `incoming` / `outgoing` 文本，读写各一处匹配。
- **File: `crates/kim-client-ffi/src/api/types.rs`** — `pub use` sdk 枚举与视图字段。删除手写 `kind: i32`。`From` 只做 sdk 视图到 FFI 视图的字段搬移；种类字段不再转成整数。
- **File: `crates/kim-client-ffi/src/api/client.rs`** — 决定 4 列出的方法改签名。`enqueue_message` 匹配 `OutgoingContent` 构造 payload。删除两处 `too_many_arguments`。
- **File: `sdk/mobile/lib/features/session/kim_session.dart`** — `kimChatFrom` / `kimThreadFrom` 读枚举与 `preview`，删除 `kind == 1` / `kind == 2`。
- **File: `sdk/mobile/lib/bridge/conversation_port.dart`** — 构造函数收 `ThreadKind`。`enter` / `leave` / `setTyping` / `markRead` 使用它。`enter` 返回 `List<RoomMember>`。
- **File: `sdk/mobile/lib/bridge/kim_ports.dart`**、**`kim_client_bridge.dart`** — 删除 `_wire`、五处 `? 1 : 0`、`_sendStatusLabel`、`_profileKind`。`KimCommandReceipt.sendStatus` 改为 `SendStatus`。
- **File: `sdk/mobile/lib/features/chats/providers/chat_session.dart`**、**`features/agent/kim_im_tools.dart`** — 发送处构造 `OutgoingContent`。
- **File: `sdk/mobile/lib/features/contacts/contacts.dart`** — `switch` 匹配 `Relation`。
- **File: `sdk/mobile/lib/core/image_extra.dart`** — 删除 `encodeImageExtra`、`parseImageExtra`、`isMediaUrl`、`kindFromWire`、`previewSnippet`、`previewBody`、`mediaHost`。
- **File: `sdk/mobile/lib/core/format.dart`** — 删除 `sendTimeMs`。`dateTimeFromEpoch` 把参数当作毫秒。
- **File: `sdk/mobile/lib/design/conversation_tile.dart`** — 预览读 `thread.preview`，媒体与系统文案查 `Copy`。
- **File: `sdk/mobile/test/media_types_test.dart`** — 改为断言 Rust 投影（或删除已无 Dart 函数的用例）。分类边界改到 `kim-sdk` 单测。
- **File: `crates/kim-sdk/src/store/schema.rs`**、**`migrate.rs`** — `messages.kind` 允许 `voice` 与 `card`。打开低于当前 `user_version` 的库时删除该 sqlite 文件并按新 `CREATE` 重建。不新增 `ensure_column` 补丁。

单测：`classify` 覆盖 `msg_type` 1/2/3/4、media host URL、带 query 的扩展名、extra 缺宽高、卡片信封。`send_time_ms` 沿用现有四档，不复制函数。`ThreadPreview` 覆盖系统消息、图片 URL、普通文本截断。

### Phase 4: 人设、账号、flags、技能路径

- **File: `crates/kim-sdk/src/store/schema.rs`** — `agent_profiles` 去掉 `body_json`。`placement` 列可留文本，Rust 侧用小枚举（现有默认值 `local`）映射。
- **File: `crates/kim-sdk/src/store/agent.rs`** — `AgentFlagRow { multi_profile, server_identity }` 的 get/set，列里仍是 JSON。profile 读写只碰 blob。`models_json` 列在 store 内解析成 `Vec<String>`。
- **File: `crates/kim-sdk/src/lib.rs`** — `upsert_agent_profile` 调 `kim_agent_codec::json_to_blob`。`list_agent_profiles` 返回前 `blob_to_json` 填 `document_json`。远程 spec 同步继续写 blob。空 blob 仍跳过上行。
- **File: `crates/kim-client-ffi/src/api/client.rs`** — `agent_flags` / `set_agent_flags` 改为结构体直传，删除手解 JSON。profile / account 的 `From` 不再暴露 blob、`deleted_at`、`models_json` 字符串。
- **File: `crates/kim-agent-ffi/src/api/catalog.rs`** — `Vendor.group` 改为枚举。`skill_portable_list` 去掉 `project_root`，改为读已登记的 workspace grant（路径来自 `workspace_grant_register`）。`list_builtin_profiles` 的每个字符串先过 codec，失败的模板不返回。
- **File: `sdk/mobile/lib/features/agent/agent_profiles.dart`** — 删除 `specJsonToBlob` / `specBlobToJson`。提交文档 JSON，展示用返回的 `document_json`。
- **File: `sdk/mobile/lib/features/agent/provider_accounts.dart`** — 模型列表用 `List<String>`，去掉为 FFI 准备的 `jsonEncode`。
- **File: `sdk/mobile/lib/bridge/goose_bridge.dart`**、**`features/agent/skills_catalog.dart`** — `skillPortableList()` 无路径参数。
- **File: `sdk/mobile/test/`** — 增加一份人设样例：`agent_profile_model` 的 `toJson` 经 `json_to_blob` 再 `blob_to_json` 成功。codec 拒绝的文档，`upsert` 返回 `ApiFailure`。

### Phase 5: 工具卡片不再由 Dart 解析正文

- **File: `crates/kim-sdk/src/store/messages.rs`** — `classify` 识别卡片信封时写 `card`，并把字段填进 `AgentCard`。无法识别的正文保持 `Text`。
- **File: `crates/kim-sdk/src/timeline.rs`** — `AgentCard` 用 Phase 3 的枚举。`SessionUpdate::AgentCard` 使用同一结构。
- **File: `sdk/mobile/lib/design/agent_action_bubble.dart`**、**`design/kim_bubble.dart`** — 删除 `AgentToolCard.parse` / `encode`。气泡读 `MessageView.card`。
- **File: `sdk/mobile/lib/models/models.dart`** — `KimMsgKind.agentCard` 改为跟随 `MediaKind.card`。删除只为旧 JSON 缓存服务的 `kind` 字符串解析；若 `toJson` 仍给测试夹具用，种类字段用枚举名，且不作为运行时真源。

### Phase 6: 把落地后的边界写回专题文档

- **File: `docs/ffi-oo-contract.md`** — 写上闭集、`ThreadPreview`、token 不出 FFI、codec 是人设 schema、桌面 catalog crate 的链接理由。删掉「步骤见切片」这句。
- **File: `docs/mobile-client.md`** — 投影字段与「开发库版本不符则重建」写进 Flutter 壳一节。
- **File: `docs/impl/README.md`** — 删除本切片在「未合入」的行。本文件在合入主干的那个提交里删除。

### Phase 7: 验证

- `cargo fmt --check && cargo clippy -p kim-sdk -p kim-client-ffi -p kim-agent-ffi --all-targets -- -D warnings`
- `cargo test -p kim-sdk -p kim-client-ffi -p kim-agent-ffi -p kim-desktop-runtime`
- `cd sdk/mobile && dart run custom_lint && flutter analyze && flutter test`
- 桌面 `flutter build macos`：登录、发文本、发图片、跑一轮 agent（权限卡片走 `respondAgentPermission`）、人设保存被 codec 接受。
- Android 构建：不生成 `libkim_agent_ffi`；IM 收发与 `mediaUploadBytes` 可用。
- `rg` 确认 `lib/` 与 `frb_generated.dart` 不再出现 `UiCommand`、`AgentUiEvent`、`body_blob`、`importLegacyPrefs`、`TokenRenew`、`sendTimeMs`、`kindFromWire`。

## Architectural Notes

- **没有 semver 对象。** workspace 内删 `FfiAgentRuntime` 与 session API。手机 OTA 的 `.so` 名仍是 `libkim_client_ffi.so`。
- **协议与 Web SDK 不改。** 整型留在 wire。本端不再靠读侧启发式理解自己写出的数据。
- **SQLite 对开发库是破坏性的。** 低于当前 `user_version` 的客户端文件删除重建。不写 v10 补列。服务端 Postgres 不在本切片。
- **`kim-agent-ffi` 仍是桌面专用。** 原因是链接边界，不是历史包袱。catalog 之外的 session API 删除。
- **密钥。** `TokenRenew` 的副作用留在 `spawn_session_bridge`，先于任何 `SessionUpdate`。键名常量不动。
- **不改。** `ScriptedRuntime`、`kim-desktop-runtime` 的 turn 循环、`platform_bootstrap`、SecretStore 回调形态、UI prefs、Web `isRetryable`。
- **锁与 `Result<_, String>`。** 随 `session.rs` 删除消失，不单独立项。

## File Change Summary

- `crates/kim-agent-ffi/src/api/catalog.rs` — 接收 builtin 列表；`Vendor.group` 枚举；`skill_portable_list` 不再收路径
- `crates/kim-agent-ffi/src/api/mod.rs` — 去掉 session 模块
- `crates/kim-agent-ffi/src/api/session.rs` — 整删
- `crates/kim-agent-ffi/src/frb_generated.rs` — 重生成
- `crates/kim-agent-ffi/tests/` — 去掉 session 集成测试
- `crates/kim-client-ffi/src/api/auth.rs` — 去掉 origin 导出
- `crates/kim-client-ffi/src/api/client.rs` — 删死面；方法改用闭集；flags / profile 直传
- `crates/kim-client-ffi/src/api/handles.rs` — 删五个句柄
- `crates/kim-client-ffi/src/api/simple.rs` — 删 spec 转换
- `crates/kim-client-ffi/src/api/types.rs` — 复用 sdk 闭集；删 `UiCommand` 与 token 变体
- `crates/kim-client-ffi/src/frb_generated.rs` — 重生成
- `crates/kim-desktop-runtime/src/lib.rs` — 注释里去掉 `FfiAgentRuntime`（行为不变）
- `crates/kim-sdk/src/agent/mod.rs` — re-export 收缩
- `crates/kim-sdk/src/agent/runtime.rs` — 删 `FfiAgentRuntime`
- `crates/kim-sdk/src/command.rs` — `ThreadKind` + `Voice`
- `crates/kim-sdk/src/contacts.rs` — `Relation`
- `crates/kim-sdk/src/lib.rs` — 删 agent-run API；续签先写密钥；profile 经 codec；搜索填真实种类
- `crates/kim-sdk/src/model.rs` — 新：闭集与协议整型的唯一映射
- `crates/kim-sdk/src/store/agent.rs` — 结构化 flags；blob-only profile；模型列表
- `crates/kim-sdk/src/store/contacts.rs` — `Relation` 读写
- `crates/kim-sdk/src/store/messages.rs` — 入库分类，含语音与卡片
- `crates/kim-sdk/src/store/migrate.rs` — 旧版本客户端库删除重建
- `crates/kim-sdk/src/store/schema.rs` — 去掉 `body_json`；kind 文本包含 `voice` / `card`
- `crates/kim-sdk/src/store/threads.rs` — `ThreadPreview`
- `crates/kim-sdk/src/timeline.rs` — 视图字段改为闭集；删除 `TokenRenew`
- `docs/ffi-oo-contract.md` — 合入时写成落地后的边界
- `docs/impl/README.md` — 合入时去掉本行
- `docs/mobile-client.md` — 合入时写上投影与本地库重建
- `sdk/mobile/lib/bridge/conversation_port.dart` — 携带 `ThreadKind`；`enter` 返回 `RoomMember`
- `sdk/mobile/lib/bridge/goose_bridge.dart` — 删 `AgentSessionPort`；技能列表无路径
- `sdk/mobile/lib/bridge/kim_client_bridge.dart` — 删哨兵翻译
- `sdk/mobile/lib/bridge/kim_ports.dart` — 端口与投影类型对齐
- `sdk/mobile/lib/core/format.dart` — 删 `sendTimeMs`
- `sdk/mobile/lib/core/image_extra.dart` — 删启发式
- `sdk/mobile/lib/design/agent_action_bubble.dart` — 读结构化卡片
- `sdk/mobile/lib/design/conversation_tile.dart` — 读 `ThreadPreview`
- `sdk/mobile/lib/design/kim_bubble.dart` — 读结构化卡片
- `sdk/mobile/lib/features/agent/agent_permission.dart` — 只走 client FFI 应答
- `sdk/mobile/lib/features/agent/agent_profiles.dart` — 提交 codec 文档
- `sdk/mobile/lib/features/agent/kim_im_tools.dart` — `OutgoingContent`
- `sdk/mobile/lib/features/agent/provider_accounts.dart` — `List<String>` 模型
- `sdk/mobile/lib/features/agent/skills_catalog.dart` — 不再传 `projectRoot`
- `sdk/mobile/lib/features/chats/providers/chat_session.dart` — `OutgoingContent` 与 `ThreadKind`
- `sdk/mobile/lib/features/contacts/contacts.dart` — `Relation`
- `sdk/mobile/lib/features/session/kim_session.dart` — 投影映射
- `sdk/mobile/lib/features/session/link.dart` — 去掉 `TokenRenew` 分支
- `sdk/mobile/lib/main.dart` — 删 `importLegacyPrefs`
- `sdk/mobile/lib/models/models.dart` — 种类跟随 `MediaKind`
- `sdk/mobile/lib/src/rust/**`、`src/rust_agent/**` — FRB 重生成
- `sdk/mobile/test/support/fake_kim.dart` — 死方法删除，timeline 可注入
- `sdk/mobile/test/support/legacy_agent_drive.dart` — 整删
- `sdk/mobile/test/media_types_test.dart` — 跟随启发式删除
- `sdk/mobile/test/{agent,state,widgets}` — 六个测试改为 fake
