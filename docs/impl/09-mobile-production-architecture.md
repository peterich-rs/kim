# Flutter 只做 UI：Dart 剩余逻辑全量收归 Rust，重建移动端生产架构

| 字段 | 值 |
|---|---|
| 状态 | Draft |
| 作者 | — |
| 日期 | 2026-09-14 |
| 对照代码 | HEAD `d2d6ddb`。行号以标识符为准；下文引用均已对当前树核验。 |
| 父规格 | [08-kim-sdk-ownership.md](./08-kim-sdk-ownership.md)（kim-sdk 所有权已落地：store / outbox / persist-then-ack；本文件收口 **Dart 侧剩余逻辑**）；[06-mobile-client-maturity.md](./06-mobile-client-maturity.md)（Phase 3–7 已合入，是地基）；[mobile-client.md](../mobile-client.md)（已落地形状） |
| 范围 | `sdk/mobile` + `crates/kim-sdk` + `sdk/mobile/rust`、`sdk/mobile/rust_agent` FFI 面 + `crates/kim-agent-host`（仅桌面 `rust_agent` 使用）。不动 gateway / chat / royal 热路径，不改 ACK 模型与协议。 |
| 铁律 | **Dart 不做业务逻辑、不做持久化、不做编排。** Dart 只：渲染、手势/输入、平台 API 呈现（通知/权限/分享）、生命周期转发。任何「if 涉及业务语义」的代码都在 Rust。 |
| 产品拍板 | **无回滚开关。无 `KIM_RUST_STORE`。无 `kim.rustStore` prefs。无双写。无弃用窗口。无 fat events + watch 双发。** 测试环境、无外部用户。每阶段一步切。打开 Rust store 的同一 PR 删除 Dart store / outbox / link 落盘。本地库 wipe 谓词见 Data Model（`< 1` 才抹，合法 v1+ 走 migrate）。 |

---

## Breaking Change Notice（仓库内部）

不涉外部 crate 与服务端。以下内部契约在标注 PR **整组删除，不是弃用**：

1. **Store 切流 PR（Phase 2）一次性删除，无 flag、无双写、无双发：**
   - Dart：`KimFlags.rustStore` / prefs `kim.rustStore` / `--dart-define=KIM_RUST_STORE`、`KimRuntime.rustStore`、`ConversationStore`（1426 行）、`MessageRepository`、`OutboxNotifier` 本地泵、`state/inbox.dart` 的 `ThreadsNotifier` 写路径、`link.dart` 的 fat-event 分发 / `applyLive` / `applySync` / `syncConfirm` / `ack`。
   - FFI：`session_events`、`persist_talks`、`persist_inbox`、`sync_confirm`、`ack`。`KimSessionEvent` 胖袋不再是 Dart inbox。
   - 行为：bootstrap **始终** `attach_store`。`protocol_only()` 仅保留给 `cargo test -p kim-sdk` 与 CLI。`KimSdk::open` 是测试夹具，不再是「Flag-on」路径。
   - 本地库：同路径 `support/kim-cache.db`。wipe **只在 Rust** `KimSdk::attach_store` → `prepare_store_file`（见 Data Model）。Dart **禁止** `File.delete`。合法 kim-sdk v1 库不得抹掉。
   - **推翻 08 D11「中间不得断 bot 回复」：** P2–P4 桌面 Goose **黑暗**（测试环境、仅内部）。`NoopAgent` 保持默认；P4 才 `set_agent(MobileAgent)`。
2. **FFI 面（Phase 1 生成、Phase 2 消费、Phase 5 收口门面）：**
   - `TimelineUpdateDto` / `SessionUpdateDto` 从扁平袋改为 FRB **tagged union，携带 payload**（`MessageViewDto` / `ThreadViewDto`）。`send_status` / link 为枚举，禁止再压成 `String`。`frb_generated.rs` 重新生成。
   - `AgentTurn` / `AgentCard` **不**在 P1 占位；P4 再改 FRB。
   - Phase 5：`KimSdkHandle` **就地改名** `KimUiHandle` + `command(UiCommandDto)`。登录 HTTP **不**进 `command`：始终 `KimAuthPort` / `KimAuth`。
3. **设置 / token（Phase 3，一次性导入后 Dart 存储死亡）：** `SettingsStore` 的 URL / origin / account / token / `JwtPeek` 删除。**不**注入 `TokenStore` trait。启动：Dart 从 Keychain 读 token，传入已有 `start_session`。后续唯一 writer 是 Rust，经 `watch_token_persist` 回调写回 Keychain。prefs 键导入后删除。**主题不进 SQLite。**
4. **Agent：** P2 把 `chat_agent.dart` / `capability_host.dart` 换成 **编译期 no-op stub**（不碰已删 store / `linkState` / `threads.notifier`），以便 P2 独立 `flutter analyze`。P4 删除 stub 与 `agent_profiles.dart`（1667），profiles JSON 一次性导入后删 prefs 键。Goose **只**经现有 `rust_agent`（`session_open`，`machine.rs:29` `MachineFactory::assemble`），FFI 缝为 `watch_agent_run` / `submit_agent_run`。`kim_client_ffi` **不加** `kim-agent-host` 依赖。电话保持 `NoopAgent`，不装队列。
5. **目录重组（Phase 7）：** `lib/screens` + `lib/state` + `lib/widgets` + `lib/data` + `lib/agent` → `lib/features/*` + `lib/design/*` + `lib/bridge/*`。纯移动 + import 更新。
6. **`me_page.dart` 环境切换（Phase 8）** 移入 DevPanel；生产构建编译剔除。`SettingsStore.defaultDest = 'bob'` 删除。

---

## Feasibility Assessment

08 切片已经把消息生命周期的 **Rust 半边**铺好，本文件要收的是 Dart 半边与 FFI 契约，不是从零造 store。核验过的接缝：

| 接缝 | HEAD 事实 | 本文件动作 |
|---|---|---|
| `AgentPort` | `crates/kim-sdk/src/agent.rs` 23 行：`enqueue_turn` + `catch_up`，仅 `NoopAgent`。`KimSdk::set_agent`（`lib.rs:280`）只在 `tests/agent_port.rs` 调用；FFI 从未 install | P4 桌面 `set_agent(MobileAgent)`。电话保持 `NoopAgent`。P2–P4 Goose 黑暗 |
| `SdkPersistHook` | `lib.rs:539-572` 实现 `kim_client::PersistHook`；`start_session` 在 store 已挂时安装（`lib.rs:143-148`） | **不**发明 `persist_on_event`。P2 删 Dart confirm 闸门 |
| `watch_session` / `watch_timeline` | FFI `client.rs:153,167` 已生成。inbox **不**走它们。`kim_bridge.dart:372`：「Fat session_events is the Dart inbox」。`watch_session` 源是 `KimSdk::subscribe_session`（mpsc），由 `spawn_session_bridge` **和** `persist_inbox_for` 喂。`subscribe_timeline`（`lib.rs:493-508`）插入 watch sender，**唯一初值**是 `TimelineUpdate::Resync { reason: "subscribe" }`；`persist_talks_for`（`lib.rs:327-337`）/ enqueue / mark_sent / delete **从不** `send` Snapshot/Delta | P1 补 payload；P2 给 **session snapshot 与 per-dest timeline 都加上 publisher**。subscribe 初值改热窗口 Snapshot（≤50），禁止 Resync-only |
| DTO payload | `TimelineUpdateDto`（`types.rs:3-12`）丢掉 `messages` / `pending` / `upserts` / `deleted_keys`。`From<SessionUpdate>`（`types.rs:70-128`）Inbox 只带 count；其余多项 `kind: "other"` | **P1 是 P2 的硬阻塞** |
| `rust_agent` | `hook/build.dart:10-17` 仅 macOS/windows/linux。`MachineFactory::assemble` 在 `crates/kim-agent-host/src/machine.rs:29`（`pub(crate)`），**不**在 kim-sdk | 双 FFI 不合并。**不**把 `kim-agent-host` 加进 `kim_client_ffi` |
| `load_older` | `KimSdk::load_older`（`lib.rs:310`）只读本地页。FFI **未暴露**。Dart `messages.dart:137` 是本地页 + `history()` | P1 暴露 `load_older`（含 `before_id`）。Rust 在本地不足时自己调 `protocol.history` 再 persist |
| `MediaUploader::upload_image` | `media.rs:69` 已有路径流式 POST | P5 扩 `fetch` + 磁盘 LRU |
| 事件丢失 | **两条独立故障：** (1) fat `session_events` `Lagged`（`client.rs:389`）可丢 Kickout — 这是今日 Dart inbox；(2) `emit_session` try_send Full（`lib.rs:380-387`）丢的是 `persist_inbox_for` 的 **Inbox**，不是 Kickout。Kickout 已走 `spawn_session_bridge` → `emit_session_wait`（`lib.rs:407-409`） | P2 删 fat 流；(2) persist 改 snapshot publish + Inbox wait-send。Kickout 到达测试保留 |
| Dart 消费面 | `lib/state` + `lib/agent` + `lib/data` + `lib/core/{settings,errors,media,jwt}`。会话列表权威在 `state/inbox.dart` `threadsProvider` | P2 `threadsProvider` = `select(snapshot.threads)`，删第二份列表 |

**Feasible with caveats。**

1. **FFI DTO 无 payload 是事件翻转的硬阻塞。** P1 不落地，P2 无法编译出能聊天的 App。
2. **同路径 `kim-cache.db`：** Dart 库无 `schema_version` 键。wipe 谓词是 `version < 1`（含缺失/不可解析），**不是** `!= SCHEMA_VERSION`。`migrate.rs:61-80` 的 `INSERT INTO outbox … FROM messages` 是产品禁止的双 schema 导入器，P2 必须删除或 `cfg(test)` 闸掉。

---

## Overview

08 把消息身份、outbox、未读、persist-then-ack 放进了 `kim-sdk`，但 Flutter 默认仍走 Dart SQLite + fat `session_events`。`KimFlags.rustStore` 默认 `false`（`settings.dart:11-21`，`runtime.dart:19,67`）。flag 打开时 Dart 仍然听 fat 流、仍然 `receiveAll` 进 Riverpod、仍然持内存 `ConversationStore.memory()`，只是跳过落盘。`Inner.session_snapshot` watch **从未 send**。

本文件把剩余 Dart 业务收进 `kim-sdk`，FFI 改成带 payload 的 tagged union，Dart 退回展示壳。无开关、无双写、无双发。P2 之后能登录、会话列表、收发文本/图片；**桌面 Goose 要到 P4 才恢复**（明确覆盖 08 D11）。电话不装 Goose、不安 MobileAgent 队列。

---

## Background & Motivation

### 08 已落地 vs Dart 仍拥有的

08 的目标在 **Rust crate 内**已经成立：`Store` 写队列、`outbox::pump`、`SdkPersistHook`、`enqueue_message`。验收 `cargo test -p kim-sdk` 覆盖 invariants / persist_ack / outbox_kill / store_restart / unread_replay。

生产 App 默认不走这条路径。`ConversationStore.openForRuntime`（`conversation_store.dart:76-88`）仅当 `rustStore==true` 才 `attachStore`，否则 Dart isolate 打开同一文件名 `kim-cache.db`。Dart `meta` 表有 `prefs_imported` 等键，**没有** `schema_version`。kim-sdk `SCHEMA_VERSION = 1`（`store/schema.rs:2`）。

`migrate_v1`（`store/migrate.rs:47-80`）在 `schema_version < 1` 时 `INSERT OR IGNORE INTO outbox … FROM messages WHERE status IN ('sending','failed')`。若 P2 不先 wipe 就 `attach_store`，这就是产品禁止的 Dart→Rust 行级导入器。

### 当前痛点（代码事实）

1. **Fat inbox 仍是跨语言契约。** `KimSessionEvent`（`client.rs:74-95`）`kind: String` + 复用字段；`SyncPage` talks 塞进 `body` JSON（`kim_bridge.dart:688-719`）。`session_events` 在 `Lagged` 时只 `tracing::warn` — **Kickout 丢在这条 fat 流上**。
2. **watch DTO 不能渲染聊天，且 timeline watch 没有 publisher。** `From<TimelineUpdate>` 丢掉 `snapshot.messages` / `delta.upserts`。即便 P1 补上 payload，`subscribe_timeline` 初值仍是 `Resync { reason: "subscribe" }`，persist/enqueue 从不 `send` Snapshot/Delta。
3. **snapshot watch 是死的。** `subscribe_session_snapshot`（`lib.rs:489`）存在；没有任何生产路径 `send`。`KimSdk::session_snapshot()`（`session.rs:35`）返回的是 `StartSession`，不是 UI snapshot。`persist_inbox_for`（`lib.rs:353-365`）只 `emit_session(Inbox)`（try_send，可丢线程列表）。
4. **会话列表有两份。** `inbox.dart` `threadsProvider` 从 `conversationStoreProvider.loadThreads` 起步，再 `mergeInbox` / `applyTalk`。即使将来 `kimSessionProvider` 持 `snapshot.threads`，若不删 `ThreadsNotifier` 写路径，Dart 仍有第二份状态。
5. **Agent 编排在 Dart。** `outbox.dart:255-262` 在 Sent-to-owned-bot 调 `ChatAgent.enqueueTurn`（还要 `agentHostSupported`）。`link.dart:227` Online 时 `catchUpPending`（走 `botPending` HTTP，**不是** `AgentPort::catch_up`）。Rust `outbox/pump.rs:77-85` 对 **每条已发送文本** 调 `AgentPort::enqueue_turn`，无 owned-bot 过滤。`AgentPort::catch_up` 生产调用者为零。
6. **设置/token/JWT 过期在 Dart。** `JwtPeek` 66 行；`errors.dart` 仍 `contains('status 109')`。
7. **双 FFI、双库、双泵。** 默认走 Dart 泵。

---

## Goals & Non-Goals

### Goals

1. Dart 零业务语义 `if`、零 SQLite、零 JSON 持久化、零 `dart:io` 网络、零定时器驱动的业务轮询、零 `JwtPeek`。
2. Rust 是本地业务状态的唯一写入者：消息、outbox、会话摘要、未读、sync 游标、通讯录、device/account settings（非主题）、agent profiles/permissions（桌面）、媒体磁盘缓存。
3. FFI 下行 = 三条 watch（snapshot / 离散事件 / per-dest timeline）；上行 P2 为命名方法，P5 起加 `command(UiCommandDto)`。Dart 不见 ack / epoch / sync cursor。
4. **桌面** `MobileAgent` 拥有队列/LRU/profiles。Goose 只经 `rust_agent::session_open`。**电话**保持 `NoopAgent`：`enqueue_turn` / `catch_up` 是 trait 级 no-op，**不安** per-dest 队列。
5. 每 phase 独立可编译。P2 之后：登录、会话列表、收发文本/图片。**P2–P4 桌面 Goose 黑暗**（内部可接受）。P4 恢复 owned-bot 回复。
6. 主题/locale 留 Dart。Token：启动 Dart 读 Keychain → `start_session`；之后 Rust 经 `watch_token_persist` 回写。
7. 验收见各 phase 清单 + P2 验收勾选。

### Non-Goals

- 不改 gateway / chat / royal / kim-tcp 热路径，不改 ACK 协议，不关 G-03。
- 不重写 Web SDK；不引入 Cupertino app theme；不做 QUIC / FCM / 整库加密（AES-GCM **只**用于 provider key）。
- 不把 `rust_agent` 合并进 `kim_client_ffi`；**不**让 `kim_client_ffi` 依赖 `kim-agent-host`。
- 不把 FTS5 当作 P5 门闩。
- 不在本切片把 Goose 打进 iOS/Android；电话不安 MobileAgent 队列。
- 不重写自研 `ChatList`；不加 drafts 表（08 D16）。
- 不发明 `timeline_page` / `persist_on_event` / 第二套 uploader。
- **不**在 P2–P3 维持桌面 bot 回复（覆盖 08 D11）。

---

## Current Surface Inventory

### 分层现状

```text
┌ Flutter 渲染/手势/主题 (现有)
├ Dart 逻辑 ✂  link 515 / outbox 444 / store 1426 / inbox.dart
│              chat_agent 943 / agent_profiles 1667 / settings 261
├ FFI rust     KimSdkHandle.create = 协议-only; attach_store 可选
│              session_events = Dart inbox; watch_* 已生成、不当 inbox
│              DTO 扁平袋 ✗ 无 MessageView / 无 ThreadView 列表
├ kim-sdk      store/outbox/SdkPersistHook/enqueue_message (现有)
│              AgentPort 23 行 NoopAgent; snapshot watch 从未 send
│              migrate_v1 会从 Dart messages 导入 outbox ✗
├ kim-client   WSS / SyncEngine / PersistHook (现有)
└ kim-agent-host  MachineFactory::assemble (machine.rs:29) 仅桌面
```

### Rust（已有，勿当本设计新造）

| 路径 | 现状 |
|---|---|
| `crates/kim-sdk/src/lib.rs` | `protocol_only` / `attach_store` / `start_session` 装 `SdkPersistHook`；命令与 watch；`subscribe_session_snapshot` watch（无 publisher）；`subscribe_timeline` 初值仅 Resync（无 Snapshot/Delta publisher）；`emit_session` Full 丢 Inbox |
| `crates/kim-sdk/src/session.rs:35` | `session_snapshot()` → `StartSession`，**不是** UI `SessionSnapshot` |
| `crates/kim-sdk/src/store/` | messages / threads / outbox / cursors / watermarks / timeline_meta；`SCHEMA_VERSION = 1`；与 Dart 同文件名 |
| `crates/kim-sdk/src/store/migrate.rs:61-80` | `schema_version < 1` 时从 `messages` 导入 `outbox` — 禁止的双 schema 导入器 |
| `crates/kim-sdk/src/outbox/pump.rs:77-85` | 每条已发送 **文本** 调 `enqueue_turn`，不过滤 owned bot |
| `crates/kim-sdk/src/media.rs` | `MediaUploader::upload_image`；并发 2；5 MiB |
| `crates/kim-sdk/src/agent.rs` | `AgentPort` + `NoopAgent`，23 行。`catch_up` 生产零调用 |
| `crates/kim-sdk/src/timeline.rs:58-117` | `SessionUpdate` 已有 Link…GroupCreate。缺 ContactsChanged / AgentTurn / AgentCard / RustPanic |
| `crates/kim-sdk/src/timeline.rs:131-135` | `SessionSnapshot { link, last_error, threads }` 仅三字段 |
| `crates/kim-sdk/src/error.rs` | 19 变体 |
| `crates/kim-agent-host/src/machine.rs:29` | `MachineFactory::assemble`（`pub(crate)`） |
| `sdk/mobile/rust/src/api/client.rs` | `session_events` = Dart inbox；`watch_session` / `watch_timeline` 在 |
| `sdk/mobile/hook/build.dart` | `rust_agent` 仅 macOS/windows/linux |
| `sdk/mobile/rust_agent/src/api/session.rs:275` | `session_open` |

### Dart 剩余逻辑（`wc -l`）

| 文件 | 行 | 做什么 | 迁往 |
|---|---:|---|---|
| `state/link.dart` | 515 | fat → 8 notifier；ACK/`syncConfirm`；Online 时 `catchUpPending`；`retry()` = `notifyRadioUp`/`_start` | P2：`linkProvider` = `select(snapshot.link)` + 生命周期 `retry` |
| `state/inbox.dart` | — | `threadsProvider` 读 Dart store，`mergeInbox`/`applyTalk`/`_persist` | P2：`select(snapshot.threads)` |
| `state/chat_agent.dart` | 943 | 队列/门闩/LRU(4)/权限；`catchUpPending` → `botPending`；写 `conversationStore` / `linkState()` / `threadsProvider.notifier` | P2：编译期 no-op stub；P4 删，改 `MobileAgent` |
| `state/agent_profiles.dart` | 1667 | CRUD + prefs JSON；`serverAccount` 字段（`agent_profiles.dart:856`） | P4：`agent_profiles.server_account` 列 |
| `data/conversation_store.dart` | 1426 | Dart SQLite + isolate | P2：**删** |
| `state/outbox.dart` | 444 | 默认泵；Sent-to-owned-bot → `enqueueTurn` | P2：**删泵** |
| `core/settings.dart` | 261 | URL/origin/account/token + `JwtPeek`；flag 默认 false | P2 删 flag；P3 删存储 |
| `core/errors.dart` | 89 | 字符串匹配 | P0：`switch (kind)` |
| `core/media.dart` | 159 | Dart `HttpClient` 上传 | P5：删 `KimMediaClient` |
| `kim_bridge.dart` | 1019 | 4×`jsonDecode` + persist FFI | P1 去 people JSON；P2 删 fat/persist |
| `state/contacts.dart` | 328 | 内存 roster | P3：离散 `ContactsChanged` |
| `state/messages.dart` | 292 | `loadOlder` = 本地 + `history()` | P2：`watch_thread` + Rust `load_older` |
| `agent/capability_host.dart` | 232 | IM 工具 `switch (name)`；`conversationStoreProvider.loadMessages`（`:129,157`） | P2：编译期 no-op stub；P4 删，迁 host |
| `core/jwt.dart` | 66 | `JwtPeek` | P3：**删** |
| `data/message_repository.dart` | 144 | rustStore 短路 | P2：**删** |
| `core/runtime.dart` | 70 | `rustStore` 默认 false | P2：删字段 |

`lib/` 内 `catch (_) {}` **76** 处；`Theme.of` **62** 处。`watchTimeline` 业务代码零消费。

登录 HTTP 今日走 `KimAuthPort`（`kim_bridge.dart:189`，底层 `KimAuth` FRB）。本设计 **保持** 这条缝，不把 `SignIn` 放进 `UiCommandDto`。

### 今日读路径

```text
WGateway ══► SessionSupervisor.events (broadcast 64)
                │
                └─ session_events fat ──► LinkNotifier._onEvent
                     Lagged ✗ 丢 Kickout    │
                                            ├─ threadsProvider.mergeInbox
                                            ├─ threadMessages.receiveAll
                                            └─ ack / syncConfirm (flag 关)

KimSdk.subscribe_session (mpsc)
   ▲  spawn_session_bridge: Kickout wait-send (现有)
   ▲  persist_inbox_for: emit_session(Inbox) ✗ Full 可丢
   └─ watch_session ──► 仅 Kickout/token/friend 并入 fat inbox

session_snapshot watch: Sender 存在, 从不 send ✗
watch_timeline: 初值仅 Resync("subscribe"); persist/enqueue 从不 send ✗
```

Legend: `──►` 同步/调用   `══►` 推送   `✗` 丢失或死路径

### 今日写路径（默认 = Dart 泵）

```text
Composer ──► OutboxNotifier.enqueue
                ├─ rustStore=false ✂
                │     ConversationStore._persist ──► _pump
                │     ──► KimMediaClient.uploadImage (dart:io)
                │     ──► sendMessage
                │     ──► ChatAgent.enqueueTurn (owned bot, 桌面)
                └─ rustStore=true (现有)
                      enqueue_message FFI ──► kim-sdk outbox pump
                      pump 对每条文本 enqueue_turn → NoopAgent
```

---

## Design

### 目标分层

```text
┌ Flutter 渲染/手势/主题/生命周期转发/select 映射
├ bridge     KimSdkHandle 转发 (P5 改名 KimUiHandle); FakeKim 测试缝
├ FFI        watch ×3 + 命名方法; P5 command(); tagged union ★
├ kim-sdk    store + SdkPersistHook (现有)
│            prepare_store_file wipe ★
│            snapshot publisher ★ / timeline publisher ★
│            桌面 MobileAgent ★ / contacts ★ / settings ★
│            MediaUploader + fetch/LRU ★
├ kim-client WSS / SyncEngine / PersistHook (现有, 零改热路径)
└ kim-agent-host  只经 rust_agent; kim_client_ffi 不依赖它
```

### 目标读路径（P2 起）

```text
persist / mark_read / link / delete_thread
   │
   ├─ publish_session_snapshot() ★  (watch, latest-wins)
   │     {link, last_error, threads, unread_total}
   │     ══► watch_session_snapshot ──► kimSessionProvider
   │            threadsProvider = select(snapshot.threads)  仅此一份
   │            unread 用 snapshot.unread_total, 禁止 fold
   │
   ├─ SessionUpdate mpsc wait-send
   │     Kickout / AuthExpired / TokenRenew / Friend*
   │     Typing / Presence / ReceiptRead / ProfileUpdated
   │     GroupCreate / SyncProgress
   │     P3: ContactsChanged    P4: AgentTurn/AgentCard
   │     P8: RustPanic
   │     ══► watch_session_events ──► upsert-by-id (无业务 if)
   │     Dart 忽略 Inbox/ThreadUpsert/Link
   │     (列表只来自 snapshot.threads; 链路只来自 snapshot.link)
   │
   └─ publish_timeline(dest) ★  Snapshot|Delta|Resync (满 payload)
         subscribe 初值 = 热窗口 Snapshot (<=50 + pending)
         ══► watch_thread(dest) ──► threadTimelineProvider
         Dart Resync = 丢掉热窗口, 等下一条 Snapshot
         禁止再调 watchThread 形成循环
```

### P2 写路径（`command()` 要到 P5）

```text
P2 写路径
Composer ──► enqueueMessage FFI ──► Store tx (message+outbox)
                                      │
                                      ▼
                                 outbox pump (现有)
                                      ├─ MediaUploader.upload_image
                                      └─ send_message ──► WGateway
                                      │
                                      ══► TimelineUpdate / snapshot
P2-P4: pump enqueue_turn → NoopAgent (Goose 黑暗)
```

### P5+ 写路径

```text
P5+ 写路径
Widget ──► command(UiCommandDto) ──► 同 P2 的 enqueue / mark_read / friend
桌面 owned-bot: pump 过滤后 MobileAgent.enqueue_turn
     ══► watch_agent_run(AgentRunRequestDto)
     ──► AgentBridge ──► rust_agent.prompt
     ──► submit_agent_run(AgentRunResultDto)
     P5 可将 submit 折进 command(AgentRunResult)
电话: NoopAgent, 无队列
Royal 登录: KimAuthPort (现有, 不进 command)
```

### persist-then-ack（已在 Rust；本设计只 ✂ Dart 闸门）

```text
 WGateway      SyncEngine     SdkPersistHook     SQLite      Dart UI
     │              │                │              │            │
     │ offline page │                │              │            │
     │─────────────►│ persist Keep   │              │            │
     │              │───────────────►│ BEGIN IMMED  │            │
     │              │                │─────────────►│            │
     │              │                │ commit       │            │
     │              │◄───────────────│              │            │
     │ ack_batch    │                │ publish snap │            │
     │◄─────────────│                │ + timeline   │            │
     │              │                │══════════════════════════►│
     │              │                │              │   ✂ ack()  │
     │              │                │              │   ✂ syncConfirm
```

Live Talk 维持 08：`try_send` 到 store worker，满则不 ACK、不停读循环。

---

### Key Design Decisions

编号决策是正文。文末 **Key Decisions 表是索引**，不重复「选择/否决」长段。

1. **无 flag、无双写、无双发、无弃用窗口。** P2 同 PR 打开 Rust store 并删除 Dart store/outbox/fat inbox。否决 `KIM_RUST_STORE` 过渡与 fat+watch 双发。测试环境无外部用户；今日 `rustStore=true` 已证明半开不能消灭第二份 UI 状态。回滚 = `git revert` P2。

2. **P1 = 带 payload 的 FRB tagged union。** `TimelineUpdateDto = Snapshot|Delta|Resync`（内嵌 `Vec<MessageViewDto>`）；`SessionUpdateDto` 与 Rust 枚举 1:1。`SendStatusDto` / `LinkStateDto` 为枚举。否决再做一个扁平袋；否决只改 `friend_list` JSON。没有 MessageView 过缝则 P2 不能聊天。`AgentTurn`/`AgentCard` **推迟到 P4** 再改 FRB，P1 不放 JSON `payload: String` 占位。

3. **不长期并列第二 public handle。** P1–P4 仍 `KimSdkHandle`；P5 就地改名 `KimUiHandle` + `command()`。否决 P2 新增第二 opaque。登录保持 `KimAuthPort`，`command` 不含 `SignIn`。

4. **Snapshot 不是垃圾场；timeline watch 必须有 publisher。** `SessionSnapshot = {link, last_error, threads, unread_total}` **仅四字段**。Contacts **不**进 snapshot，只走离散 `ContactsChanged`。线程列表 **只**来自 `select(snapshot.threads)`；链路 **只**来自 `select(snapshot.link)`（`linkProvider`）。Dart **禁止**再 `mergeInbox`，**禁止**把 `SessionUpdate::Link` 当 ConnStatus 源。Typing/presence/receipts 不进 snapshot。P2 必须实现 `publish_session_snapshot()` **和** `publish_timeline(dest)`：今日 `session_snapshot` sender 从未 `send`；`subscribe_timeline` 初值只有 Resync，persist/enqueue 从不 `send` Snapshot/Delta — 只接 FFI 是两条死流。

5. **两条丢失故障分开修。** (1) 删 `session_events`，消灭 fat Lagged 丢 Kickout。(2) persist 路径：先 `publish_session_snapshot`，Inbox 改 `emit_session_wait`。Kickout 已是 wait-send，P2 加负载下到达测试，不把「Kickout 丢在 mpsc」写成现状。

6. **Token 缝：启动传入 + persist sink，不搞 TokenStore trait。** Dart 启动从 Keychain 读 token，传入已有 `start_session`。Rust 是后续唯一 writer。`watch_token_persist(StreamSink<TokenPersistDto>)`：`Write { token }` | `Clear`（TokenRenew / sign-out）。等价于 `fn(String) -> Result<(), SdkErrorDto>` 的写回，不用把 Dart closure 做成 `Send+Sync` trait。Agent AES-GCM 用独立 Keychain 键 `kim.agent_wrap`，**禁止**用 JWT 字节当 wrapping key。URL/env 走 device settings 行（`account = ''`），登录前即可读。

7. **复用现有 API。** 分页 = `load_older(PageCursor)`（含 `before_id`）。本地页短则 Rust 调 `protocol.history`、Keep persist、再读本地 — 组合在 Rust，Dart 禁止本地+远程拼接。落盘 = `SdkPersistHook`。上传 = 扩 `MediaUploader`。

8. **P2 单 PR（旧 2a+2b 合并），但必须可审。** 无 flag 则无「只开 store」中间态。P1 已把 DTO 落地；P2 = publisher + 消费翻转 + 删除。下文贴出 P2 `KimClientPort` / FakeKim 方法表、测试文件清单、验收勾选。

9. **覆盖 08 D11；Agent 桌面/电话不对称。** P2–P4 桌面 Goose 黑暗（内部-only）。P2 **不**把 agent 整段下沉：把 `chat_agent.dart` / `capability_host.dart` 换成编译期 no-op stub（不碰 store / `linkState()` / `threadsProvider.notifier`），P4 再删 stub、装 `MobileAgent`。P4：`outbox/pump` 的 `enqueue_turn` **过滤** owned registered bot（读 `agent_profiles.server_account`，对齐 `isOwnedRegisteredBot`，`mention.dart:44`）。`catch_up` 接到桌面 link-online / sync-done（`SyncProgress.catching_up == false`），内部 `bot_pending` + 入队。电话：**不** `set_agent(MobileAgent)`，保持 `NoopAgent`，trait 级 no-op，**没有** per-dest 队列。桌面 `AgentRuntime` 的 FFI 缝是 P4 命名方法 `watch_agent_run` / `submit_agent_run`（**不是** P5 才有的 `command()`）。`kim_client_ffi` **零** `kim-agent-host` 依赖。

10. **wipe 谓词唯一，且在 Rust。** 见 Data Model `prepare_store_file`。否决 Dart 删文件；否决 `!= SCHEMA_VERSION`；否决保留 `migrate_v1` 的 messages→outbox 导入。

11. **编排规则写进 review checklist。** 禁止/允许清单见下。`jwt.dart` 删除。P0 空 catch 只改 **P4 之后仍在的文件**，不改即将删除的 `agent_profiles.dart` / `conversation_store.dart` / `outbox.dart`，也不改 P2 stub 化的 `chat_agent.dart` / `capability_host.dart`。

12. **FTS5 不挡 P5。** `search(query, dest: Option<String>)`：`dest = Some` 则 `LIKE` 仅该线程；`None` 则全局 `LIKE`。一律封顶 50。

13. **首次 wipe 在 DevPanel 提示，永不弹系统对话框。** `store_wipe_total` 进 metrics。P8 DevPanel 在本 boot/session 发生过 wipe、或 `store_wipe_total` 自上次展示后 `> 0` 时，显示面板内横幅（例如「本地库已按 kim-sdk schema 重建」+ 计数）。**禁止** OS alert / `showDialog` / 系统对话框。

---

### 编排规则（review checklist）

Dart **禁止**：`dart:io` 网络；`sqflite`/`sqlite3`/SQL；业务语义 `if`（重试、ACK、好友状态机、未读加减、sync 游标、epoch）；JSON 持久化（theme 键除外）；定时器驱动的业务轮询；`JwtPeek`。

Dart **允许**：渲染分支；动画/手势/滚动锚/IME；平台 channel 转发（权限、通知、分享、Keychain 读写转发、`notify_radio_up` / `notify_foreground`）；`select` 映射；时间/大小格式化、layout、theme。

Rust FFI **禁止**：`String` 作为业务 payload（列表/消息/profile/发送状态/链路状态必须 DTO 或 enum）；UI isolate 同步调用 >1ms（禁止新增 `rt().block_on` 热路径；HEAD 好友/历史的 block_on 在 P5 改 async）。

**例外：** `core/format.dart`、`core/layout.dart`。`core/jwt.dart` **删除**。

---

### Snapshot publisher（P2 必做，否则 FFI 接到死流）

```rust
// crates/kim-sdk/src/lib.rs  — 今日 Inner.session_snapshot 从不 send

fn publish_session_snapshot(&self) {
    let link = /* supervisor.state 映射 LinkStateView，无 sup 则 Offline */;
    let last_error = /* 最近 DropReason 短码 */;
    let threads = /* store.load_threads(account) 或空 */;
    let unread_total = threads.iter().map(|t| t.unread.max(0) as i64).sum::<i64>()
        .clamp(0, i32::MAX as i64) as i32;
    let _ = self.inner.session_snapshot.send(SessionSnapshot {
        link,
        last_error,
        threads,
        unread_total,
    });
}
```

调用点（commit 成功之后，非事务内）：`persist_inbox_for`、`persist_talks_for`（线程摘要变了）、`mark_read`、`delete_thread`、`spawn_session_bridge` 收到 `Link`。`Inbox` / `ThreadUpsert` 仍可 wait-send 给测试，**Dart 不消费它们做列表**。Dart **忽略** `SessionUpdate::Link` 作为 ConnStatus 源。

---

### Timeline publisher（P2 必做，否则 `watch_thread` 永远 Resync 空聊天）

HEAD `subscribe_timeline`（`lib.rs:493-508`）插入 watch sender，初值 **只有** `TimelineUpdate::Resync { reason: "subscribe" }`。没有任何生产路径 `send` `Snapshot` / `Delta`。P1 把 `MessageViewDto` 放到 DTO 上不够：没有 publisher，P2 Dart 按「Resync = 重订阅」会 **无限循环**，聊天空白。FakeKim `pushTimeline` 绿不了生产路径。

```rust
// crates/kim-sdk/src/lib.rs

fn publish_timeline(&self, dest: &str) {
    // 热窗口: 最新 N 条 sent + 该 dest 全部 pending; N = subscribe 时的 limit, 默认 50
    // 禁止把 load_older 翻到的更早页塞进 Snapshot (08: 不扩热窗口)
    let snapshot = /* store.load_hot_window(account, dest, limit=50) */;
    if let Some(tx) = lock(&self.inner.timelines).get(dest) {
        let _ = tx.send(TimelineUpdate::Snapshot { snapshot });
    }
}

fn publish_timeline_delta(&self, dest: &str, delta: TimelineDelta) {
    if let Some(tx) = lock(&self.inner.timelines).get(dest) {
        let _ = tx.send(TimelineUpdate::Delta { delta });
    }
}

fn publish_timeline_resync(&self, dest: &str, reason: &str) {
    // 仅 version 缺口 / store 损坏. 紧接着必须 publish_timeline Snapshot
    // 禁止只发 Resync 当 subscribe 初值
    if let Some(tx) = lock(&self.inner.timelines).get(dest) {
        let _ = tx.send(TimelineUpdate::Resync {
            dest: dest.into(),
            reason: reason.into(),
        });
    }
    self.publish_timeline(dest);
}
```

**调用点（commit 成功之后，非事务内）：**

| 点 | 发什么 |
|---|---|
| `subscribe_timeline` | **热窗口 Snapshot**（≤50 + 该 dest 全部 pending）。**禁止** Resync-only 初值 |
| `persist_talks_for` | 每个涉及 dest：有订阅且 version 连续 → Delta（`upserts`/`deleted_keys` 含 `MessageView`）；否则 Snapshot |
| `enqueue_message` / `mark_sent` / `mark_failed` / `cancel_send` | 该 dest Delta（pending/status） |
| `delete_thread` | 该 dest `Resync("deleted")` 后 Snapshot 空窗，或 Delta `deleted_keys` = 热窗口全部 key |
| `load_older` 在 Rust 侧 `protocol.history` + Keep persist **之后** | 只刷新热窗口 Snapshot/Delta（仍 ≤50）。更早页只经 RPC `MessagePageDto` 回给 Dart，**不**扩 watch 热窗口 |

**Dart `messages.dart`：**

- Snapshot = 热窗口全量替换（保留 VM 已用 `loadOlder` 拿到的更早页，除非 `deleted_keys` 点名或 `delete_thread`）。
- Delta = 按 `key` upsert / 删。
- **Resync = 丢掉热窗口，等待下一条 Snapshot。禁止再调 `watchThread` 形成循环**（HEAD 初值就是 Resync，重订阅只会再拿到 Resync）。

P2 `cargo test -p kim-sdk` 必须有：persist 一条 talk → `subscribe_timeline` 订阅者收到带 `MessageView` 的 Snapshot 或 Delta（body 非空）。只测 FakeKim `pushTimeline` **不算**过关。

---

### API / Interface Changes

#### P1 DTO（`sdk/mobile/rust/src/api/types.rs`）— 可 codegen，无 comment 占位

```rust
pub enum SendStatusDto {
    Pending, Uploading, Sending, Sent, Failed, Cancelled,
}

pub enum LinkStateDto {
    Connecting, Online, Reconnecting { attempt: u32 }, Offline,
}

pub struct MessageViewDto {
    pub key: String,
    pub dest: String,
    pub sender: String,
    pub body: String,
    pub local_path: Option<String>,
    pub at: i64,
    pub sys: bool,
    pub kind: i32,
    pub width: i32,
    pub height: i32,
    pub message_id: i64,
    pub batch_id: Option<String>,
    pub send_status: SendStatusDto,
}

pub struct ThreadViewDto {
    pub id: String,
    pub kind: i32,
    pub title: String,
    pub avatar: String,
    pub last_body: String,
    pub last_at: i64,
    pub unread: i32,
}

pub struct TimelineSnapshotDto {
    pub dest: String,
    pub version: u64,
    pub messages: Vec<MessageViewDto>,
    pub pending: Vec<MessageViewDto>,
    pub unread: i32,
    pub last_read_message_id: i64,
    pub has_more: bool,
}

pub struct TimelineDeltaDto {
    pub dest: String,
    pub from_version: u64,
    pub to_version: u64,
    pub upserts: Vec<MessageViewDto>,
    pub deleted_keys: Vec<String>,
    pub unread: Option<i32>,
    pub last_read_message_id: Option<i64>,
}

pub enum TimelineUpdateDto {
    Snapshot { snapshot: TimelineSnapshotDto },
    Delta { delta: TimelineDeltaDto },
    Resync { dest: String, reason: String },
}

pub struct PersonDto {
    pub account: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
    pub relation: String, // friend | incoming | outgoing | none | blocked
}

pub struct RoomMemberDto {
    pub account: String,
    pub status: i32,
    pub last_seen: i64,
}

pub struct BotDto {
    pub dest: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
    pub model: String,
    pub thinking_effort: String,
    pub context_tokens: i32,
    pub visibility: String,
}

pub struct ProfileDto {
    pub account: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
}

pub struct MessagePageDto {
    pub dest: String,
    pub messages: Vec<MessageViewDto>,
    pub has_more: bool,
}

pub struct CommandAckDto {
    pub request_id: String,
    pub client_id: String,
    pub dest: String,
    pub accepted_at: i64,
    pub send_status: SendStatusDto,
}

pub struct SessionSnapshotDto {
    pub link: LinkStateDto,
    pub last_error: Option<String>,
    pub threads: Vec<ThreadViewDto>,
    pub unread_total: i32,
}

pub enum SessionUpdateDto {
    Link { state: LinkStateDto, last_error: Option<String> }, // Dart 忽略, 用 snapshot
    Inbox { threads: Vec<ThreadViewDto> },                    // Dart 忽略, 用 snapshot
    ThreadUpsert { thread: ThreadViewDto },                   // Dart 忽略, 用 snapshot
    SyncProgress { pulled: u64, catching_up: bool },
    Kickout { channel_id: String },
    AuthExpired { reason: String },
    TokenRenew { token: String, exp: i64 },
    FriendRequest { from: String, nickname: String },
    FriendAccepted { from: String, nickname: String },
    ProfileUpdated { account: String, nickname: String, avatar: String },
    Presence { account: String, status: i32, last_seen: i64 },
    Typing { typer: String, dest: String, kind: i32, active: bool },
    ReceiptRead { reader: String, dest: String, kind: i32, message_id: i64 },
    GroupCreate { group_id: String, members: Vec<String> },
    ContactsChanged { contacts: Vec<PersonDto> }, // P3 起有数据; P1 生成变体
    RustPanic { message: String },                // P8 才 emit; P1 生成变体
    // AgentTurn / AgentCard: 不在 P1。P4 再加 tagged DTO, 禁止 JSON String
}

pub struct SettingsDto {
    pub ws_url: String,
    pub http_origin: String,
    pub env: String,      // dev | staging | prod
    pub locale: String,   // 给服务器
    pub account: String,  // 当前账号; 未登录为空
}

pub enum TokenPersistDto {
    Write { token: String },
    Clear,
}

pub struct LocalMediaDto {
    pub local_path: String,
    pub byte_size: i64,
    pub width: i32,
    pub height: i32,
}
```

`From<TimelineUpdate>` / `From<SessionUpdate>` 穷尽匹配，**禁止** `_ => "other"`。P1 同时把 `friend_list` / `friend_incoming` / `search_users` / `profile` / `room_enter` 改为 DTO。

```rust
impl KimSdkHandle {
    pub fn watch_session_snapshot(&self, sink: StreamSink<SessionSnapshotDto>);
    pub async fn load_older(
        &self,
        dest: String,
        before_at: i64,
        before_key: String,
        before_id: i64, // PageCursor.before_id; 0 = 未提供
        limit: i32,
    ) -> Result<MessagePageDto, SdkErrorDto>;
}
```

`load_older` 不重置 watch、不把热窗口扩成全量。Rust 实现：

1. `store.load_older(PageCursor { dest, before_at, before_key, before_id, limit })`。
2. 若 `messages.len() < limit` 且已连接且 `before_id != 0`：`protocol.history(dest, kind, before_id, limit)` → `persist_talks(Keep)` → 再读本地页。
3. Keep persist 之后调 `publish_timeline(dest)`（热窗口 Snapshot/Delta，仍 ≤50）。
4. 返回 `MessagePageDto`（含更早页）。Dart **禁止**再调 `history()`。

#### P2 `KimClientPort` / FakeKim（本 PR 的测试缝）

P2 之后 port **删除**：`sessionEvents`、`syncConfirm`、`ack`、`persistTalks`、`persistInboxThreads`、`attachStore`、`rustStoreAttached`、`history`、`sendMessage`（UI；测试夹具可留内部方法但不在 port 上）。

P2 port：

```dart
abstract class KimClientPort {
  Stream<SessionSnapshotDto> watchSessionSnapshot();
  Stream<SessionUpdateDto> watchSessionEvents();
  Stream<TimelineUpdateDto> watchThread(String dest, {int limit = 50});

  Future<void> startSession(String url, String token, {required String userAgent});
  Future<void> stopSession();
  Future<void> notifyRadioUp();
  Future<void> notifyForeground();

  Future<KimCommandReceipt> enqueueMessage({
    required String dest,
    required ThreadKind kind,
    required KimOutgoingContent content,
    required String clientId,
    String localPath = '',
    String mime = '',
    int width = 0,
    int height = 0,
    int byteSize = 0,
  });
  Future<void> cancelSend(String clientId);
  Future<KimCommandReceipt> retrySend(String clientId);
  Future<void> deleteThread(String dest);
  Future<MessagePageDto> loadOlder({
    required String dest,
    required int beforeAt,
    required String beforeKey,
    int beforeId = 0,
    int limit = 50,
  });
  Future<void> markRead(String dest, ThreadKind kind, int messageId);

  Future<List<PersonDto>> friendList();
  Future<List<PersonDto>> friendIncoming();
  Future<List<PersonDto>> searchUsers(String query);
  Future<void> friendRequest(String dest);
  Future<void> friendAccept(String dest);
  Future<void> friendReject(String dest);
  Future<void> friendRemove(String dest);
  Future<ProfileDto> profile({String dest = ''});
  Future<ProfileDto> updateProfile(
      {required String nickname, required String avatar, String bio = ''});
  Future<List<RoomMemberDto>> roomEnter(String dest, {int kind = 0});
  Future<void> roomLeave(String dest, {int kind = 0});
  Future<void> sendTyping(String dest, {int kind = 0, bool active = true});

  // bot_* 保留到 P4（黑暗期间设置页仍可调；不驱动 ChatAgent）
  Future<PersonDto> botCreate({...});
  Future<void> botDelete(String dest);
  Future<PersonDto> botUpdate({...});
  Future<KimTalkResult> botReply({...});
  Future<List<KimBotPendingItem>> botPending(String dest, {int limit = 20});
  Future<void> botTyping(String dest, {int kind = 0, bool active = true});
}

// P4 才追加 (P2 port 无此二项; FakeKim P2 可空实现到 P4):
//   Stream<AgentRunRequestDto> watchAgentRun();
//   Future<void> submitAgentRun(AgentRunResultDto result);
```

FakeKim（替换 642 行里的 `eventsController.add(KimEvent…)`）：

```dart
class FakeKim implements KimAuthPort, KimClientPort {
  final snapshotCtrl = StreamController<SessionSnapshotDto>.broadcast();
  final eventsCtrl = StreamController<SessionUpdateDto>.broadcast();
  final timelines = <String, StreamController<TimelineUpdateDto>>{};
  SessionSnapshotDto snapshot; // link/threads/unread_total

  void pushSnapshot(SessionSnapshotDto s);
  void pushEvent(SessionUpdateDto e);
  void pushTimeline(String dest, TimelineUpdateDto u);
  // 捷径: 更新 snapshot.threads + timeline Delta
  void fakeIncomingText({required String dest, required String body, int id = 1});
}
```

`KimHarness` 删除 `rustStore` 参数与 `ConversationStore`。

#### P3 token sink

```rust
impl KimSdkHandle {
    pub fn watch_token_persist(&self, sink: StreamSink<TokenPersistDto>);
    pub async fn settings_get(&self) -> Result<SettingsDto, SdkErrorDto>;
    pub async fn settings_patch(&self, ws_url: Option<String>,
        http_origin: Option<String>, env: Option<String>)
        -> Result<SettingsDto, SdkErrorDto>;
}
```

Dart bootstrap：`secure.read(kim.jwt)` → `startSession(url, token)`。监听 `watchTokenPersist`：`Write` → `secure.write`；`Clear` → `secure.delete`。无过期判断。

#### P5 `KimUiHandle.command`

```rust
pub enum UiCommandDto {
    SendText { dest: String, text: String, kind: i32 },
    SendMedia { dest: String, path: String, mime: String,
                width: i32, height: i32, byte_size: i64, kind: i32 },
    RetrySend { client_id: String },
    CancelSend { client_id: String },
    MarkThreadRead { dest: String, kind: i32, visible_message_id: i64 },
    DeleteThread { dest: String },
    FriendRequest { dest: String },
    FriendAccept { dest: String },
    FriendReject { dest: String },
    FriendRemove { dest: String },
    AgentEnqueueTurn { dest: String, text: String, in_reply_to: i64 },
    AgentRespondPermission { dest: String, call_id: String, permission: String },
    AgentAbortTurn { dest: String },
    AgentRunResult { dest: String, profile_id: String, epoch: u64,
                     output: String, error: Option<String> }, // P5 折入 P4 submit_agent_run
    SettingsPatch { ws_url: Option<String>, http_origin: Option<String>, env: Option<String> },
}
```

无 `SignIn` / `SetTheme`。`KimAuthPort.login/register/logout/changePassword` 保留到终态。

---

### Data Model Changes

现有表语义不动。`SCHEMA_VERSION` 按 phase 递增。

**`prepare_store_file`（P2，Rust，`attach_store` 入口，Dart 禁止 wipe）：**

```rust
/// kim-sdk/src/store/prepare.rs
pub fn prepare_store_file(path: &Path) -> Result<PrepareOutcome, SdkError> {
    // 1. 文件不存在 → CreateEmpty（随后 migrate）
    // 2. 打开失败 / 不是 SQLite → remove_file, CreateEmpty, metrics.inc_store_wipe()
    // 3. 只读读 meta.schema_version
    //    缺失 / 不可解析 / parse 后 < 1  → close, remove_file, CreateEmpty, inc_store_wipe
    //    1 <= v < SCHEMA_VERSION          → Keep, 随后 migrate.rs 升级
    //    v == SCHEMA_VERSION              → Keep
    //    v > SCHEMA_VERSION               → Err(SdkError::InvalidArgument {
    //         message: "store newer than sdk" })  // 禁止抹掉更新的 kim-sdk 库
}
```

`attach_store`：`spawn_blocking(prepare_store_file)` → `Store::open` → migrate。`SdkMetrics` 增 `store_wipe_total`。

**删除/闸掉导入器：** `migrate_v1` 中 `INSERT OR IGNORE INTO outbox … FROM messages WHERE status IN ('sending','failed')`（`migrate.rs:61-80`）**删除**。出盒表仍 `CREATE`。漏 wipe 时不得把 Dart pending 行拷进 kim-sdk。可用 `#[cfg(test)]` 保留仅供旧单测，生产路径不编译。

**P3 `SCHEMA_VERSION = 2`：**

```sql
CREATE TABLE IF NOT EXISTS contacts (
  account TEXT NOT NULL,
  peer TEXT NOT NULL,
  relation TEXT NOT NULL,
  nickname TEXT NOT NULL DEFAULT '',
  avatar TEXT NOT NULL DEFAULT '',
  bio TEXT NOT NULL DEFAULT '',
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (account, peer)
);

-- account = '' : device 行 (登录前 URL/env/origin/locale)
-- 非空 : 每账号 agent_flags 等
CREATE TABLE IF NOT EXISTS settings (
  account TEXT NOT NULL PRIMARY KEY,
  ws_url TEXT NOT NULL DEFAULT '',
  http_origin TEXT NOT NULL DEFAULT '',
  env TEXT NOT NULL DEFAULT 'prod',
  locale TEXT NOT NULL DEFAULT '',
  agent_flags TEXT NOT NULL DEFAULT '{}'
);
```

无 theme 列。一次性导入：prefs 空 `kim.account` 时写入 `settings.account = ''` 的 device 行（URL/origin）；token 只进 Keychain，不进 SQLite。`meta.imported_prefs = 1`。

**P4 `SCHEMA_VERSION = 3`：** 仅桌面有意义；电话表可空。pump 过滤读 **列** `server_account`，不从 `body_json` 抠。

```sql
CREATE TABLE IF NOT EXISTS agent_profiles (
  account TEXT NOT NULL,
  profile_id TEXT NOT NULL,
  nickname TEXT NOT NULL,
  server_account TEXT NOT NULL DEFAULT '', -- owned registered bot dest; 对齐 AgentProfile.serverAccount
  body_json TEXT NOT NULL,                 -- 非密钥、非 server_account 的 persona 字段
  key_ciphertext BLOB,                     -- AES-GCM; wrapping key = Keychain kim.agent_wrap, 不是 JWT
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (account, profile_id)
);

CREATE TABLE IF NOT EXISTS agent_permissions (
  account TEXT NOT NULL,
  profile_id TEXT NOT NULL,
  tool TEXT NOT NULL,
  decision TEXT NOT NULL,                  -- always_allow / always_deny
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (account, profile_id, tool)
);
```

**P5 `SCHEMA_VERSION = 4`：** `media_cache(url PK, local_path, byte_size, last_access)`。LRU 512 MiB。

---

### MobileAgent（P4，桌面）

```rust
pub struct MobileAgent { /* 仅桌面 set_agent */
    queues: Mutex<HashMap<String, mpsc::Sender<Turn>>>, // cap 8
    lru: Mutex<VecDeque<String>>,                       // dest×profile, cap 4
    runtime: Arc<dyn AgentRuntime>,
}

#[async_trait]
pub trait AgentRuntime: Send + Sync {
    async fn run_turn(&self, dest: &str, profile_id: &str, text: &str,
                      in_reply_to: i64, epoch: SessionEpoch)
        -> Result<(), SdkError>;
}
```

P4 **命名 FFI**（挂在现有 `KimSdkHandle` 上，**不**等 P5 `command()`）：

```rust
pub struct AgentRunRequestDto {
    pub dest: String,
    pub profile_id: String,
    pub text: String,
    pub in_reply_to: i64,
    pub epoch: u64,
}

pub struct AgentRunResultDto {
    pub dest: String,
    pub profile_id: String,
    pub epoch: u64,
    pub output: String,            // 助手回复正文; 空且 error=Some 表示失败
    pub error: Option<String>,
}

impl KimSdkHandle {
    /// 桌面: MobileAgent 每 turn 推一条. 电话: 空流.
    pub fn watch_agent_run(&self, sink: StreamSink<AgentRunRequestDto>);
    /// Dart AgentBridge 跑完 rust_agent.prompt 后交回.
    pub async fn submit_agent_run(&self, result: AgentRunResultDto)
        -> Result<(), SdkErrorDto>;
}
```

FFI 桌面 `AgentRuntime`：`run_turn` → `sink.add(AgentRunRequestDto)`（有界；满则 `SdkError::Busy { queue: "agent_run" }`），await 对应 `submit_agent_run`（epoch 不匹配则丢）。Dart `lib/bridge/agent_bridge.dart`：`watchAgentRun` → 现有 `rust_agent` `session_open` / `prompt` / `listen` → `submitAgentRun`。**禁止** Dart 持队列/LRU/门闩。`kim_client_ffi` Cargo.toml **不加** `kim-agent-host`。P5 可将 `submit_agent_run` 折进 `UiCommandDto.AgentRunResult`；P4 不能依赖 `command()`。

`AgentCardDto` 与 `AgentToolCard`（`widgets/agent_action_bubble.dart:133-180`）1:1，P4 钉死，不再是 Open Question：

```rust
pub struct AgentCardDto {
    pub v: i32,            // 恒 1
    pub card_type: String, // JSON 键 "type": "tool" | "action_required"
    pub call_id: String,
    pub name: String,
    pub state: String,     // pending | running | ok | error
    pub preview: String,
    pub ok: bool,
}

pub enum AgentTurnStateDto { Queued, Running, WaitingPermission, Done, Error }
```

时间线落库：`MessageView.kind` = agentCard，`body` = `AgentToolCard.encode()` 同形 JSON（`v/type/call_id/name/state/preview/ok`），`AgentActionBubble` / `parse` **零改**。离散 `SessionUpdate::AgentCard { dest, card }` 给权限条；`respondPermission` 经 P4 命名方法（可与 `submit_agent_run` 并列，P5 收进 `UiCommandDto.AgentRespondPermission`）。

- `outbox/pump.rs`：仅当 dest == `agent_profiles.server_account`（非空）才 `enqueue_turn`。电话 `NoopAgent` 立即返回。
- `catch_up`：P4 在桌面 `Link=Online` 或 `SyncProgress { catching_up: false }` 时由 kim-sdk 调用（不是 Dart `catchUpPending`）。内部 `bot_pending` + 入队。
- 电话：**不安装** `MobileAgent`，无队列、无 LRU。

P4 `SessionUpdate` 增补（FRB 再生成一次）：

```rust
AgentTurn { dest: String, state: AgentTurnStateDto, text: String },
AgentCard { dest: String, card: AgentCardDto }, // tagged; 禁止 String JSON payload
```

---

## Key Decisions

| # | 决策（索引） |
|---|---|
| K1 | 无 flag / 无双写 / 无双发；P2 一步切；回滚 = revert |
| K2 | P1 tagged union + MessageView；AgentTurn/Card 推迟 P4 |
| K3 | 一个 handle，P5 改名；登录留 `KimAuthPort` |
| K4 | Snapshot 仅四字段；contacts 离散；列表/链路只 select snapshot；P2 必须 publish session **和** timeline |
| K5 | 分修：删 fat Lagged；persist → snapshot + Inbox wait-send |
| K6 | 启动 token 传入 `start_session`；`watch_token_persist`；独立 `kim.agent_wrap` |
| K7 | `load_older` 含 `before_id`；短页由 Rust 调 history |
| K8 | P2 单 PR，带 port/FakeKim/测试清单 |
| K9 | 覆盖 08 D11；P2 agent stub；P4 `watch_agent_run`/`submit_agent_run`；电话 NoopAgent 无队列 |
| K10 | `prepare_store_file`：`< 1` wipe，`1..N` migrate，`> N` 硬错；删 messages→outbox 导入 |
| K11 | P0 catch 只改活文件 |
| K12 | FTS5 不挡 P5；`search(query, dest?)`：有 dest 限该线程，否则全局 LIKE，封顶 50 |
| K13 | 首次 wipe：DevPanel 面板内提示 + `store_wipe_total`；永不弹 OS 对话框 |

Q2（Goose 放哪）**已关**：P4 `watch_agent_run` / `submit_agent_run` → Dart AgentBridge → `rust_agent`；`kim_client_ffi` 不依赖 host。  
Q3（TokenStore vs start_session）**已关**：启动传入 + persist sink，无 trait。  
Q（AgentCard 字段）**已关**：与 `AgentToolCard` 1:1，见 MobileAgent 节。  
Q（search dest / wipe 提示）**已关**：K12 / K13。

相对 08：保留 D1–D10、D13–D16。**覆盖 D11「不得断回复」**（P2–P4 黑暗）。终止 08「flag 关时 Dart 拥有 db」「胖流过渡到 PR 11」。06 Decision 2/4 保持被推翻。

---

## Phased Implementation

```text
P0  错误 + 全局钩子                 无依赖
P1  typed payload DTO + FRB regen   P2 硬阻塞
P2  store always-on + 事件翻转      依赖 P1
    + 删除 Dart store/outbox/fat
    + snapshot publisher + wipe
P3  contacts + settings/token       依赖 P2     ⎫
P4  桌面 MobileAgent                依赖 P2     ⎬ 可并行
P6  主题 token + 组件库 + golden    无依赖      ⎭
P5  KimUiHandle 改名 + media LRU    依赖 P2–P4；FTS5 不挡
P7  目录重组                        P2–P5 之后
P8  DevPanel + env + panic + 验证   依赖 P5
```

### Phase 0：错误体系与全局钩子（无依赖）

- **新** `core/failures.dart`：sealed `KimException` + 19 kind 全表。
- `core/errors.dart`：`switch (kind)`。
- **新** `core/logger.dart`；`main.dart` 三钩子。
- `state/retry.dart` 看 `KimException.retryable`。
- 空 catch **只改 P4 后仍存在的文件**（`main.dart`、`kim_bridge.dart`、`state/{link,messages,contacts,presence,typing,receipts,auth}`、`core/{errors,media,settings}`）。**不改** `agent_profiles.dart` / `conversation_store.dart` / `outbox.dart` / `jwt.dart`，也不改 P2 将 stub 化的 `chat_agent.dart` / `capability_host.dart`。
- **新** `test/core/failures_test.dart`。

### Phase 1：typed payload DTO（P2 硬阻塞）

- `types.rs` 上节全部类型（无 AgentTurn/Card）。
- `client.rs`：`watch_session_snapshot`、`load_older(..., before_id, ...)`、社交返回 DTO。
- FRB regen。`kim_bridge.dart` 去掉 people/profile/room 的 `jsonDecode`。fat `_talksFromJson` 仍在（P2 删）。
- FakeKim 先跟 DTO 签名，不切 inbox 数据源。P1 App 仍用 fat 聊天。

### Phase 2：store always-on + 事件翻转（依赖 P1）— 单 PR

**Rust checklist**

- [ ] `prepare_store_file` + `store_wipe_total`；Dart 不删文件
- [ ] 删除 `migrate_v1` messages→outbox `INSERT`
- [ ] `publish_session_snapshot` 接到 persist/mark_read/delete/link
- [ ] `publish_timeline(dest)`：subscribe 初值 = 热窗口 Snapshot（≤50+pending），**禁止** Resync-only
- [ ] `persist_talks_for` / `enqueue_message` / `mark_sent` / `delete_thread` / `load_older`（Keep persist 后）调用 `publish_timeline`
- [ ] `persist_inbox_for`：publish snapshot + `emit_session_wait(Inbox)`
- [ ] FFI 删 `session_events` / `persist_talks` / `persist_inbox` / `sync_confirm` / `ack`
- [ ] 暴露 `watch_session_snapshot` / `watch_session` / `watch_timeline` / `load_older`
- [ ] `KimSdk::open` 注释去掉「Flag-on」
- [ ] bootstrap（`main.dart`）始终 `attach_store`（经 FFI，wipe 在 Rust）
- [ ] **不** `set_agent`；Goose 黑暗
- [ ] **不**把 `MobileAgent` / Goose FFI 塞进本 PR

**Dart checklist**

- [ ] `KimClientPort` / FakeKim 换成上节方法表
- [ ] **新** `state/kim_session.dart`：`kimSessionProvider` 持 snapshot
- [ ] `state/inbox.dart`：`ThreadsNotifier` 写路径删除；`threadsProvider = select(snapshot.threads)`（query 过滤可留在 Dart，属 UI）
- [ ] `state/messages.dart`：`watchThread` + `loadOlder` FFI；禁止 `history()` / `conversationStore`。Resync = 丢热窗口等下一条 Snapshot，**禁止**重订阅循环
- [ ] `link.dart`：`linkProvider` 的 **ConnStatus 只** `select(kimSessionProvider.link)`（映射 `LinkStateDto` → `KimLinkState`）。`retry()` 留在同一 tiny notifier 上：`notifyRadioUp`，失败则 `startSession`。删 `_onEvent` 分发、`catchUpPending`、对已删 `linkState()` 的读取。Dart **忽略** `SessionUpdate::Link`
- [ ] `chat_agent.dart` / `capability_host.dart`：**替换为编译期 no-op stub**（见下），不是保持 943/232 行原文件
- [ ] 删 `conversation_store.dart`、`message_repository.dart`、`outbox.dart`、`KimFlags`、`KimRuntime.rustStore`
- [ ] `providers.dart` 删 `conversationStoreProvider`；`harness.dart` 删 `rustStore` / `store` / `ConversationStore` 字段。**不**留 dummy store
- [ ] presence/typing/receipts：离散事件 upsert-by-id

**P2 agent stub（选项 a；禁止把整段 agent 下沉塞进 P2；禁止再引入 flag）：**

`chat_agent.dart` 公开表面保留调用方仍引用的方法，体全部空：

```dart
class ChatAgent {
  ChatAgent(this._ref);
  final Ref _ref;
  Future<void> enqueueTurn(String dest, String text, int inReplyTo) async {}
  Future<void> catchUpPending() async {}
  Future<void> sendDirect({required String dest, required String text}) async {}
  Future<void> respondPermission({
    required String dest, required String callId,
    required String permission, String toolName = '',
  }) async {}
}
final chatAgentProvider = Provider<ChatAgent>(ChatAgent.new);
```

禁止 import / 读 `conversationStoreProvider`、`messageRepositoryProvider`、`clientPortProvider.linkState()`、`threadsProvider.notifier`。`capability_host.dart`：`execute` 立即返回 `'{"error":"unavailable"}'`，禁止 `loadMessages`。P4 删除这两个 stub，换 `agent_bridge.dart`。

**P2 测试清单**

| 文件 | 动作 |
|---|---|
| `test/bootstrap_rust_store_test.dart` | **删** |
| `test/conversation_store_test.dart` | **删** |
| `test/message_repository_rust_test.dart` | **删** |
| `test/support/fake_kim.dart` | **重写** snapshot/delta API |
| `test/support/harness.dart` | **改**：无 store、无 rustStore；`KimHarness` 删 `store` 字段 |
| `test/state/inbox_test.dart` | **重写**：`pushSnapshot` → 列表；删 `rustStore:` 用例 |
| `test/state/link_ordering_test.dart` | **重写**：Kickout/Auth 走 `pushEvent`；无 fat `KimEvent` |
| `test/state/gateway_test.dart` | **重写**：link 来自 snapshot；`retry` 调 `notifyRadioUp` |
| `test/state/outbox_account_test.dart` | **重写**：`enqueueMessage` + timeline delta（删 `env.store`） |
| `test/state/presence_test.dart` / `typing_receipts_test.dart` | **改**事件类型 |
| `test/state/auth_test.dart` | **跟 harness 改**（`kimHarness` 调用方，今日不碰 `env.store` 也要过新签名） |
| `test/state/profile_test.dart` | **跟 harness 改** |
| `test/state/contacts_test.dart` | **跟 harness 改**（P3 再改数据源） |
| `test/signed_out_smoke_test.dart` | **跟 port/harness 改** |
| `test/widget_test.dart` | **跟 harness 改**：自建 `testRuntime` 今日 `ConversationStore.memory()` + `env.store`，P2 删 |
| `test/widgets/**` | **跟 harness 改**（若仍走 `kimHarness` / `env.store` / `conversationStoreProvider`）；纯 widget 泵 MaterialApp 的（如 `conversation_rail_test`）可不动 |
| `test/state/chat_agent_test.dart` | **重写**：stub 不跑 Goose、不碰 store、`enqueueTurn` 无副作用 |
| `test/agent/capability_host_test.dart` | **重写**：`execute` 不读 store，返回 unavailable |
| `test/agent/**`（其余页面测） | **跟 harness 改**；Goose 黑暗；不触发 `session_open` |
| `test/core/jwt_test.dart` / `settings_store_test.dart` | P3 再删 |
| `crates/kim-sdk/tests/store_wipe.rs`（新） | Dart 形库（无 schema_version）被 wipe；v1 保留；v99 `InvalidArgument`；非 SQLite 文件 wipe |
| `crates/kim-sdk/tests/invariants.rs` | persist Inbox 后 snapshot.threads 更新；Kickout 在负载下仍到达（supervisor inject） |
| `crates/kim-sdk/tests/timeline_watch.rs`（新） | persist talk → `subscribe_timeline` 收到带 `MessageView` 的 Snapshot 或 Delta；subscribe 初值不是 Resync-only |
| `crates/kim-sdk/tests/migrate.rs` | 断言生产 migrate **无** messages→outbox INSERT |

凡 `kimHarness(` 调用方都在 P2 范围内：harness 丢掉 store 后，即使测试体从不写 `env.store`，也要编过（签名/override）。`flutter analyze` 必须绿。

**P2 验收（必须全部勾上才合）：**

1. 无 `schema_version` 的 Dart `kim-cache.db` → wipe，空库 v1；`store_wipe_total >= 1`。
2. 已有 kim-sdk v1 库 → 不 wipe，消息还在。
3. FakeKim `fakeIncomingText` → `kimSessionProvider.threads` 更新且 `threadTimelineProvider` 出现 Delta；`threadsProvider` 与 snapshot 同一份。
4. **`cargo test -p kim-sdk`：persist 一条 talk → `subscribe_timeline` 订阅者收到 Snapshot 或 Delta，且 `MessageView.body` 非空。** FakeKim `pushTimeline` 不能替代这条。
5. `enqueueMessage` 文本 → pending 出现在 timeline pending/upserts。
6. Kickout event 在 Fake 与 Rust inject 下到达，触发登出。
7. FakeKim `pushSnapshot(link: Offline)` → `linkProvider` / 离线横幅画出 Offline；`retry()` 只走 `notifyRadioUp`/`startSession`，不读已删 `linkState()`。
8. `flutter test` 上表文件绿（含全部 `kimHarness` 调用方）；`cargo test -p kim-sdk` 绿；`flutter analyze --fatal-infos --fatal-warnings` 绿（agent stub 不引用已删 provider）。
9. 桌面发往 owned bot **不**跑 Goose（NoopAgent / stub）；测试可断言无 `session_open`。

### Phase 3：contacts + settings/token（依赖 P2）

- `contacts.rs` + 表 v2；`SessionUpdate::ContactsChanged`；`contacts.dart` 只 upsert 离散事件，**不** select snapshot.contacts（snapshot 无该字段）。
- `settings.rs` + device 行；`watch_token_persist`；删 `jwt.dart` / `SettingsStore` 业务存储。
- 一次性导入 prefs URL/origin/account → device 行；token Keychain 已在则沿用，fallback prefs 导入后删键。
- 无 token 双写窗口。

### Phase 4：桌面 MobileAgent（依赖 P2；∥ P3）

- `agent/{queue,profiles,sessions,permissions,runtime}.rs`；表 v3（**含 `server_account` 列**）。
- FFI **不**加 `kim-agent-host`。P4 命名方法：`watch_agent_run(StreamSink<AgentRunRequestDto>)`、`submit_agent_run(AgentRunResultDto)`。`lib/bridge/agent_bridge.dart` 转发 `rust_agent.session_open`/`prompt`。
- **钉** `AgentCardDto` ↔ `AgentToolCard`（`agent_action_bubble.dart:133`）；时间线 body 仍是 encode JSON，bubble 零改。
- 删 P2 stub：`chat_agent.dart` / `capability_host.dart`；删 `agent_profiles.dart`。
- pump 过滤 `server_account`；`catch_up` 接线 link-online / sync-done。
- 电话：保持 `NoopAgent`。
- 测试：`agent_port.rs` 队列/Busy/LRU/过滤非 bot；scripted host 仍在 `kim-agent-host`；Dart 测 `watch_agent_run` → stub prompt → `submit_agent_run`。

### Phase 5：改名 + media LRU（依赖 P2–P4）

- `KimSdkHandle` → `KimUiHandle`；`command()`；`rt().block_on` 社交改 async。
- `media.rs`：`fetch` + LRU；删 `KimMediaClient`（头像也走 `media_upload`）。
- 拆光 `kim_bridge.dart`。`search(query, dest: Option<String>)`：有 dest 则该线程 `LIKE`，否则全局 `LIKE`；封顶 50。FTS5 不做。

### Phase 6：主题 + 组件库（无依赖）

- `ThemeExtension<KimTokens>`；prefs `kim.theme` 仅 Dart。
- 组件库；golden light+dark × zh。62 处 `Theme.of` 收敛。

### Phase 7：目录重组（P2–P5 后，纯移动）

`core/` 平台适配；`bridge/`；`design/`；`features/{auth,chats,contacts,profile,agent,settings}/`。

### Phase 8：DevPanel + panic + 验证（依赖 P5）

- `KimEnv`；DevPanel；`me_page` 环境切换删除。
- 首次 wipe 提示：本 boot/session 发生过 wipe，或 `store_wipe_total` 自上次展示后 `> 0` 时，DevPanel **面板内**横幅「本地库已按 kim-sdk schema 重建」+ 计数。`store_wipe_total` 仍进 metrics。**永不**弹 OS alert / `showDialog` / 系统对话框。
- `std::panic::set_hook` → `RustPanic`。
- 验证：`flutter analyze --fatal-infos --fatal-warnings`；`flutter test`；`cargo test -p kim-sdk -p kim-agent-host`；`cargo clippy --workspace -D warnings`；**P4 起** iOS/Android 产物 `nm`/`otool` 无 goose 符号。冒烟：登录→会话→发图→断网重连→主题切换→诊断导出。

---

## PR Plan

| PR | 标题 | 依赖 | 主要文件 | 说明 |
|---|---|---|---|---|
| P0 | mobile: kind-based KimException + 全局钩子 | 无 | `failures.dart`、`errors.dart`、`logger.dart`、`main.dart`、`retry.dart`；**不改**即将删除的 agent/store 文件 | 行为不变 |
| P1 | ffi: Timeline/Session tagged union 带 MessageView | 无 | `types.rs`、`client.rs`、FRB、`kim_bridge` people JSON、FakeKim 签名 | 仍走 fat inbox |
| P2 | mobile: Rust store 一步切 + 删 Dart store/outbox/fat | P1 | 见 Phase 2 checklist；`publish_timeline`；`linkProvider` select；agent stub；`kim_session.dart`；harness 去 store | **风险 PR。** 上表验收全绿。Goose 黑暗 |
| P3 | sdk: contacts + device settings + token persist sink | P2 | `contacts.rs`、`settings.rs`、表 v2、`watch_token_persist`；删 `jwt.dart` | 无 TokenStore trait；无双写 |
| P4 | sdk: 桌面 MobileAgent；电话保持 NoopAgent | P2（∥ P3） | `kim-sdk/src/agent/*`；表 v3 + `server_account`；`watch_agent_run`/`submit_agent_run`；`AgentCardDto`；删 stub | Goose 只经 rust_agent；不改 `rust/Cargo.toml` 加 host |
| P5 | ffi: 改名 KimUiHandle + media fetch/LRU | P2–P4 | `client.rs`、`media.rs`、删 `KimMediaClient` | `search(query, dest?)` LIKE 封顶 50；FTS5 不挡 |
| P6 | ui: KimTokens + 组件库 + golden | 无 | `lib/design/**`、`state/theme.dart` | 可提前 |
| P7 | mobile: lib/ 迁 features/design/bridge | P2–P5 | 目录移动 | 纯移动 |
| P8 | mobile: DevPanel + panic hook + 符号抽查 | P5 | `env.dart`、`dev_panel.dart`、`me_page.dart` | wipe 面板内提示；iOS/Android 无 goose 符号 |

---

## Alternatives Considered

1. **保留 `KIM_RUST_STORE`。** 否决（产品拍板）。回滚 = revert。
2. **fat + watch 双发。** 否决。
3. **第二 opaque 长期并存。** 否决；P5 改名。
4. **contacts/typing 进 snapshot。** 否决。列表只 snapshot；contacts 离散。
5. **theme 进 SQLite。** 否决。
6. **`kim_client_ffi` target 依赖 `kim-agent-host`。** 否决。cfg 漏一次 iOS 就进 goose。Goose 只经已有 `rust_agent`。
7. **Dart→Rust 行级迁移 / 保留 `migrate_v1` INSERT。** 否决。wipe + 删除导入器。
8. **P2 拆 2a/2b。** 否决。用 checklist 让单 PR 可审。
9. **P2 装最小 MobileAgent 以不断 bot 回复。** 否决。测试环境覆盖 08 D11，P2 范围已过大。
10. **TokenStore trait + Dart Fn。** 否决。FRB 2.13 把 closure 当 `Send+Sync` 不值得；启动传入 + `StreamSink` 即可。

---

## Security & Privacy Considerations

| 威胁 | 缓解 |
|---|---|
| Provider key 在 prefs 明文 | P4 AES-GCM；wrapping key = Keychain `kim.agent_wrap`，与 JWT 独立旋转 |
| JWT fallback prefs | P3 导入后删；不进 SQLite |
| JWT 当 AES wrapping key | **禁止**（TokenRenew 后解密失败 + 密钥耦合） |
| 媒体路径穿越 | 现有 `validate_media_path`；`fetch` 只写 cache 目录 |
| Goose 进电话 SO | `hook/build.dart` + `kim_client_ffi` 不依赖 host + 电话 NoopAgent；P4 CI `nm`/`otool` |
| 诊断包泄露 token | `export_diagnostics` 红作 token / key_ciphertext |
| 新版本 db 被 wipe | `version > SCHEMA_VERSION` 硬错，不删文件 |

整库 SQLCipher 不做。Keychain `first_unlock_this_device` 语义保留。

---

## Observability

- Dart `KimLogger` → FFI → `tracing`。P8 rolling 7 天。
- `SdkMetrics` 现有 enqueue/persist/epoch_drop。新增：`store_wipe_total`、`session_event_wait_ms`、`timeline_resync_total`、`agent_queue_busy_total`、`media_cache_evict_bytes`。
- `RustPanic`、连续 `AuthExpired`、`StorageFull`、首次 wipe 横幅（`store_wipe_total`）在 DevPanel 可见。不弹系统对话框。

---

## Risks

| 风险 | 严重度 | 缓解 |
|---|---|---|
| FRB enum 抖动 | 高 | P1 单独；P4 再生成一次（AgentTurn/Card） |
| wipe 谓词写错抹掉 v1 | 高 | `prepare_store_file` 单测：`<1` wipe、v1 留、v99 错 |
| 漏删 migrate INSERT 导入 Dart pending | 高 | P2 删除该 SQL；测试断言源码/行为 |
| snapshot 无 publisher | 高 | P2 checklist 强制 `publish_session_snapshot` |
| timeline watch 死 / Resync 循环 | 高 | `publish_timeline`；subscribe 初值 Snapshot；Dart Resync 不等价于重订阅；`cargo test` persist→MessageView |
| P2 删 store 后 agent 文件编不过 | 高 | P2 stub 去掉 store/`linkState`/`threads.notifier`；测试改「无 Goose / 无 store」 |
| `linkProvider` 无 ConnStatus | 高 | `select(snapshot.link)` + tiny `retry`；FakeKim Offline 横幅 |
| 第二份 `threadsProvider` | 高 | `inbox.dart` 写路径删除；只 select |
| P2–P4 桌面 bot 静默 | 中 | 产品接受（覆盖 D11）；P4 恢复 |
| Goose 链进 iOS | 高 | 不改 `kim_client_ffi` deps；P4 符号抽查 |
| FakeKim 642 行爆炸 | 高 | P2 一次换成 snapshot API |
| P2 单 PR 过大 | 高 | P1 已落地 DTO；checklist + 全量测试；revert 回滚 |

---

## Architectural Notes

- **回滚：** 无运行时开关。`git revert` 该 PR。
- **isolate 消失：** 随 `ConversationStore` 删除。
- **性能：** Delta 60fps；Snapshot ≤50 + `load_older`；桌面 agent 队列 8；LRU 512 MiB。
- **协议不变。** `KimAuthPort` 终态仍在。
- **双 FFI 不合并；`kim_client_ffi` 不依赖 host。**
- **新增依赖：** Rust `image`。Dart 零新增。
- **08 D11：** 本文件覆盖「中间不得断回复」。其余 08 深模块决策保留。

---

## File Change Summary

- `.github/workflows/ci.yml` — golden 分片；P4+ iOS/Android 无 goose 符号
- `crates/kim-sdk/src/agent.rs` — 桌面 `MobileAgent`；电话不安装
- `crates/kim-sdk/src/agent/{permissions,profiles,queue,runtime,sessions}.rs` — 新（P4）
- `crates/kim-sdk/src/contacts.rs` — 新（P3）
- `crates/kim-sdk/src/lib.rs` — `publish_session_snapshot`；`publish_timeline`（subscribe 初值 Snapshot）；Inbox wait-send；`open` 去 Flag-on 注释
- `crates/kim-sdk/src/media.rs` — fetch + LRU
- `crates/kim-sdk/src/metrics.rs` — `store_wipe_total`
- `crates/kim-sdk/src/outbox/pump.rs` — P4 起过滤 owned bot
- `crates/kim-sdk/src/settings.rs` — 新；device 行
- `crates/kim-sdk/src/store/prepare.rs` — 新：`prepare_store_file`
- `crates/kim-sdk/src/store/migrate.rs` — **删除** messages→outbox INSERT；v2–v4
- `crates/kim-sdk/src/store/schema.rs` — 版本递增 + 新表
- `crates/kim-sdk/src/store/{contacts,settings,agent_profiles,agent_permissions,media_cache}.rs` — 新表
- `crates/kim-sdk/src/timeline.rs` — snapshot 加 `unread_total`；P3 ContactsChanged；P4 Agent*；P8 RustPanic
- `crates/kim-sdk/tests/store_wipe.rs` — 新
- `crates/kim-sdk/tests/invariants.rs` — snapshot publish + Kickout 负载到达
- `crates/kim-sdk/tests/timeline_watch.rs` — persist talk → 订阅者收到 MessageView Snapshot/Delta
- `sdk/mobile/lib/agent/capability_host.dart` — P2 stub（无 store）；P4 删除
- `sdk/mobile/lib/bridge/agent_bridge.dart` — P4：`watch_agent_run` / `submit_agent_run` 薄转发 rust_agent
- `sdk/mobile/lib/bridge/kim_ui_bridge.dart` — P5
- `sdk/mobile/lib/core/{env,failures,logger}.dart` — 新
- `sdk/mobile/lib/core/errors.dart` — switch(kind)
- `sdk/mobile/lib/core/jwt.dart` — P3 删除
- `sdk/mobile/lib/core/media.dart` — P5 删 HttpClient
- `sdk/mobile/lib/core/runtime.dart` — 删 rustStore
- `sdk/mobile/lib/core/settings.dart` — P2 删 KimFlags；P3 删存储
- `sdk/mobile/lib/data/conversation_store.dart` — P2 删除
- `sdk/mobile/lib/data/message_repository.dart` — P2 删除
- `sdk/mobile/lib/kim_bridge.dart` — P1 去 people JSON；P2 删 `sessionEvents`/`persist*`/`ack`/`syncConfirm`/`history`/`attachStore`；实现新 port
- `sdk/mobile/lib/main.dart` — 错误钩子；P2 始终 `attachStore`（wipe 在 Rust）
- `sdk/mobile/lib/screens/home/me_page.dart` — P8 删环境切换
- `sdk/mobile/lib/state/agent_profiles.dart` — P4 删除
- `sdk/mobile/lib/state/chat_agent.dart` — P2 stub（无 store/`linkState`/`threads.notifier`）；P4 删除
- `sdk/mobile/lib/state/inbox.dart` — P2：select(snapshot.threads)；删 merge/persist
- `sdk/mobile/lib/state/kim_session.dart` — 新
- `sdk/mobile/lib/state/link.dart` — `linkProvider` = select(snapshot.link)；`retry` = notifyRadioUp/startSession
- `sdk/mobile/lib/state/messages.dart` — watch_thread + load_older；Resync 不等价于重订阅
- `sdk/mobile/lib/state/outbox.dart` — P2 删除
- `sdk/mobile/lib/state/contacts.dart` — P3 离散事件
- `sdk/mobile/lib/state/{presence,receipts,typing}.dart` — upsert-by-id
- `sdk/mobile/lib/state/providers.dart` — 删 conversationStoreProvider
- `sdk/mobile/rust/src/api/client.rs` — snapshot watch、load_older、删 fat/persist/ack；P4 `watch_agent_run`/`submit_agent_run`；P5 改名
- `sdk/mobile/rust/src/api/types.rs` — tagged union 全集；P4 `AgentRunRequestDto`/`AgentRunResultDto`/`AgentCardDto`
- `sdk/mobile/rust/Cargo.toml` — **不**加 kim-agent-host
- `sdk/mobile/test/bootstrap_rust_store_test.dart` — P2 删除
- `sdk/mobile/test/conversation_store_test.dart` — P2 删除
- `sdk/mobile/test/message_repository_rust_test.dart` — P2 删除
- `sdk/mobile/test/support/fake_kim.dart` — P2 重写 snapshot/delta
- `sdk/mobile/test/support/harness.dart` — 删 rustStore/store
- `sdk/mobile/test/state/{inbox,link_ordering,gateway,outbox_account,auth,profile}_test.dart` — P2 跟 harness 改 / 重写
- `sdk/mobile/test/widget_test.dart` — 删 `ConversationStore.memory()` / `env.store`
- `sdk/mobile/test/widgets/**` — 跟 harness 改（用到 store/harness 的）
- `sdk/mobile/test/state/chat_agent_test.dart` / `test/agent/capability_host_test.dart` — P2 重写为无 Goose / 无 store
- `sdk/mobile/test/core/failures_test.dart` — 新
- `sdk/mobile/test/core/jwt_test.dart` — P3 删除
- `sdk/mobile/test/design/golden/` — P6 新

`machine.rs` 不搬进 kim-sdk。`hook/build.dart` 闸门保留。`rust_agent/` 保留。

---

## Open Questions

**无未决开放问题。**

已关决策（不再讨论）：Q2 Goose 只经 `rust_agent`；Q3 token 启动传入 + persist sink；AgentCard 与 `AgentToolCard` 1:1；`search(query, dest?)` 有 dest 限该线程否则全局 LIKE、封顶 50（K12）；首次 wipe 在 DevPanel 面板内提示 + `store_wipe_total`，永不弹 OS 对话框（K13）。

---

## References

- [docs/impl/08-kim-sdk-ownership.md](./08-kim-sdk-ownership.md) — 本文件覆盖其 D11；其余深模块决策保留
- [docs/impl/06-mobile-client-maturity.md](./06-mobile-client-maturity.md)
- [docs/mobile-client.md](../mobile-client.md)
- [docs/impl/07-mobile-link-control.md](./07-mobile-link-control.md)
- [docs/media.md](../media.md)
- HEAD：`crates/kim-sdk/src/{lib,agent,timeline,media,store/{schema,migrate},outbox/pump,metrics,session}.rs`；`sdk/mobile/rust/src/api/{client,types}.rs`；`sdk/mobile/{hook/build.dart,lib/kim_bridge.dart,lib/state/inbox.dart,lib/main.dart}`；`crates/kim-agent-host/src/machine.rs:29`
