# FFI 面向对象契约

桌面编排在 Rust。Dart 只持有句柄、发意图、订阅 UI 投影。禁止为跑一轮 Agent 把 profile JSON、账号、overlay 在 Dart 与两条 FFI 之间来回搬运。

## 句柄

| Opaque | 职责 | Dart 可见 |
|---|---|---|
| `KimUiHandle`（`KimApp`） | 根：attach store、session、子句柄工厂 | 连接态、账号、全局事件 |
| `InboxHandle` | 会话列表投影 | `Inbox` 已有 watch；命令 `delete` |
| `ConversationHandle` | 单会话：进房、发送、已读、typing | timeline 增量、成员快照 |
| `ContactsHandle` | 好友 / 搜索 / 请求 | `PersonView` |
| `MediaHandle` | 上传 / 拉取 | 本地媒体引用 |
| `AgentCatalogHandle` | profile / account 的列表与编辑命令 | 行视图 + ack，不回传整份 host JSON |
| `HostAgentRuntime` | 进程内消费 `AgentRuntime::run_turn` | UI 事件流 + `respond` 权限 |

资源跟随句柄。`close` / drop 释放 native 会话与订阅。Dart 不保存 transcript、SQLite 行或 api key。

## 允许跨边界

- 用户意图：发消息、改设置、批准/拒绝权限（`callId` + decision）
- UI 投影：消息气泡、列表行、typing、permission 预览、连接状态、toast 文案
- 句柄本身（FRB opaque）

## 禁止跨边界

- `profile_json` / `SessionOpenOpts` 由 Dart 拼好再 `session_open`
- 为一次 turn `listAgentProfiles` → decode → overlay → accounts → reopen
- timeline 全表拉到 Dart 再过滤
- api key 明文经 Dart 再塞进 open opts

## `HostAgentRuntime`

实现 `kim_sdk::AgentRuntime`。桌面 `attach_store` 调用 `kim_desktop_runtime::install`，不再走 `FfiAgentRuntime` → Dart 编排循环。

```text
MobileAgent worker
  → HostAgentRuntime::run_turn
      load profile / overlay / account from KimSdk
      drive kim-agent-host
      IM tools on KimSdk
      action_required → oneshot, Dart respond_agent_permission
      publish AgentUiStatus (running / done / failed)
  → AgentRunResult
```

Dart `AgentHostController` 只做三件事：一次性 `cache_agent_secret`、订阅 permission、把 presence 交给 `AgentRunSink`。`test/support/legacy_agent_drive.dart` 保留旧的 session 端口驱动，仅供单测。

手机不链接 agent host，继续 `NoopAgent`。

`kim-agent-host` 不依赖 `kim-client`。编排 crate `kim-desktop-runtime` 同时依赖 `kim-sdk` 与 `kim-agent-host`。

## 唯一保留的 Agent 往返

```text
Rust action_required → StreamSink
Dart permission card → respond(call_id, allow|deny)
Rust oneshot → continue turn
```

## 生命周期

- `KimUiHandle::create` 不打开 SQLite；`attach_store` 后才有 store 与桌面 agent。
- `ConversationHandle` 不拥有进程级连接；`enter` / `leave` 成对。
- `HostAgentRuntime` 按 `dest:profile` 持有 session；`park` 保留，进程退出 `close`。
