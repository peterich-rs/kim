# FFI 边界

Dart 的会话句柄只有 `KimUiHandle`（账号 HTTP 仍是 `KimAuth`）。桌面 turn 在 `kim-desktop-runtime` 里跑完，不经过 Dart 驱动。手机不编 `kim-agent-ffi`。手机 `.so` 经 `kim-agent-codec` 链上的 `kim-agent-host` 不带 `codex`；带 `codex` 的 host 只从桌面 catalog crate 进来。

## 可以跨边界

- 意图：发消息、进房、改设置、`respond(call_id, allow|deny)`
- 投影：时间线、会话列表、联系人、连接状态、权限卡片
- 这一个句柄
- 一次 `platformBootstrap`：四个目录根、版本号、模拟器标记

Keychain / Keystore 是执行器回调。Rust 定义键名（`kim.jwt`、`account.<keyRef>`），Dart 只执行读写。

theme、dest、avatar、`notificationsAsked` 留在 SharedPreferences。

## 不跨边界

- api key 明文。模型列表走 `fetchModels(vendor, baseUrl, keyRef)`
- profile blob。人设文档的 schema 在 `kim-agent-codec`；存储格式不出 FFI
- SQLite 路径、user agent、HTTP origin、skill 缓存路径。这些由 `Layout` 从 bootstrap 派生
- 为跑一轮 agent 把 profile、账号、overlay 在 Dart 与两条 FFI 之间搬来搬去
- 续签后的 JWT。`TokenRenew` 在投影之前写入 SecretStore

## 两个 crate

`kim-client-ffi` 是手机和桌面都链的 IM 边界。

`kim-agent-ffi` 只在桌面编。它依赖带 `codex` feature 的 `kim-agent-host`，所以不能并进手机 `.so`。它只提供 catalog（厂商、技能、内置人设）。它不依赖 `kim-client`。编排 crate 是 `kim-desktop-runtime`。

## 闭集

闭集定义在 `kim-sdk`（`crates/kim-sdk/src/model.rs`）。`kim-client-ffi` `pub use` 它们。Dart 用生成的枚举，不再把种类收成 `i32` 或关系字符串。协议整数（`MESSAGE_TYPE_*`、`INBOX_KIND_*`）只留在 `kim-client` 编解码和 sqlite 绑定。

- `ThreadKind`：`User` / `Group`
- `MediaKind`：`Text` / `Image` / `Video` / `Voice` / `Card`。`MessageView.kind` 就是这个枚举。`classify_message` 在入库时写定；读路径信任存储的种类。`search_messages` 填真实 `MediaKind`
- `Relation`：`Friend` / `Incoming` / `Outgoing`
- `ProfileKind`：`User` / `Bot`。`ProfilePlacement`：`Local` / `Cloud`
- `ThreadPreview`：`Text { snippet }` / `Media { kind }` / `System { text }`。列表不在 Dart 里嗅探 URL。本地化媒体标签仍由 Dart `Copy` 负责
- `OutgoingContent`：`Text` / `Image` / `Video` / `Voice`
- `AgentCard`：类型 `Tool` / `ActionRequired`，状态 `Pending` / `Ok` / `Error`。气泡读投影，不 `jsonDecode` 正文

## 人设与 token

人设 schema 只在 `kim-agent-codec`。FFI upsert 收校验过的 JSON，表里只存 blob。`list` 用 `blob_to_json` 填 `document_json`。FFI 结构体没有 `body_blob`，也没有 `deleted_at`。`ProviderAccount.models` 是 `Vec<String>`。

续签发生在投影之前：`spawn_session_bridge` 收到 `SessionEvent::TokenRenew` 时先 `write_global(KEY_JWT, token)`，然后丢弃该事件。`SessionUpdate` 没有 `TokenRenew`。JWT 字符串不出 FFI。键名仍是 `kim.jwt`、`account.<keyRef>`。没有 `import_legacy_prefs`。

`kim-agent-ffi` 保持桌面 catalog。它依赖带 `codex` feature 的 `kim-agent-host`，并进 `kim-client-ffi` 会把该 host 链进手机 `.so`。手机 hook（`sdk/mobile/hook/build.dart`）跳过它。它不提供 session API，也不依赖 `kim-client`。

本地 sqlite：`user_version` 低于当前 schema 时删除该客户端库文件并按新 `CREATE` 重建。没有第二条 `ensure_column` 兼容迁移。服务端 Postgres 不在这条路径上。时间只在协议边界用现有的 `store::send_time_ms`。
