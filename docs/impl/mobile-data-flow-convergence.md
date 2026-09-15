# 收拢提交后查询发布并删除 Dart 业务合并

| 字段 | 值 |
|---|---|
| 状态 | Draft |
| 作者 | — |
| 日期 | 2026-09-15 |
| 对照代码 | HEAD `b661d2c`（`refactor(mobile): make Flutter a UI shell over kim-sdk (#126)`）。行号以标识符为准；下文引用均已对当前树核验。 |
| 父规格 | [08-kim-sdk-ownership.md](./08-kim-sdk-ownership.md)；[09-mobile-production-architecture.md](./09-mobile-production-architecture.md)（消息主链路已落地 Flutter-as-shell）。本文件是数据流收拢的可实施切片。 |
| 范围 | `crates/kim-sdk` 提交后查询发布；`sdk/mobile/rust` FFI 薄适配；`sdk/mobile/lib/features/{chats,contacts,session}` Dart 消费面。不动 gateway / chat / royal / ACK 协议。 |
| 铁律 | Dart 不拥有业务持久化或编排。Dart：渲染、手势、平台 API、生命周期转发。无回滚开关、无双写、无 fat event。Store 写队列保持串行。 |

---

## Breaking Change Notice（仓库内部）

不涉外部 crate 与服务端。分阶段破坏，禁止跨 phase 混用新旧 Dart 调用约定。

### Phase 1（FFI 签名不变；下列行为变化必须在 PR 中写进测试）

签名不变，**不是**行为中立。PR 对齐：PR 1 只加 dirty 记录、保留手工 publish；PR 2 切到 QueryPublisher（无 dual-publish）；PR 3 只改 `mark_read` 时序。

1. **PR 2：** 业务入口删除手工 `publish_timeline` / `publish_session_snapshot`；改 ChangeLog → QueryPublisher。外部方法签名不变。
2. **PR 2：** `persist_talks_for` / `persist_inbox_for` 在 commit 成功后立即返回，不等待查询刷新。协议 ACK 变快；UI 经 watch 收敛。
3. **PR 2：** `cancel_send` 经 CommitEffect 同时刷新 Timeline **和 Inbox**（HEAD 只 `publish_timeline`）。
4. **PR 2：** `persist_talks` 只对 **resolved dest + 非 no-op** 的行刷新（见 D5）。HEAD 按输入 `talk.dest` 发布，空 dest 会对 `""` 调 `publish_timeline`。
5. **PR 2：** QueryPublisher 绑定 **Store 寿命** token，不绑 `Inner.cancel`。`start_session` 不再杀死 publisher。
6. **PR 2：** `subscribe_timeline` 仍按 dest 复用同一个 `watch::Sender`；`TimelineSub` 盖戳 `{account, epoch, limit}`。**不** drain/drop sender，**不**在 bump_epoch 时发 Resync。
7. **PR 2：** `delete_thread` **保留** `TimelineUpdate::Resync { reason: "deleted" }`（命令路径，不进 dirty 枚举），然后 record Timeline+Inbox。Phase 1 Dart 仍有 `_older`，禁止只发空 Snapshot。
8. **PR 2：** `load_older` **继续**在 persist/history 失败时返回已有本地页（HEAD `let _ =` 语义）。只打 `warn`，不把失败变成 FFI 抛错，不把 `has_more` 置假。`history_error` 是 Phase 3 字段。
9. **PR 3：** `mark_read` 在本地 commit + `wait_applied` 后返回；网络 `protocol.mark_read` 改为同 epoch 后台发送。返回类型仍是 `Result<(), SdkError>`。

### Phase 2（通讯录契约，同 PR 整组切换）

1. `KimUiHandle::refresh_contacts`：`Result<Vec<PersonDto>, String>` → `async Result<(), SdkErrorDto>`。不再 `rt().block_on`，不再返回列表。
2. 新增 `watch_contacts(StreamSink<ContactsSnapshotDto>)`。列表唯一来源。
3. `KimClientPort.refreshContacts()`：`Future<List<PersonDto>>` → `Future<void>`。新增 `Stream<ContactsSnapshotDto> watchContacts()`。
4. `ContactsNotifier` **删除全部列表写路径**（数据只来自 `watchContacts` + `withLocalAgents` 展示映射）：
   - `applyChanged`（刷新返回值与 `ContactsChanged`）
   - `onRequest` 插入 incoming
   - `onAccepted` 乐观插入 friends + 本地 `refresh()`
   - `removePeer` 本地 `where != dest`
   - `onProfileUpdated` 本地 patch nickname/avatar
5. `FriendRequest` / `FriendAccepted` / `ProfileUpdated` 仍可走 `watchSessionEvents` **只做 haptics**；名册由 SDK upsert 后经 contacts watch 到达。
6. 生成的 FRB/Dart 类型走现有生成流程，不手改 `frb_generated.*`。`contacts_test.dart` / `FakeKim` 同步改。

### Phase 3（聊天窗口与命令回执，同 PR 整组切换）

1. `loadOlder`：Dart `Future<MessagePageDto>` → `Future<void>`。结果只经 `watchThread`。Rust `KimSdk::load_older(dest)` 扩展已注册窗口。
2. `sendMessageMutation` / `sendImagesMutation`：`Mutation<KimChatMsg>` / `Mutation<List<KimChatMsg>>` → `Mutation<KimCommandReceipt>` / `Mutation<List<KimCommandReceipt>>`。
3. 删除 `ThreadMessagesNotifier._older` 合并、`receiveAll`、`chat_session._enqueueText/_enqueueImages` 二次构造 `KimChatMsg`。
4. `TimelineSnapshot` 增加 `loading_older` / `history_error`；FFI DTO 同步，FRB 再生成一次。
5. **Composer 行为（HEAD 不是这样）：** HEAD `kim_composer.dart:99-107` `_submit` 在 `onSend(text)` 之后立刻 `_controller.clear()`，不等待发送。目标：仅当 `enqueueMessage` 返回受理回执后清空（`chat_page.dart:328` 已 `await sendText`；把 clear 从 composer 挪到 `_send` 成功分支）。这是可见 UX 变化，同 PR 改 golden/手测说明。

仓库外消费者：无（内部 App + `FakeKim`）。每个破坏 phase 的 PR 必须同改 bridge、FakeKim、调用者、测试。

---

## Feasibility Assessment

09 已把消息主链路收进 `kim-sdk`：写队列、outbox、persist-then-ack、timeline/session watch。本切片收的是**提交后发布分散**和 **Dart 仍合并的残余面**，不重建 store/outbox。

核验过的接缝（HEAD `b661d2c`）：

| 接缝 | HEAD 事实 | 本文件动作 |
|---|---|---|
| 写队列 | `Store::open`（`store/mod.rs:173`）`WRITE_CAP=128`；`write_worker`（`:688`）串行；满则 `SdkError::Busy` | 保持串行。commit 成功后记 dirty，不在 worker 里跑查询 |
| 发送事务 | `persist_enqueue`（`:1145`）同一 `BEGIN IMMEDIATE` 写 `messages` + `outbox` + `threads` + version | 不变。CommitEffect = Timeline{dest}+Inbox |
| 接收事务 | `persist_talks_tx`（`:1277`）`apply_talk` + `threads::apply_incoming` + bump + prune。`apply_talk` 只对空 dest/body 返回 `None`（`messages.rs:327-328`）；重复行仍 `Some` 且 bump version | CommitEffect 用 **ApplyOutcome.dest**；仅 `needs_publish` 才刷新/bump（见 D5） |
| 手工发布 | `publish_timeline`（`lib.rs:1012`）8 处调用；`publish_session_snapshot`（`:976`） | Phase 1 删除业务入口中的调用，改 QueryPublisher |
| ACK | Live：`supervisor.rs:143` persist 成功且 `id != 0` 才 `ack`；离线：`sync.rs:191` persist 后 `:220` `ack_batch` | **不改**。PersistHook 不再等待 UI 查询 |
| 热窗口 | `load_hot_window`（`messages.rs:157`）6 条独立 SELECT，sent `LIMIT min(limit,50)` | Phase 1 收进单读事务。默认仍 Snapshot |
| 订阅 | `subscribe_timeline`（`lib.rs:945`）`HashMap<String, Sender>` dest 键；`entry.or_insert_with` **复用** sender；`start_session` **不** drain。`bump_epoch`（`session.rs:15-25`）取消 `Inner.cancel`，`child_token()` 会随 `start_session:161` 死掉 | dest 键保留；`TimelineSub` 盖戳 account/epoch；publisher 绑 Store 寿命 token，不绑 `Inner.cancel` |
| 通讯录 | FFI `refresh_contacts`（`client.rs:669`）`block_on` 网络 + `replace_contacts` + 返回 `Vec`。`WriteOp::ReplaceContacts` **不校验 epoch**（`store/mod.rs:882`） | Phase 2 下沉 SDK；Phase 1 补 epoch |
| Dart 消息 | `messages.dart:103` 热窗口 + `_older`；`:179` `loadOlder` 再合并；`:166` `receiveAll` **全仓库零调用** | Phase 3 删除 |
| Dart 发送 | `chat_session.dart:294` 忽略 `KimCommandReceipt`，再造 `KimChatMsg`；`mutations.dart:11` 返回消息模型 | Phase 3 改为回执 |
| Dart 通讯录 | 列表写路径：`refresh`→`applyChanged`（`:172`）、`ContactsChanged`→`applyChanged`（`:164`）、`onRequest`（`:308`）、`onAccepted` 插入+`refresh`（`:326`）、`removePeer` filter（`:253`）、`onProfileUpdated`（`:347`）、`withLocalAgents` | Phase 2：前六条删除；`withLocalAgents` 留下作展示映射 |
| 对端已读 | `receipts.dart:22` Dart 内存单调合并 | 已拍板（2026-09-15）：本切片不落库；重启不保留 |

### 与初步规划文档的行号偏差（以 HEAD 为准）

| 初步文档声称 | HEAD 事实 |
|---|---|
| `outbox/pump.rs:64` 发送结果后手工 publish | `:64` 是 `Ok((message_id, _))` 匹配臂；`publish_timeline` 在 `:76`（sent）、`:107`（retry）、`:113`（failed） |
| `supervisor.rs:142` persist 成功后 ACK | 注释在 `:142`；`with_persist` 在 `:143`；ACK 在 persist `is_ok() && id != 0` 之后（`:155-161`） |
| `lib.rs:707` 通讯录落库后发 `ContactsChanged` | `replace_contacts` 确在 `:707`；随后 `:713` `emit_session_wait(ContactsChanged)`。无独立 contacts watch |
| 09 仍写 subscribe 初值 Resync、publisher 不存在 | **已过时。** HEAD 初值是空 `Snapshot`（`lib.rs:948`），persist/enqueue **会** `send` Snapshot。本切片收拢的是分散调用，不是从零造 publisher |
| `SCHEMA_VERSION = 1`（09） | HEAD `schema.rs:1` **`SCHEMA_VERSION = 4`**。本切片不加表、不加列 |

**Feasible with caveats。** 地基已在：写队列、同一事务 pending+outbox、watch Snapshot、persist-then-ack。需要补的是 dirty 集合、发布器串行刷新、订阅作用域，以及通讯录/分页的 Dart 合并删除。Phase 1 可在不改 FFI 的情况下独立合入。

---

## Current Surface Inventory

每一行都是本切片会碰或必须保持行为的调用点。

### kim-sdk 公开 / crate 内入口

- `KimSdk::enqueue_message`（`lib.rs:233`）：`store.enqueue` → `publish_timeline` + `publish_session_snapshot` → `kick_outbox` → `CommandReceipt`。
- `KimSdk::cancel_send`（`:253`）：`store.cancel` → 若有 dest 则 `publish_timeline`。不刷新 inbox。
- `KimSdk::retry_send`（`:266`）：`store.requeue` → `publish_timeline` → `kick_outbox` → `CommandReceipt { Pending }`。
- `KimSdk::mark_read`（`:291`）：`mark_read_local` → **await** `proto.mark_read`（忽略错误）→ snapshot + timeline。
- `KimSdk::delete_thread`（`:313`）：`cancel_outbox_run` → `delete_thread` → `publish_timeline_resync` + snapshot。
- `KimSdk::load_older`（`:573`）：本地页；不足且 `before_id != 0` 时 `protocol.history` → `persist_talks_for(..., Keep)`（错误被 `let _ =` 吞掉）→ 再读 → `publish_timeline` → 返回 `MessagePage`。
- `KimSdk::persist_talks` / `persist_talks_for`（`:622` / `:633`）：按**输入** dest 去重后 persist → 每个 dest `publish_timeline` → snapshot。
- `KimSdk::persist_inbox` / `persist_inbox_for`（`:676` / `:685`）：persist → snapshot → `emit_session_wait(Inbox)`（Dart 已忽略 Inbox）。
- `KimSdk::replace_contacts`（`:707`）：`store.replace_contacts`（无 epoch）→ `ContactsChanged` wait-send。
- `KimSdk::load_contacts`（`:718`）：读本地，无发布。
- `KimSdk::load_threads`（`:701`）：读本地。
- `KimSdk::subscribe_session`（`:797`）：mpsc 64，Kickout/token/friend 等离散事件。
- `KimSdk::subscribe_session_snapshot`（`:941`）：`watch::Receiver<SessionSnapshot>`。
- `KimSdk::subscribe_timeline`（`:945`）：按 dest 插入 `watch::Sender`；eager `publish_timeline_limit(dest, limit)`。
- `KimSdk::publish_timeline`（`:1012`）：`load_hot_window(account, dest, 50)`，无订阅则丢。
- `KimSdk::publish_session_snapshot`（`:976`）：supervisor link + `load_threads` + fold unread；`let _ = send`。
- `SdkPersistHook`（`:1075`）：`persist_talks_for` / `persist_inbox_for`；错误映射 `sdk_to_persist`（`:1197`）。
- `outbox::pump::run_once`（`pump.rs:6`）：`mark_sent` / `mark_retry` / `mark_failed` 后各 `publish_timeline`。
- `spawn_session_bridge`（`lib.rs:820`）：`Link` 时 `publish_session_snapshot`；Lagged 时 snapshot + `recover_lagged_fatal`。

### Store 写操作（`WriteOp`，`store/mod.rs:38`）

- `Enqueue` → `persist_enqueue`（`:1145`）：message(pending)+outbox+thread+version+prune。
- `PersistTalks` → `persist_talks_tx`（`:1277`）。
- `PersistInbox` → `persist_inbox_tx`（`:1317`）。
- `Cancel` / `DeleteThread` / `MarkSent` / `MarkFailed` / `MarkRetry` / `Requeue` / `MarkRead` / `DueNow`：均校验 epoch（`:697` 起）。
- `ReplaceContacts`（`:882`）：**不校验 epoch**。
- `UpsertDeviceSettings` / Agent profile / Media：本切片不发布 UI 查询。

### kim-client（只核验 ACK 边界，不改协议）

- `SessionSupervisor::with_persist`（`supervisor.rs:143`）：live mpsc 64，`try_send` 满则不 ACK、不堵读循环。
- `SyncEngine::run`（`sync.rs:150`）：hook persist inbox（`:162`）、persist talks Keep（`:191`）、然后 `ack_batch`（`:220`）。失败发 `SyncFailed`、不 ACK。
- `PersistHook`（`persist.rs:22`）：`persist_talks` / `persist_inbox`。

### FFI（`sdk/mobile/rust`）

- `KimUiHandle::watch_session_snapshot`（`client.rs:276`）：watch 转 StreamSink。
- `watch_session`（`:299`）：mpsc 转 StreamSink。
- `watch_timeline`（`:313`）：`subscribe_timeline` + borrow/add/changed 循环。
- `enqueue_message`（`:361`）→ `KimCommandReceipt`。
- `load_older`（`:439`）→ `MessagePageDto`。
- `refresh_contacts`（`:669`）：同步 `block_on` friend_list + friend_incoming → `replace_contacts` → 返回 `Vec<PersonDto>`。

### Flutter 消费

- `KimClientPort.watchThread`（`kim_bridge.dart:57` / `:416`）。
- `enqueueMessage`（`:74` / `:495`）→ 桥接层 `KimCommandReceipt`（`sendStatus: String`）。
- `loadOlder`（`:92` / `:424`）。
- `refreshContacts`（`:185` / `:817`）。
- `ThreadMessagesNotifier`（`messages.dart:43`）：`_onSnapshot` 合并 `_older`；`loadOlder` 再合并；`receiveAll` 死代码。
- `ThreadsNotifier`（`inbox.dart:73`）：`select(kimSessionProvider.threads)` + 本地 agent 行。
- `KimSessionNotifier`（`kim_session.dart:78`）：`watchSessionSnapshot`。
- `ContactsNotifier`（`contacts.dart:89`）：事件 + 刷新返回值三路合并；Online 才 refresh。
- `ChatSessionNotifier._enqueueText`（`chat_session.dart:294`）/ `_enqueueImages`（`:320`）：丢弃 SDK 回执，本地造 `KimChatMsg`。
- `sendMessageMutation`（`mutations.dart:11`）：`Mutation<KimChatMsg>`。
- `ReceiptsNotifier.applyPush`（`receipts.dart:22`）：对端已读内存表。
- `FakeKim.refreshContacts`（`fake_kim.dart:766`）：返回列表并 `pushEvent(ContactsChanged)`。

---

## Design

### Goals & Non-Goals

**Goals**

1. 持久业务数据：合并 → SQLite commit → 发布可重建视图。UI 不待命网络成功。
2. 提交后的查询刷新只有一个入口。写路径不再枚举「该 publish 哪些页面」。
3. 短暂状态（typing / presence / upload progress / 对端已读）保持有作用域的内存。已拍板：本切片对端已读不落库。
4. 发送：同一事务 `message(pending)+outbox`，提交后显示 pending，后台发送。Flutter 不持第二份乐观列表。
5. persist-then-ACK 停在持久化成功；ACK 不等 Flutter 渲染，也不等查询刷新。
6. `KimSdk` 继续做 Repository / 数据入口。不为每个操作加 Dart Repository/Service/UseCase。
7. Snapshot-first。`tokio::watch` 只保留最新值，**禁止**把必须按序应用的 Delta 放上 watch。Delta 仅在 Phase 4 测量之后另开切片。
8. 三步清理，顺序锁定：① 统一提交后查询发布；② 统一通讯录出口；③ SDK 拥有聊天窗口 + 命令回执。

**Non-Goals**

- 不改 gateway / chat / royal / kim-tcp，不改 ACK 协议，不关 G-03。
- 不引入泛型 Repository、事件溯源、新的跨 crate 依赖。
- 不换 Riverpod，不在 Dart 侧开 SQLite。
- 不把 Delta 作为本切片默认协议。
- 不重写 store 写队列 / outbox 状态机。
- 不对端已读持久化、不做本端已读可靠上报队列、不做 AI 流式检查点（除非后续产品拍板）。
- 不发明 `Publisher` / `Repository` trait（单一实现，无能力边界）。

### Target-state diagrams

分层（目标）：

```text
┌ Flutter  render / gesture / IME / scroll-anchor / select map
├ bridge     KimClientPort (FakeKim seam)
├ FFI        watch snapshot / events / timeline [/contacts ★]
│            named commands; no block_on on refresh ★
├ kim-sdk    Store write_worker (serial, existing)
│            ChangeLog dirty-set + QueryPublisher ★
│            contacts sync (Phase 2) ★ / window (Phase 3) ★
├ kim-client WSS / SyncEngine / PersistHook (unchanged)
└ SQLite     WAL, schema v4 (no migration this slice)
```

Legend: `──►` call   `══►` async/fanout   `┄►` optional   `★` this slice   `✂` removed

#### 接收：当前 vs 目标

当前：

```text
WGateway ══► live_persist mpsc(64)
                │
                ├─ PersistHook.persist_talks(IfInserted)
                │     └── persist_talks_for
                │           ├── dests = input talks   ← 含未 apply 的重复
                │           ├── Store PersistTalks
                │           ├── publish_timeline x N  ← 手工
                │           └── publish_session_snapshot
                └─ ACK if persist Ok && id != 0
```

目标：

```text
WGateway ══► live_persist mpsc(64)
                │
                ├─ PersistHook.persist_talks(IfInserted)
                │     └── persist_talks_for
                │           └── Store PersistTalks
                │                 commit ──► ★ ChangeLog.record(applied dests)
                │                 return     ← 不等 QueryPublisher
                └─ ACK if persist Ok && id != 0
                          ★ QueryPublisher ══► watch Snapshot ══► Dart
```

离线页同构：`sync.rs` persist Keep 成功后 `ack_batch` 不变；发布改走 ChangeLog。

#### 发送：当前 vs 目标

当前：

```text
Composer ──► enqueueMessage FFI ──► KimSdk.enqueue_message
                                      │
                                      ├─ persist_enqueue (msg+outbox)
                                      ├─ publish_timeline            ← 手工
                                      ├─ publish_session_snapshot    ← 手工
                                      └─ kick_outbox
                                           │
                                           └─ pump mark_sent/fail
                                                └── publish_timeline ← 手工
chat_session ──► 丢弃 CommandReceipt，再造 KimChatMsg  ✂ Phase 3
```

目标：

```text
Composer ──► enqueueMessage ──► persist_enqueue
                                  commit ──► ★ ChangeLog.record(Timeline+Inbox)
                                  wait_applied(seq) ★  （仅命令路径）
                                  kick_outbox
                                       │
                                       └─ pump mark_* ──► ★ ChangeLog.record
                                              不等待 publisher
watchThread ══► pending/sent/failed   （Flutter 不插第二份列表）
enqueue ──► CommandReceipt            （Phase 3 起页面只拿回执）
```

#### 历史加载：当前 vs 目标

当前：

```text
loadOlder Dart ──► FFI load_older ──► 本地页
                      ┄► history + persist_talks_for (err swallowed)
                      └── publish_timeline(hot 50)
Dart: page ∪ _older ∪ hot snapshot     ← 第二份合并
```

目标 Phase 1（兼容）：

```text
load_older ──► 本地页
              ┄► history ──► persist (Keep) ──► ★ ChangeLog
              wait_applied ★
              再读本地页并返回          （FFI 仍返回 MessagePage）
Dart _older 合并仍在                   （Phase 3 删除）
```

目标 Phase 3：

```text
loadOlder() ──► KimSdk.load_older(dest)
                  cursor = snapshot oldest (at, key, before_id)
                  load_page
                    ├─ SQLite has_more ──► stop (extend bound)
                    ├─ sent ≥ 400 ──► has_more=false; ✂ history
                    └─ sent < 400 & short & before_id≠0
                          ──► protocol.history + persist Keep
                  record Timeline ══► Snapshot(hot ∪ window)
Dart: 只渲染 snapshot.items；loading_older 来自 snapshot
```

#### 通讯录：当前 vs 目标

当前：

```text
Online ──► ContactsNotifier.refresh
              └── FFI refresh_contacts (block_on) ──► replace_contacts
                    ├── emit ContactsChanged
                    └── return Vec ──► applyChanged     ← 返回值改列表
FriendRequest/Accepted ══► onRequest/onAccepted          ← 事件改列表
removePeer ──► 协议成功后本地 filter                     ← 第三路
离线进入：friends=[] （不读 load_contacts）
```

目标 Phase 2：

```text
subscribe watchContacts ──► 立即本地 ContactsSnapshot ══► UI
refreshContacts() ──► SDK friend_list+incoming
                        replace_contacts ──► ★ ChangeLog.Contacts
                        return ()                 ← 命令完成，不是列表
Friend* ══► SDK merge/refresh ──► 同一 snapshot
removePeer ──► SDK 删除行 ──► 同一 snapshot
离线：首帧即本地缓存；sync_error 与列表分离
```

#### 提交后发布（本切片核心）

```text
write_worker
    │
    ├─ epoch != current ──► StaleEpoch, no dirty
    ├─ txn fail ──────────► ROLLBACK, no dirty
    └─ COMMIT ok
          └── ★ ChangeLog.record(account, epoch, queries, seq++)
                    notify_one
                         │
                         ▼
              ★ QueryPublisher (one task)
                    take dirty set (coalesce)
                    epoch mismatch? drop
                    serial refresh:
                      Inbox    → SessionSnapshot send
                      Timeline → load_hot_window / window, version gate
                      Contacts → load_contacts (Phase 2 watch)
                    skip Timeline/Contacts if no subscriber
                    applied_seq = batch.seq; notify waiters
```

### Key Design Decisions

1. **写 worker 只负责 commit + dirty-mark；查询刷新归独立 publisher task。** 否决在 `write_worker` 里 `load_hot_window`：那会把读放大串进写队列，拖住入站 persist 与 ACK。否决调用方继续手写 `publish_*`：新写路径会漏 inbox。
2. **dirty 用可合并集合 + `Notify`，不用有界 `try_send`。** 否决 `mpsc::try_send(CommitNotice)` 且满则丢：最后一次提交可能永远不被观察。`BTreeSet<ChangedQuery>` 上限是「打开的 dest 数 + Inbox + Contacts」，不是每条消息一条通知。
3. **`sequence` 是进程内 epoch 作用域计数，不进数据库。** 否决用它替代服务端 sync cursor / `timeline_meta.version`。`timeline_meta.version` 仍由写事务 bump，给 Snapshot 带版本。publisher 用 `sequence` 做 wait_applied 与过期查询丢弃。
4. **命令路径 `wait_applied`；PersistHook / outbox pump 不等待。** 否决 PersistHook 等待查询：那会把 ACK 绑到 UI 查询。超时 2s 仍返回成功（数据已提交，watch 会追上）。`wait_applied` 的理由是：`chat_page.dart:328-330` `await sendText` 成功后 `scrollToBottom`；Rust 测试断言 `enqueue_message` 返回时 subscriber 已能借到 pending 行。**不是**因为 HEAD composer 等 publish：`kim_composer.dart:105-107` 在 `onSend` 之后立刻 `clear()`，且 `_enqueueText` 造出的 `KimChatMsg` 没有任何生产路径插入列表（`receiveAll` 零调用）。
5. **CommitEffect 用 resolved dest，且仅 `needs_publish`。** 否决按输入 `talk.dest` 发布（HEAD `persist_talks_for:641-650`，空 dest 会 publish `""`）。否决把 `apply_talk = Some` 当成去重：HEAD `apply_talk`（`messages.rs:327-328`）只对空 dest/body 返回 `None`；重复行仍 `Some(ApplyOutcome { inserted: false, unread_delta: 0, .. })` 并 `put_msg` + bump version（`persist_talks_tx:1291-1309`）。真正的 no-op 谓词见下文 `ApplyOutcome.needs_publish`。pending 本端行被 merge 成 sent **必须**刷新。
6. **Snapshot-first；watch 上不发必须按序应用的 Delta。** `tokio::watch` 只保留最新值（[Tokio watch](https://docs.rs/tokio/latest/tokio/sync/watch/index.html)）。HEAD `TimelineUpdate::Delta` 保留类型，本切片生产路径继续只 `send Snapshot`。测量后再开 Delta 切片。
7. **dest 键的 map + `{account, epoch}` 盖戳；过期 refresh 跳过。** 不是复合键，也**不**在 bump_epoch 时 drain/drop `watch::Sender`。HEAD 已是 dest 键且跨 epoch 存活（`lib.rs:67`、`subscribe_timeline:959-963`）。泄漏风险是 refresh 不核对 account/epoch，不是「键不够」。drop sender 会让 FFI `watch_timeline`（`client.rs:330-332`）在 `changed()` 出错时退出，而 Dart 只在 `authProvider.account` 变化时重订（`messages.dart:52-66`）。同一 dest 同时只服务一个 session，不需要双账号槽。
8. **`ChangedQuery` 是枚举，不是字符串 / trait 对象。** 非法组合（无 dest 的 Timeline、跨账户 Inbox）在类型上不存在。不为单一实现发明 `Publisher` trait。
9. **通讯录不进 `SessionSnapshot`。** 沿用 09 D4。Phase 2 用独立 `watch_contacts`。`ContactsChanged` 在 Phase 2 降为可选离散信号或删除 Dart 消费；列表不走返回值。
10. **Phase 1 不改 FFI。** 09 已完成 tagged union 与 Flutter-as-shell 主路径。本切片 Phase 1 必须 `cargo test -p kim-sdk` 与现有 Dart 测试同绿，才能再破坏通讯录/分页契约。
11. **`mark_read` 本地成功即发布；网络上报后台化（落地 PR 3）。** 否决继续 await `proto.mark_read` 再 publish（HEAD `:303-309`）：离线/超时会拖住未读清零。已拍板：`protocol.mark_read` fire-and-forget + epoch 校验；本切片不加可靠已读 outbox。PR 2 仍保持 HEAD「本地 commit 后 await 网络再返回」的时序，以免把行为变化混进 publisher 切换。
12. **热窗口一致读。** `load_hot_window` 今日 6 条独立 SELECT，可能跨两个写事务拼快照。Phase 1 用同一连接 `BEGIN`/`COMMIT` 读。否决为此加 schema。

### Concrete types / interfaces

`crates/kim-sdk/src/store/changes.rs`（新建，`pub(crate)`，不上 FFI）：

```rust
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tokio::sync::Notify;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ChangedQuery {
    Timeline { dest: String },
    Inbox,
    Contacts,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CommitEffect {
    pub queries: BTreeSet<ChangedQuery>,
}

impl CommitEffect {
    pub fn empty() -> Self {
        Self { queries: BTreeSet::new() }
    }

    pub fn timeline(dest: impl Into<String>) -> Self {
        let mut e = Self::empty();
        e.queries.insert(ChangedQuery::Timeline { dest: dest.into() });
        e
    }

    pub fn inbox() -> Self {
        let mut e = Self::empty();
        e.queries.insert(ChangedQuery::Inbox);
        e
    }

    pub fn contacts() -> Self {
        let mut e = Self::empty();
        e.queries.insert(ChangedQuery::Contacts);
        e
    }

    pub fn merge(&mut self, other: CommitEffect) {
        self.queries.extend(other.queries);
    }

    pub fn is_empty(&self) -> bool {
        self.queries.is_empty()
    }
}

/// Process-local notice. `sequence` is NOT a server cursor.
#[derive(Clone, Debug)]
pub(crate) struct CommitNotice {
    pub account: String,
    pub epoch: u64,
    pub sequence: u64,
    pub queries: BTreeSet<ChangedQuery>,
}

pub(crate) struct ChangeLog {
    inner: Mutex<Option<CommitNotice>>,
    notify: Notify,
    sequence: AtomicU64,
    applied: AtomicU64,
    applied_notify: Notify,
}

impl ChangeLog {
    pub fn new() -> Self { /* sequence/applied = 0, inner = None */ }

    /// Call only after COMMIT, never while holding a DB connection across await.
    pub fn record(&self, account: String, epoch: u64, effect: CommitEffect) -> u64 {
        if effect.is_empty() {
            return self.sequence.load(Ordering::SeqCst);
        }
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match g.as_mut() {
            Some(n) if n.epoch == epoch && n.account == account => {
                n.queries.extend(effect.queries);
                n.sequence = seq;
            }
            _ => {
                *g = Some(CommitNotice {
                    account,
                    epoch,
                    sequence: seq,
                    queries: effect.queries,
                });
            }
        }
        drop(g);
        self.notify.notify_one();
        seq
    }

    /// Subscribe **before** checking the slot. Tokio `Notify` does not
    /// store a permit for a waiter that has not yet subscribed.
    pub async fn take(&self) -> CommitNotice {
        loop {
            let notified = self.notify.notified();
            if let Some(n) = self.inner.lock().unwrap_or_else(|e| e.into_inner()).take() {
                return n;
            }
            notified.await;
        }
    }

    pub fn mark_applied(&self, seq: u64) {
        self.applied.store(seq, Ordering::SeqCst);
        self.applied_notify.notify_waiters(); // multiple command waiters
    }

    pub fn applied(&self) -> u64 {
        self.applied.load(Ordering::SeqCst)
    }

    pub async fn wait_applied(&self, seq: u64, timeout: std::time::Duration) {
        let _ = tokio::time::timeout(timeout, async {
            loop {
                let notified = self.applied_notify.notified();
                if self.applied() >= seq {
                    return;
                }
                notified.await;
            }
        })
        .await;
    }
}
```

`write_worker` 在 `reply.send` **之前**（仍不 await 查询）：

```rust
let result = persist_enqueue(&pool, &account, cmd).await;
if let Ok((_, effect)) = &result {
    changes.record(account.clone(), op_epoch, effect.clone());
}
let _ = reply.send(result.map(|(receipt, _)| receipt));
```

各 `*_tx` 返回 `(T, CommitEffect)`：

| 函数 | 成功时 CommitEffect |
|---|---|
| `persist_enqueue` | `Timeline{cmd.dest}` ∪ `Inbox` |
| `persist_talks_tx` | `apply_talk` 内：先 `delete_key(loser)`（若有），再决定是否 `put_msg`。`!needs_publish` 当且仅当无 loser 且 `merged == prev`：不 `put_msg`，仍 `Some`。txn 循环 `if !out.needs_publish() { continue }`。双 key → `had_loser` → 必 publish。空 dest/body 仍 `None` |
| `persist_inbox_tx` | `Inbox` |
| `cancel_tx` | 若仍能读到 dest：`Timeline{dest}` ∪ `Inbox`；否则 empty |
| `delete_thread_tx` | `Timeline{dest}` ∪ `Inbox` |
| `mark_sent_tx` / `mark_failed_tx` / `mark_retry_tx` / `requeue_tx` | 需在事务内读出 dest：`Timeline{dest}`；sent 再 ∪ `Inbox` |
| `mark_read_tx` | `Timeline{dest}` ∪ `Inbox` |
| `due_now_tx` | empty（只拨 `next_attempt_at`） |
| `replace_contacts_tx` | `Contacts`；**补 epoch 校验** |
| settings / agent / media | empty |

`ReplaceContacts` 改为：

```rust
ReplaceContacts {
    epoch: u64,
    account: String,
    rows: Vec<PersonRef>,
    reply: oneshot::Sender<Result<(), SdkError>>,
}
```

与其它写相同：`op_epoch != current` → `StaleEpoch`，不 record。

`ApplyOutcome` 增补（`store/messages.rs`，与现有 `inserted` / `unread_delta` 并列）：

```rust
pub(crate) struct ApplyOutcome {
    pub dest: String,          // resolved: talk.dest else talk.sender
    pub inserted: bool,
    pub unread_delta: i32,
    pub msg: StoredMsg,
    pub fields_changed: bool,  // merged vs cloned prev (full StoredMsg)
    pub had_loser: bool,       // by_mid and by_key were different keys
}

impl ApplyOutcome {
    /// Skip put_msg / apply_incoming / bump / prune only when
    /// !inserted && unread_delta==0 && !fields_changed && !had_loser.
    /// Dual-key collapse is always a publish (identity changed).
    /// Pending own-send merged into sent is fields_changed (status/message_id).
    pub fn needs_publish(&self) -> bool {
        self.inserted || self.unread_delta != 0 || self.fields_changed || self.had_loser
    }
}
```

**闸门在 `apply_talk` 内。** HEAD（`messages.rs:368-393`）在 `put_msg` **之前** 删 `loser`（`by_mid` 与 `by_key` 不同 key）。禁止在 loser cleanup 之前 `return`。HEAD 把 `survivor` **move** 进 `merge_stored`（`:379-382`），比较前必须 `clone`。

```rust
// after (survivor, loser) match — same as HEAD :368-377
let inserted = survivor.is_none();
let prev = survivor.clone(); // clone BEFORE merge_stored
let merged = match survivor {
    None => incoming,
    Some(prev_owned) => merge_stored(prev_owned, incoming),
};
let had_loser = loser.as_ref().is_some_and(|l| l.key != merged.key);
if let Some(loser) = loser {
    if loser.key != merged.key {
        delete_key(tx, account, &dest, &loser.key).await?;
    }
}
let unread_delta = /* same policy as HEAD :394-401; does not need put_msg */;
let fields_changed = match &prev {
    None => true,
    Some(p) => merged != *p, // PartialEq on full StoredMsg, incl. sys/batch_id/thread_kind
};
let needs_publish = inserted || unread_delta != 0 || fields_changed || had_loser;
if !needs_publish {
    // no loser AND merged == prev: skip put_msg only
    return Ok(Some(ApplyOutcome {
        dest, inserted, unread_delta, msg: merged,
        fields_changed: false, had_loser: false,
    }));
}
put_msg(tx, account, &merged).await?;
Ok(Some(ApplyOutcome { dest, inserted, unread_delta, msg: merged, fields_changed, had_loser }))
```

双 key（`had_loser`）**一律** `delete_key(loser)`，并 `needs_publish=true`（折叠身份必须刷新）。**只有** `loser is None && merged == prev` 才跳过 `put_msg`。HEAD `:388-392` 的「surv.key != merged.key」删行仍走写路径（此时 `had_loser` 或 `fields_changed` 已为 true）。

`persist_talks_tx`：

```rust
if let Some(out) = messages::apply_talk(...).await? {
    if !out.needs_publish() {
        continue; // no apply_incoming, no dests.push
    }
    threads::apply_incoming(...).await?;
    dests.push(out.dest);
}
// bump + prune only dests that were pushed
```

`StoredMsg` 加 `PartialEq`（HEAD `messages.rs:290` 目前只有 `Clone, Debug`）。比较 **全部字段**，含 `sys` / `batch_id` / `thread_kind`。Keep 只补齐 `thread_kind` 也必须 `put_msg` + refresh。

`crates/kim-sdk/src/query.rs`（新建）：

```rust
pub(crate) struct TimelineSub {
    pub account: String,
    pub epoch: u64,
    pub dest: String,
    pub limit: i32,
    pub tx: watch::Sender<TimelineUpdate>,
    // Phase 3:
    // pub older_bound: Option<(i64 /* at */, String /* key */, i64 /* before_id */)>,
    // pub loading_older: bool,
    // pub history_error: Option<String>,
}

/// Lives with Store, NOT `Inner.cancel`.
/// `bump_epoch` (`session.rs:15-25`) cancels `Inner.cancel`; `start_session:161`
/// would kill a publisher spawned with `child_token()`. Outbox avoids this by
/// respawning in `start_session`; QueryPublisher instead uses `store_life`.
pub(crate) fn spawn_query_publisher(
    sdk: KimSdk,
    changes: Arc<ChangeLog>,
    store_life: CancellationToken,
) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = store_life.cancelled() => break,
                notice = changes.take() => {
                    let current = sdk.current_epoch().0;
                    // Stale epoch: still mark_applied so wait_applied unblocks.
                    // Do not refresh using the old account's dest list onto new subs
                    // without checking each sub's stamp (refresh_timeline does that).
                    for q in notice.queries {
                        if store_life.is_cancelled() { break; }
                        match q {
                            ChangedQuery::Inbox => sdk.refresh_session_snapshot().await,
                            ChangedQuery::Timeline { dest } => {
                                sdk.refresh_timeline(&dest, notice.epoch, &notice.account).await;
                            }
                            ChangedQuery::Contacts => {
                                sdk.refresh_contacts_view(notice.epoch).await;
                            }
                        }
                    }
                    changes.mark_applied(notice.sequence);
                    let _ = current;
                }
            }
        }
    });
}
```

刷新规则（写进 `KimSdk` 私有方法，从今日 `publish_*` 搬迁）：

```rust
async fn refresh_timeline(&self, dest: &str, notice_epoch: u64, notice_account: &str) {
    let Some(sub) = lock(&self.inner.timelines).get(dest).cloned() else {
        return; // no subscriber: skip. ACK/command already committed.
    };
    // Stamp check: skip if this dest's watcher is for another account/epoch.
    if sub.epoch != notice_epoch || sub.account != notice_account {
        return;
    }
    let Ok(store) = self.store() else { return };
    let Ok(snap) = store.load_hot_window(&sub.account, dest, sub.limit).await else {
        // do not send empty-as-success; leave last snapshot
        return;
    };
    // Re-read stamp; subscribe_timeline may have restamped in place.
    let still = lock(&self.inner.timelines).get(dest).cloned();
    let Some(sub) = still else { return };
    if sub.epoch != notice_epoch || sub.account != notice_account {
        return;
    }
    let _ = sub.tx.send(TimelineUpdate::Snapshot { snapshot: snap });
}

async fn refresh_session_snapshot(&self) { /* 今日 publish_session_snapshot 体，保留 */ }

async fn refresh_contacts_view(&self, epoch: u64) {
    // Phase 1: 无 contacts watch。仍由 replace_contacts emit ContactsChanged。
    // Phase 2: 有订阅则 load_contacts + send ContactsSnapshot。
    let _ = epoch;
}
```

`Inner` 增补（`lib.rs`）：

```rust
struct Inner {
    // existing fields...
    timelines: Mutex<HashMap<String, TimelineSub>>, // dest-keyed; was Sender only
    contacts_watch: watch::Sender<ContactsSnapshot>, // Phase 2; Phase 1 可占位
    changes: Mutex<Option<Arc<ChangeLog>>>,
    /// Cancelled only if the store is ever torn down. NEVER replaced by bump_epoch.
    store_life: CancellationToken,
}
```

`protocol_only` 创建 `store_life: CancellationToken::new()`。`bump_epoch` **只**替换 `Inner.cancel`（现有 `session.rs:15-25`），不动 `store_life`。

`attach_store` 在 `Store::open` 成功后 spawn **一次**：

```rust
let changes = store.changes(); // Arc<ChangeLog>
*lock(&self.inner.changes) = Some(changes.clone());
spawn_query_publisher(self.clone(), changes, self.inner.store_life.clone());
```

禁止用 `self.child_token()`。禁止在 `start_session` 里依赖「publisher 已被杀再 respawn」才能工作；`start_session` 之后 publisher 必须仍活着。测试：`attach_store` → `start_session` → `enqueue_message` → timeline subscriber 在 **远小于 2s** 内看到 pending（失败形态是每次 enqueue 都 `wait_applied` 超时）。

HEAD `switch_account`（`lib.rs:201-214`）**就是** `start_session`（换 `account`/`token`）。没有独立 switch 函数体。restamp vs 持 stamp **必须在 `start_session` 里分支**，且 **在 `replace_session` 之前**读出旧账号：

```rust
pub async fn start_session(&self, s: StartSession) -> Result<(), SdkError> {
    let prev_account = lock(&self.inner.session).as_ref().map(|p| p.account.clone());
    let _ = self.bump_epoch();
    // stop_supervisor, outbox_kick = None (existing)
    self.replace_session(s.clone());
    let epoch = self.current_epoch().0;
    let same_account = match prev_account.as_deref() {
        None | Some("") => true,
        Some(prev) => prev == s.account,
    };
    {
        let mut map = lock(&self.inner.timelines);
        if same_account {
            for sub in map.values_mut() {
                if sub.account.is_empty() || sub.account == s.account {
                    sub.account = s.account.clone();
                    sub.epoch = epoch;
                }
            }
        } else {
            for sub in map.values() {
                let _ = sub.tx.send(TimelineUpdate::Resync {
                    dest: sub.dest.clone(),
                    reason: "account".into(),
                });
                // leave stamp: refresh_timeline skips until Dart rebuilds subscribe
            }
        }
    }
    if same_account {
        if let Some(ch) = lock(&self.inner.changes).clone() {
            let mut effect = CommitEffect::inbox();
            for dest in lock(&self.inner.timelines).keys() {
                effect.merge(CommitEffect::timeline(dest.clone()));
            }
            ch.record(s.account.clone(), epoch, effect);
        }
    }
    // existing: supervisor, persist hook, spawn_session_bridge, spawn_outbox_worker...
}
```

- **同账号或首次（prev 空）：** 原地 restamp `epoch`（及空 account → 新 account）。然后 `record(Inbox ∪ 每个 dest 的 Timeline)`，让 QueryPublisher 用 **新 epoch** 从 DB 重建。否则 bump 前 COMMIT 的 notice.epoch 对不上新 stamp，`refresh_timeline` skip，watch 停在重连前快照。
- **不同账号：** 原地 Resync `"account"`，**不** restamp。Dart `authProvider.account` 变化才 `build()` → `subscribe_timeline` 盖戳 + `record`。禁止在 `start_session` 无条件 restamp，否则新账号 Snapshot 会打进仍开着的旧 dest FFI 流。
- `stop_session`：不 drop sender；无 session 时 stamp 对不上则 skip。

测试（PR 2）：

| 测试 | 不变量 |
|---|---|
| `reconnect_same_account_rebuilds` | persist 一条 → 立刻 `start_session` 同账号 → subscriber **无需再写** 就看到该 body |
| `switch_account_does_not_paint_new_body` | 账号 A 订 dest=bob → `start_session` 账号 B → 该 receiver 在 Dart 重订前 **不能** 出现 B 的消息；可有 Resync `"account"` |

`subscribe_timeline`（复用 dest 键上的 sender，与 HEAD `entry.or_insert_with` 同构）：

```rust
pub fn subscribe_timeline(&self, query: TimelineQuery) -> watch::Receiver<TimelineUpdate> {
    let dest = query.dest.clone();
    let limit = if query.limit <= 0 { 50 } else { query.limit.min(50) };
    let account = self.session_snapshot().map(|s| s.account).unwrap_or_default();
    let epoch = self.current_epoch().0;
    let mut map = lock(&self.inner.timelines);
    let rx = if let Some(sub) = map.get_mut(&dest) {
        if sub.tx.is_closed() {
            let init = empty_snapshot(&dest);
            let (tx, rx) = watch::channel(init);
            *sub = TimelineSub {
                account: account.clone(),
                epoch,
                dest: dest.clone(),
                limit,
                tx,
            };
            rx
        } else {
            sub.account = account.clone();
            sub.epoch = epoch;
            sub.limit = limit;
            sub.tx.subscribe()
        }
    } else {
        let init = empty_snapshot(&dest);
        let (tx, rx) = watch::channel(init);
        map.insert(dest.clone(), TimelineSub {
            account: account.clone(),
            epoch,
            dest: dest.clone(),
            limit,
            tx,
        });
        rx
    };
    drop(map);
    if let (Ok(handle), Some(changes)) = (
        tokio::runtime::Handle::try_current(),
        lock(&self.inner.changes).clone(),
    ) {
        handle.spawn(async move {
            changes.record(account, epoch, CommitEffect::timeline(dest));
        });
    }
    rx
}
```

第二个 watcher 拿到的是 **当前** Snapshot，不是空初值（HEAD 行为）。禁止每次 subscribe `insert` 新 channel。

保持「无 Tokio 不 panic」——现有 `subscribe_timeline_without_tokio_runtime_does_not_panic` 继续绿。

命令 vs hook 等待：

```rust
const APPLY_WAIT: Duration = Duration::from_secs(2);

async fn after_command(&self, seq: u64) {
    if let Some(ch) = lock(&self.inner.changes).clone() {
        ch.wait_applied(seq, APPLY_WAIT).await;
    }
}

// enqueue_message
let (receipt, seq) = store.enqueue(...).await?; // enqueue 返回 seq
self.inner.metrics.inc_enqueue();
self.after_command(seq).await;
self.kick_outbox();
Ok(receipt)

// persist_talks_for
store.persist_talks(...).await?; // 内部 record；不等待
self.inner.metrics.inc_persist_talk();
Ok(())
```

`Store::enqueue` 等 API 需把 `ChangeLog::record` 的 seq 传出。更干净：`record` 在 worker 内完成，oneshot 返回 `(T, u64)`。

边界行为（必须按此实现，禁止临场发挥）：

| 事件 | 行为 |
|---|---|
| txn fail | ROLLBACK；不 `record`；命令返回 `SdkError`；不 ACK |
| 重复输入 | 空 dest/body → None。identical Keep 且无 loser → 不 `put_msg`/bump。mid+client_id 双 key → 删 loser、`needs_publish`、一行残留。pending→sent → `fields_changed` |
| 写队列满 | 现有 `try_send` → `SdkError::Busy { queue: "store" }`。不把 PersistHook Busy 变成 ACK |
| dirty 积压 | 合并，不丢。publisher 一次 take 全部 queries |
| 慢 UI | watch latest-wins。publisher 不 wait StreamSink。FFI 循环 `borrow`/`changed` 不变 |
| 无订阅 | Timeline/Contacts 跳过刷新；Inbox 仍更新 `session_snapshot` sender（始终存在） |
| 首次 subscribe | 插入 sub + `record(Timeline{dest})`，从 DB 重建。禁止只靠空初值 |
| 切账号 / bump epoch | 全在 `start_session`（HEAD `switch_account` 只转调它）。`replace_session` **之前**读 prev.account。同或空：restamp + `record(Inbox∪每个 dest Timeline)`。不同：Resync `"account"`、不 restamp。publisher 继续跑 |
| `load_hot_window` 失败 | 保留上一次 Snapshot，不发空列表冒充「无消息」 |
| 磁盘满 | `SdkError::StorageFull` → PersistHook `StorageFull` → 不 ACK |

Phase 2 类型：

```rust
#[derive(Clone, Debug)]
pub struct ContactsSnapshot {
    pub version: u64,
    pub contacts: Vec<PersonRef>,
    pub sync_error: Option<String>,
}

impl KimSdk {
    pub fn subscribe_contacts(&self) -> watch::Receiver<ContactsSnapshot> { /* ... */ }

    /// Command completion only. UI reads subscribe_contacts.
    pub async fn refresh_contacts(&self) -> Result<(), SdkError> { /* protocol + replace */ }
}
```

Phase 2 FFI：

```rust
impl KimUiHandle {
    #[flutter_rust_bridge::frb(sync)]
    pub fn watch_contacts(&self, sink: StreamSink<ContactsSnapshotDto>) -> Result<(), String> { /* watch loop */ }

    pub async fn refresh_contacts(&self) -> Result<(), SdkErrorDto> {
        self.inner.refresh_contacts().await.map_err(SdkErrorDto::from)
    }
}
```

Phase 3 窗口（在 `TimelineSub` / `TimelineSnapshot` 上扩，不加第二套订阅）：

```rust
// timeline.rs — additive fields, default false/None so Phase 1 DTO 仍可编
pub struct TimelineSnapshot {
    pub dest: String,
    pub version: u64,
    pub messages: Vec<MessageView>,
    pub pending: Vec<MessageView>,
    pub unread: i32,
    pub last_read_message_id: i64,
    pub has_more: bool,
    pub loading_older: bool,           // Phase 3
    pub history_error: Option<String>, // Phase 3；磁盘/网络失败，不是「没有更多」
}

impl KimSdk {
    pub async fn load_older(&self, dest: String) -> Result<(), SdkError> {
        // See Phase 3 unique policy: load_page; SQLite has_more → stop;
        // sent≥400 → no history; else short page + before_id≠0 → history+Keep.
    }
}
```

一致读（Phase 1，`messages.rs`）：

```rust
pub(crate) async fn load_hot_window(...) -> Result<TimelineSnapshot, SdkError> {
    let mut conn = pool.acquire().await.map_err(map_sqlx)?;
    sqlx::query("BEGIN").execute(&mut *conn).await.map_err(map_sqlx)?;
    let result = load_hot_window_on(&mut conn, account, dest, limit).await;
    let _ = sqlx::query("COMMIT").execute(&mut *conn).await;
    result
}
```

把今日 6 条 SELECT 原样搬进 `load_hot_window_on`。失败 ROLLBACK 读事务（SQLite 读事务 rollback 无数据损失）。

### Usage examples

Flutter 聊天页（Phase 3 目标；Phase 1 仍可走现有 `watchThread` + `enqueueMessage`）：

```dart
// 列表只听 watch
ref.listen(threadMessagesProvider(dest), (_, next) { /* render next.items */ });

Future<void> onSend(String text) async {
  final receipt = await ref.read(clientPortProvider).enqueueMessage(
    dest: dest,
    kind: ThreadKind.user,
    content: KimOutgoingContent.text(text.trim()),
    clientId: const Uuid().v4(),
  );
  if (receipt.sendStatus == 'pending') {
    controller.clear(); // Phase 3：从 composer 立刻 clear 挪到此处（Breaking Change 3.5）
  }
}

Future<void> onLoadOlder() =>
    ref.read(threadMessagesProvider(dest).notifier).loadOlder();
```

Flutter 通讯录（Phase 2）：

```dart
class ContactsNotifier extends Notifier<ContactsState> {
  @override
  ContactsState build() {
    _sub = ref.read(clientPortProvider).watchContacts().listen((snap) {
      if (!ref.mounted) return;
      state = mapSnapshot(snap); // 含 sync_error
    });
    return ContactsState.empty(); // 首帧本地 snapshot 会立刻到来
  }

  Future<void> refresh() async {
    state = state.copyWith(loading: true);
    try {
      await ref.read(clientPortProvider).refreshContacts(); // void
    } finally {
      if (ref.mounted) state = state.copyWith(loading: false);
    }
  }
}
```

SDK 侧命令（Phase 1 起）：

```rust
let receipt = sdk.enqueue_message(SendMessageCommand {
    dest: "bob".into(),
    kind: 0,
    payload: OutgoingPayload::Text { body: "hi".into() },
    client_id: Some(id),
    batch_id: None,
}).await?;
assert_eq!(receipt.send_status, SendStatus::Pending);
// subscribe_timeline 已能借到含 pending 的 Snapshot
```

### Data model / schema

**无迁移。** `SCHEMA_VERSION` 保持 4。

沿用表：`messages`、`outbox`、`threads`、`read_watermarks`、`timeline_meta`、`contacts`。`sequence` / dirty / 订阅窗口是内存，进程重启从 DB 重建。

Outbox 与历史 prune 约束不变：`messages::prune`（`messages.rs:67`）已排除未完成 outbox。清历史缓存不得删发送任务。Phase 3：满 400 不 history（避免 persist-then-delete）；未满 400 的短页仍 history+Keep，行落在 newest-400 内。

`SendStatus` 继续用现有枚举（`command.rs:37`）。非法组合不引入平行 `bool failed` 新语义；Dart `KimChatMsg.failed` 仍由 `sendStatus` 映射（`kim_session.dart:50`）。

Phase 3 窗口边界存在 `TimelineSub`，不进 SQLite。重启后热窗口仍是最新 50 + 全部 pending；更早页需再次 `loadOlder`。

### Per-flow sequences

#### receive（live）

```text
WGateway  supervisor  PersistHook  write_worker  ChangeLog  QueryPub  Dart
    │           │          │            │            │          │       │
    │ talk      │          │            │            │          │       │
    │══════════►│ try_send │            │            │          │       │
    │           │═════════►│ persist    │            │          │       │
    │           │          │───────────►│ BEGIN      │          │       │
    │           │          │            │ apply_talk │          │       │
    │           │          │            │ COMMIT     │          │       │
    │           │          │            │─record────►│          │       │
    │           │          │◄─ Ok ──────│            │          │       │
    │           │ ACK      │            │            │          │       │
    │◄──────────│          │            │            │          │       │
    │           │          │            │            │══take═══►│       │
    │           │          │            │            │          │ load  │
    │           │          │            │            │          │══════►│ Snapshot
```

失败：`apply`/`COMMIT` ✗ → PersistError → **不 ACK**。满队列：live `try_send` 失败 → 不 persist、不 ACK、读循环不停（现有）。

#### send

```text
Dart        KimSdk         write_worker    ChangeLog   QueryPub   pump
 │ enqueue   │                │               │           │        │
 │──────────►│ Enqueue        │               │           │        │
 │           │───────────────►│ msg+outbox    │           │        │
 │           │                │ COMMIT        │           │        │
 │           │                │─record───────►│           │        │
 │           │ wait_applied   │               │══refresh═►│        │
 │           │◄───────────────│               │           │        │
 │ receipt   │ kick_outbox    │               │           │        │
 │◄──────────│───────────────────────────────────────────────────►│
 │           │                │               │           │   send │
 │           │                │ mark_sent     │           │◄───────│
 │           │                │─record───────►│           │        │
 │ watch sent│                │               │══refresh═►│        │
```

`kick_outbox` 不是查询发布，仍留在 `enqueue_message`。

#### load older（Phase 1 兼容 / Phase 3 目标差量）

Phase 1：FFI 仍返回 page；ChangeLog 刷新热窗口；Dart `_older` 暂留。

Phase 3：`loadOlder` 无返回值；`TimelineSnapshot.messages` 含窗口；`has_more=false` 与 `history_error=Some` 分家。

#### contacts（Phase 2）

```text
Dart                 KimSdk                    Store         QueryPub
 │ watchContacts      │                         │              │
 │───────────────────►│ subscribe + record      │              │
 │ local snapshot     │                         │              │
 │◄══════════════════════════════════════════════refresh───────│
 │ refreshContacts()  │ friend_list+incoming    │              │
 │───────────────────►│ replace_contacts        │              │
 │                    │────────────────────────►│ COMMIT       │
 │                    │                         │ record       │
 │                    │ wait_applied            │              │
 │ void Ok            │                         │              │
 │◄───────────────────│                         │              │
 │ new snapshot       │                         │              │
 │◄══════════════════════════════════════════════refresh───────│
```

#### mark_read

```text
Dart ──► markRead ──► mark_read_local (unread=0, watermark MAX)
                         COMMIT ──► ChangeLog Timeline+Inbox
                         wait_applied ──► return Ok
                         ┄► spawn protocol.mark_read (epoch-checked)
                              ✗ 网络失败：本地已清未读；不回滚
                              （已拍板：fire-and-forget；下次打开会话再 mark_read）
```

---

## Key Decisions

| # | 选择 | 否决 | 为什么 |
|---|---|---|---|
| D1 | 写 worker commit+dirty；独立 publisher 刷新 | 调用方手工 publish；worker 内查询 | 漏刷新 vs 拖 ACK |
| D2 | 可合并 dirty + Notify | 有界 try_send 丢通知 | 最后一次提交必须可见 |
| D3 | 进程内 sequence | sequence 落库 / 当 sync cursor | 可重建；无迁移 |
| D4 | 命令 wait_applied；hook/pump 不等 | 全等待或全不等 | scroll/测试看见 pending；非 HEAD composer |
| D5 | resolved dest + `needs_publish` | `apply_talk=Some` 当去重；按输入 dest 发布 | HEAD 重复 Keep 仍 bump version |
| D6 | Snapshot on watch | Delta on watch | watch 丢中间值 |
| D7 | dest 键 map + account/epoch 戳 | 复合键；drain/drop sender | FFI 流会死；HEAD 已 dest 键 |
| D8 | ChangedQuery 枚举，无 Publisher trait | 字符串标签 / trait 抽象 | 单一实现 |
| D9 | contacts 独立 watch | 塞进 SessionSnapshot | 09 D4 |
| D10 | Phase 1 FFI 签名不变（行为见 Breaking Change） | 一 PR 改完通讯录+分页 | 每步可编译 |
| D11 | mark_read 本地先发布（PR 3） | 等网络再 publish | 离线未读；不混进 PR 2 |
| D12 | 热窗口单读事务 | 6 条独立 SELECT | 一致快照 |

---

## Phased Implementation

每 phase 结束工作区可编译。禁止 phase 内留下「只改了一半签名」的 FFI。

### Phase 1: 统一提交后查询发布

**目标：** 所有业务写入经 ChangeLog → QueryPublisher。FFI/Dart 签名不变。`cargo test -p kim-sdk` 与 `sdk/mobile/test/state/*` 保持绿。

**File: `crates/kim-sdk/src/store/changes.rs`（新增）**

- 加入上文 `ChangedQuery` / `CommitEffect` / `CommitNotice` / `ChangeLog`。
- 单测（可放 `changes.rs` 的 `#[cfg(test)]` 或 `tests/query_publish.rs`）：合并、epoch 替换清空旧 queries、empty 不 bump sequence、**subscribe-before-check**：在 `take` 已进入 wait 前 `record` 不得丢 wakeup。

**File: `crates/kim-sdk/src/store/mod.rs`**

- `Store` 持有 `Arc<ChangeLog>`；`open` 创建并 `clone` 进 `write_worker`。
- 每个 `*_tx` 返回 `(T, CommitEffect)`；worker 在 Ok 时 `record`。
- `ReplaceContacts` 加 `epoch`，与其它写一样校验。
- `mark_sent_tx` 等在事务内 `SELECT dest FROM outbox/messages WHERE client_id=?`，以便 CommitEffect 带 dest。
- `Store::enqueue` 等对外 async 方法返回值增加 seq，或返回 `(T, u64)` 仅 crate 内使用。
- **不**在 worker 里 await load。

**File: `crates/kim-sdk/src/store/messages.rs`**

- `load_hot_window` 改为单连接读事务。查询 SQL 不变。

**File: `crates/kim-sdk/src/query.rs`（新增）**

- `TimelineSub`、`spawn_query_publisher`。
- 不导出 FFI 类型。

**File: `crates/kim-sdk/src/lib.rs`**

- `mod query;`
- `Inner.timelines` 改为 `HashMap<String, TimelineSub>`；`Inner.changes`；`Inner.store_life`（`bump_epoch` 不动它）。
- `attach_store` spawn publisher：`spawn_query_publisher(self.clone(), changes, self.inner.store_life.clone())`。`store_life` 是裸 `CancellationToken`，**不要** `lock()`。
- `start_session`：`replace_session` 前读 prev.account。同/空 → restamp + `record` 每个 dest+Inbox。不同账号 → Resync `"account"`，不 restamp。不 drain。`switch_account` 无独立逻辑。
- `subscribe_timeline` 复用 dest sender，盖戳 + `record`；sender closed 才换 channel。
- `enqueue_message` / `cancel_send` / `retry_send` / `delete_thread` / `load_older` / `replace_contacts`：删除 `publish_*`；命令路径 `after_command(seq)`。`mark_read` 的网络时序留到 PR 3。
- `delete_thread`：**命令路径**（非 dirty 枚举）在 store 删除成功后，对现有 dest sender `send(TimelineUpdate::Resync { dest, reason: "deleted" })`，再 `record(Timeline{dest} ∪ Inbox)` 并 `wait_applied`。QueryPublisher 随后发空/热 Snapshot。Phase 1 Dart `messages.dart:88-91` 靠 Resync 清 `_older`；只发 Snapshot 会让 `_onSnapshot` 把 `_older` 拼回来。
- `persist_talks_for` / `persist_inbox_for`：删除 `publish_*`；**不等待**。保留 `emit_session_wait(Inbox)` 与 `ContactsChanged`（Dart 忽略 Inbox；Phase 1 通讯录测试仍听 mpsc）。
- `load_older`：persist/history 失败仍返回已有本地页（保持 HEAD `let _ =` 对 Dart 的可见语义），`warn` 日志；**不**把错误变成 FFI `SdkErrorDto`。成功 persist 后 `after_command` 再读页。
- `spawn_session_bridge` 的 Link / Lagged 继续直接 `refresh_session_snapshot`（非 store 写）。
- 删除业务入口对 `publish_timeline` / `publish_session_snapshot` 的调用；两函数改为 `pub(crate)` refresh。`publish_timeline_resync` **保留**给 `delete_thread`。

**File: `crates/kim-sdk/src/outbox/pump.rs`**

- 删除三处 `sdk.publish_timeline`。`mark_*` 的 CommitEffect 负责。

**File: `crates/kim-sdk/src/metrics.rs`**

- 新增 `query_refresh_total`、`query_stale_skip_total`、`query_apply_wait_timeout_total`（`AtomicU64`）。`snapshot()` 元组可保持原 4 元以免 FFI 破坏；新计数先 `tracing::debug` / 独立 getter。Phase 4 再进 `MetricsDto`。

**测试（Phase 1 必须新增/扩展）**

| 测试 | 文件 | 不变量 |
|---|---|---|
| `commit_failure_does_not_publish` | `tests/query_publish.rs`（新） | persist 失败后 timeline 仍无该 body |
| `duplicate_keep_is_noop` | 同上 | (a) identical Keep、单 key：version 不变。(b) **mid + client_id 双 key** fixture：结束后该 dest **一行**（loser 已删），且 `needs_publish`（折叠身份） |
| `pending_merge_still_publishes` | 同上 | 本地 pending 被 live/offline 合成 sent：必须刷新，snapshot 中该 key 为 sent + message_id |
| `changelog_no_lost_wakeup` | `changes.rs` 单测 | `take` 在空槽上等待时插入 `record`，必须返回该 notice（subscribe-before-check） |
| `publisher_survives_start_session` | `tests/query_publish.rs` | `attach_store` → `start_session` → `enqueue_message` → subscriber 在 200ms 内见到 pending（不得靠 2s `wait_applied` 超时） |
| `reconnect_same_account_rebuilds` | 同上 | persist → 立刻同账号 `start_session` → 无需再写，subscriber 见到该 body |
| `switch_via_start_session_holds_stamp` | 同上 | 账号 A 订阅 dest → `start_session` 账号 B → 该 receiver 在重订前没有 B 的 body |
| `delete_thread_resync_clears_older` | 同上或 `delete_thread.rs` | 先 load_older 进 `_older` 语义（SDK 侧至少：Resync 后再 Snapshot 不含已删 key）；watch 先 Resync 再空/热 Snapshot |
| `coalesced_refresh` | 同上 | N 条 enqueue 同 dest，publisher 刷新次数 < N（允许 1..=N，断言远小于 N+inbox） |
| `slow_subscriber_converges` | 同上 | 订阅者停 100ms 后再 `changed`，最终 snapshot 含最后一条 |
| `stale_epoch_skipped` | 同上 | `switch_account` 后旧 stamp 的 dest **不**出现新账号 body；FFI sender 仍存活（`changed()` 不成 Err） |
| `first_subscribe_rebuilds` | 扩展 `timeline_watch.rs` | persist 后再 subscribe，首个非空 Snapshot 含消息（现有 `persist_talk_reaches_timeline_subscriber_with_message_view` 仍绿） |
| `persist_hook_does_not_wait_query` | 扩展 `persist_ack.rs` | hook 返回时 publisher 可被故意阻塞；ACK 边界仍由 kim-client 测试覆盖 |
| `replace_contacts_stale_epoch` | 扩展 `contacts_settings.rs` | bump epoch 后 in-flight replace → `StaleEpoch`，旧 contacts 不变 |
| `mark_read_returns_without_protocol` | 新或 `invariants.rs` | **PR 3：** 无 protocol 时本地 unread=0 且 snapshot 已更新 |
| `load_hot_window_consistent_read` | `tests/query_publish.rs` | 并发 persist 时单次 snapshot 的 unread 与 messages 同事务 |

保持绿：`timeline_watch.rs`（3）、`persist_ack.rs`、`store_restart.rs`、`outbox_kill.rs`、`contacts_settings.rs`、`invariants.rs`、`unread_replay.rs`、`session_events.rs`、`delete_thread.rs`。Dart `sdk/mobile/test/state/*` 不应因 Phase 1 失败。

**Phase 1 编译门槛：** `cargo test -p kim-sdk`；`cargo clippy -p kim-sdk --all-targets`；不跑 FRB 生成。

### Phase 2: 收拢通讯录出口

**File: `crates/kim-sdk/src/contacts.rs`（新增）**

```rust
impl KimSdk {
    pub async fn refresh_contacts(&self) -> Result<(), SdkError> {
        let proto = self.protocol()?;
        let epoch = self.current_epoch().0;
        let account = self.session_snapshot()?.account.clone();
        let friends = match proto.friend_list().await {
            Ok(v) => v,
            Err(e) => {
                self.send_contacts_error(&account, epoch, e.to_string()).await;
                return Err(map_client(e, ""));
            }
        };
        let incoming = match proto.friend_incoming().await {
            Ok(v) => v,
            Err(e) => {
                self.send_contacts_error(&account, epoch, e.to_string()).await;
                return Err(map_client(e, ""));
            }
        };
        if self.current_epoch().0 != epoch {
            return Err(SdkError::StaleEpoch { expected: epoch, actual: self.current_epoch().0 });
        }
        self.replace_contacts(map_person_refs(friends, incoming)).await?;
        Ok(())
    }

    // map_client(err, dest) HEAD error.rs:69 需要 dest。好友列表失败不是会话级
    // Status 109/110。此处 dest="" 表示非 thread 错误：NotFriends/Blocked 的 dest
    // 为空字符串，调用方只当 sync_error 文案，不走进「与某人不是好友」UI。
    // 禁止编造假 dest。保持 send_contacts_error 在 return Err 之前。

    async fn send_contacts_error(&self, account: &str, epoch: u64, msg: String) {
        if self.current_epoch().0 != epoch {
            return;
        }
        let rows = match self.store() {
            Ok(s) => s.load_contacts(account).await.unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        let _ = self.inner.contacts_watch.send(ContactsSnapshot {
            version: 0,
            contacts: rows,
            sync_error: Some(msg),
        });
    }

    pub fn subscribe_contacts(&self) -> watch::Receiver<ContactsSnapshot> { /* dest 同构：复用 sender */ }
}
```

`send_contacts_error` 实现时用已有 async `load_contacts`，**禁止** `block_on`。失败路径：`contacts_watch.send` 上一帧 contacts + `sync_error: Some`，再 `return Err`。成功 `replace_contacts` 后 publisher 发送 `sync_error: None`。

**好友/资料事件（关闭 OQ3，09 铁律：Dart 不拥有名册持久化）：**

`spawn_session_bridge` 在 `emit_session_wait`（haptics 仍走 Dart `link.dart`）之外：

| 事件 | SDK |
|---|---|
| `FriendRequest { from, nickname }` | `WriteOp::UpsertContact { peer: from, relation: "incoming", nickname, avatar: "" }` → CommitEffect::Contacts |
| `FriendAccepted { from, nickname }` | `UpsertContact { relation: "friend", nickname }`；删 incoming 行若有 |
| `ProfileUpdated { account, nickname, avatar }` | `UpsertContact` 只覆盖 nickname/avatar，保留 relation/bio/kind |
| 在线时 | 不强制全表 refresh。事件 payload 足以表达成员关系；avatar/bio 缺省等用户点刷新 |

`WriteOp::UpsertContact` 校验 epoch。不在 Dart 里 insert/patch 列表。

**Dart 必须删除的列表写（Phase 2 清单）：** `applyChanged`、`onRequest` 插入、`onAccepted` 插入+`refresh()`、`removePeer` 本地 filter、`onProfileUpdated` patch。保留 `withLocalAgents` 作为展示映射。`removePeer` 只调 `friendRemove`/`botDelete`，等 contacts snapshot。

**File: `crates/kim-sdk/src/store/contacts.rs`**

- 现有 `replace_all` 保留。
- 新增 `upsert_one` / `delete_peer`（`removePeer` 用）。

**File: `crates/kim-sdk/src/lib.rs` / `query.rs`**

- `refresh_contacts_view`：`load_contacts` → `contacts_watch.send`。
- `replace_contacts` 可保留 `ContactsChanged` mpsc **一版**，但 Dart 不再用它改列表。下一清理 PR 再删变体（非本 phase 必须）。

**File: `sdk/mobile/rust/src/api/client.rs`**

- 删除 `refresh_contacts` 的 `block_on` 编排（`:669-701`）。
- 改为 async 转调 `KimSdk::refresh_contacts`。
- 新增 `watch_contacts`。

**File: `sdk/mobile/rust/src/api/types.rs`**

- `ContactsSnapshotDto { version, contacts: Vec<PersonDto>, sync_error: Option<String> }`。

**File: `sdk/mobile/lib/bridge/kim_bridge.dart`、`FakeKim`**

- `refreshContacts() → Future<void>`。
- `watchContacts()`。
- FakeKim：`pushContacts(ContactsSnapshotDto)`；`refreshContacts` 只 `pushContacts` 并 `return;`。

**File: `sdk/mobile/lib/features/contacts/contacts.dart`**

- `build`：订阅 `watchContacts`，离线也展示。
- `refresh`：只设 `loading` 并 `await refreshContacts()`；列表与 `sync_error` 来自 snapshot，catch 不清空 friends。
- **删除：** `applyChanged`、`onRequest` 插入、`onAccepted` 插入、`removePeer` 本地 filter、`onProfileUpdated` patch。
- `removePeer`：只发命令，列表等 snapshot。
- `withLocalAgents` 留下作展示映射（09 允许）。这不是业务持久化。

**测试**

| 测试 | 不变量 |
|---|---|
| `contacts_offline_first`（`contacts_settings.rs`） | 无 protocol 时 `subscribe_contacts` 仍给出上次 `replace_contacts` 的行 |
| `contacts_refresh_error_keeps_cache` | refresh 失败 snapshot.sync_error 有值，contacts 非空 |
| `contacts_test.dart` | 不再断言 `onAccepted` 立刻改 friends；改为 `fake.pushContacts` 后列表更新 |
| `outbox_account_test.dart` | 切账号后 contacts 不是上一账号 |

**Phase 2 编译门槛：** FRB 生成 + `flutter analyze` + 上述测试。`refresh_contacts` 旧签名的调用点必须为零。

### Phase 3: SDK 拥有聊天窗口 + 命令回执

**`load_older` 唯一策略（删除「只读 SQLite」）：**

`MAX_MESSAGES = 400` 是 newest-400 缓存 cap。Keep persist 仍 `prune`，因此 **已满 400 时再 history 会 persist-then-delete**。未满 400 时 HEAD 的 history 回填 **能留下**（新行进 newest-400）。Phase 3 对齐 HEAD 的「短页才拉网」，并堵上满 cap 的浪费路径。

唯一实现：

1. `load_page`（cursor = 窗口最旧 `(at, key)`，`limit` 默认 50）。
2. 若 SQLite `has_more == true`：**停**。扩展 `older_bound`，不打网。
3. 否则数该 dest `status='sent'` 行数：
   - **≥ `MAX_MESSAGES`：** `has_more = false`，**禁止** `protocol.history`。
   - **< 400** 且本页短于 `limit` 且 `before_id != 0`：`protocol.history` + persist **Keep**（这些行在 newest-400 内，prune 留得住）。失败：`history_error = Some`，`has_more` 保持 true（不是尽头）。
4. persist 成功后再 `load_page`，更新 `older_bound`。

测试：

| 测试 | 不变量 |
|---|---|
| `load_older_at_cap_skips_history` | 本地已 400 newest → history RPC 次数 = 0，`has_more=false`，行数仍 400 |
| `load_older_under_cap_fetches_history` | 本地 80 sent、`load_page` 短、`before_id != 0` → **尝试** `protocol.history`；假客户端返回更旧行则 persist 后窗口含它们 |

更深于 newest-400 的历史是独立切片（提高 cap 或窗口内跳过 prune）。

**游标（Dart 不再算）：**

当前窗口 = `TimelineSnapshot.messages ∪ pending`，按 `(at, key)` 升序。

| 字段 | 来源 |
|---|---|
| `before_at` / `before_key` | 窗口最旧一条的 `at` / `key`（与 HEAD Dart `messages.dart:184-191` 的 `oldest` 相同） |
| `before_id` | 窗口中最小的非 0 `message_id`（与 `_historyBeforeId` 相同）。history RPC 需要它；满 400 时不用 |
| 后续 `older_bound` | 上次成功 `load_page` 返回的最旧 `(at, key)` |
| `has_more` | `load_page` 是否多取到第 `limit+1` 行 |

第一次 `load_older`：从当前 snapshot 取 oldest 作为 cursor。无消息：`InvalidArgument` 或 no-op + `has_more=false`。

**`loading_older` 单一写者 = QueryPublisher：**

`load_older` 不直接 `watch::send`。步骤：

1. 无 `TimelineSub` → `InvalidArgument { "no timeline subscriber" }`。
2. `sub.loading_older = true`；`sub.history_error = None`；`changes.record(Timeline{dest})`；`wait_applied`（UI 先看到 loading）。
3. `load_page`。SQLite `has_more` → 停。否则 sent count ≥ 400 → `has_more=false`、不 history。sent &lt; 400 且页短且 `before_id != 0` → `protocol.history` + persist Keep。
4. `sub.older_bound = 新最旧`；`sub.loading_older = false`；磁盘/网络失败 → `history_error = Some`，**不**把 `has_more` 置假。
5. `record(Timeline)` + `wait_applied`。publisher 读热窗口 ∪ `[older_bound, hot)`。

禁止 load_older 与 publisher 双写同一 watch。`delete_thread` 的 Resync 仍是命令路径例外。

**File: `crates/kim-sdk/src/query.rs` / `timeline.rs` / `store/messages.rs` / `lib.rs`**

- `TimelineSub.older_bound: Option<(i64 at, String key)>`、`loading_older: bool`、`history_error: Option<String>`。
- `refresh_timeline`：热窗口 ∪ `[older_bound, hot)` 本地页 → `messages`；`pending` 仍单独。有序、按 key 去重。
- `load_older(dest: String) -> Result<(), SdkError>`：上表步骤。
- 窗口淘汰（热 50）≠ `delete_thread`。Resync `"deleted"` 仍清空（Phase 3 起 Dart 无 `_older`，Resync+空 Snapshot 都可；仍发 Resync 以免过渡期双实现）。

**File: FFI `client.rs` / `types.rs`**

- `load_older(dest)` async `Result<(), SdkErrorDto>`。删除 `before_at/before_key/before_id/limit` 参数（Dart 不再算游标）。
- `TimelineSnapshotDto` 增 `loadingOlder` / `historyError`。

**File: `messages.dart`**

- 删除 `_older`、`receiveAll`、`_onDelta` 的业务合并可留（生产不发 Delta；Resync 仍清列表等 Snapshot）。
- `_onSnapshot`：`items = messages + pending`，以 SDK 为准，不再 ∪ `_older`。
- `loadOlder`：本地 `loadingOlder` 可保留到 snapshot 回传；调用 `client.loadOlder(dest: dest)`。

**File: `chat_session.dart` / `mutations.dart`**

```dart
final sendMessageMutation = Mutation<KimCommandReceipt>(label: 'inbox.send');
final sendImagesMutation = Mutation<List<KimCommandReceipt>>(label: 'inbox.sendImages');

Future<KimCommandReceipt> _enqueueText(...) async {
  return client.enqueueMessage(...); // 不再 return KimChatMsg(...)
}
```

`KimChatMsg` 身份 = snapshot 的 `key`（`client_id`）。输入框清空见 Breaking Change Phase 3.5：从 composer 立刻 `clear()` 改为 `_send` 在 `enqueue` 成功后 clear（HEAD composer 不等待）。

**测试**

| 测试 | 不变量 |
|---|---|
| `window_load_older_then_live` | 分页期间 persist 新消息，最终 snapshot 含新消息 + 旧页，无重复 key |
| `load_older_at_cap_skips_history` | 400 newest → 零 history RPC，`has_more=false` |
| `load_older_under_cap_fetches_history` | 80 local / 短页 / before_id≠0 → 尝试 history |
| `history_error_not_end` | 磁盘 `load_page` 失败：`history_error.is_some()` 且 `has_more` 保持原值（不是 false） |
| `messages` widget/state | `receiveAll` 符号删除；`loadOlder` 不传 cursor |
| `outbox_account_test.dart` | 发送成功不把 `KimChatMsg` 插进 notifier；只出现在 fake timeline push |

**Phase 3 编译门槛：** FRB 生成；删除 `MessagePageDto` 的 Dart 生产调用（测试 Fake 可暂留空实现）。

### Phase 4: 契约验证与成本

不改协议。补齐并跑：

- `crates/kim-sdk/tests/query_publish.rs`（Phase 1 已建，本 phase 加并发与重启）。
- `timeline_watch.rs`：分页 + 新消息；subscribe 无 Resync-only。
- `store_restart.rs`：重启后 outbox pending 仍在，subscribe 重建。
- `outbox_kill.rs`：杀泵恢复。
- `persist_ack.rs` + `kim-client` `persist_hook_acks_without_confirm` / `persist_hook_failure_does_not_ack`：分层断言，不把 SDK 测试名称当成端到端 ACK。
- `contacts_settings.rs`：离线首屏。
- Dart：`contacts_test.dart`、`inbox_test.dart`、`outbox_account_test.dart`。

测量（DevPanel / tracing，无 feature flag）：

- commit 耗时
- commit → 首个 Snapshot send
- 每次写入触发的查询数（coalesce 后）
- FFI Snapshot 字节
- `ThreadMessagesNotifier` rebuild 次数

**仅当** 热路径 Snapshot 字节或 rebuild 成为实测瓶颈，另开 Delta 切片。Delta 不得走 `watch` 的必须按序通道。

---

## Architectural Notes

- 这是内部 API 迁移。Phase 2/3 破坏面同 PR 更新 bridge、FakeKim、生成代码。无仓库外消费者。
- 保持 Riverpod、Rust SQLite、kim-client 协议层。写队列串行、`BEGIN IMMEDIATE`、`WRITE_CAP=128` 不变。
- 不引入 dyn Publisher。`ChangeLog` / `QueryPublisher` 是具体结构。
- `KimSdk::session_snapshot()`（返回 `StartSession`）与 UI `SessionSnapshot` 撞名，本切片不重命名，以免大面积 diff。新代码用 `start_session_config()` 仅在新函数上避免，不强制改旧名。
- `emit_session(Inbox)` 与 snapshot 双发：Phase 1 保留 mpsc Inbox 以免测试/诊断断裂；Dart `link.dart:208` 已忽略。Phase 4 若无 Rust 测试依赖可删 Inbox 变体——非必须。
- 统一入口 ≠ 每次全表扫描。Timeline 只刷新脏 dest；Inbox 只 `load_threads`；Contacts 只 `load_contacts`。
- Flutter 数据层职责在 Rust（[Flutter data layer](https://docs.flutter.dev/app-architecture/case-study/data-layer)，[offline-first](https://docs.flutter.dev/app-architecture/design-patterns/offline-first)）。
- Library 路径：`thiserror`，禁止 `unwrap`（`lock` 的 poisoned mutex `into_inner` 沿用现有 `session::lock`）。
- 跨 FFI 只用 owned 类型。`ChangedQuery` 不上 FFI。

---

## File Change Summary

按 crate 字母序，一行一个文件。

- `crates/kim-client/src/supervisor.rs` — 不改逻辑；Phase 4 用现有 persist-then-ack 测试作边界对照。
- `crates/kim-client/src/sync.rs` — 同上。
- `crates/kim-sdk/src/contacts.rs` — **新增**（Phase 2）：refresh 编排、事件 upsert。
- `crates/kim-sdk/src/lib.rs` — 收拢入口；spawn publisher；命令 wait_applied；hook 不等待。
- `crates/kim-sdk/src/metrics.rs` — 查询刷新/超时计数。
- `crates/kim-sdk/src/outbox/pump.rs` — 删除手工 `publish_timeline`。
- `crates/kim-sdk/src/query.rs` — **新增**：`TimelineSub`、publisher 循环、Phase 3 窗口。
- `crates/kim-sdk/src/store/changes.rs` — **新增**：dirty 类型。
- `crates/kim-sdk/src/store/contacts.rs` — upsert/delete 单行（Phase 2）。
- `crates/kim-sdk/src/store/messages.rs` — `StoredMsg: PartialEq`；clone prev 再 merge；loser 先 `delete_key`；仅无 loser 且 `merged==prev` 才跳过 `put_msg`。
- `crates/kim-sdk/src/store/mod.rs` — CommitEffect；ReplaceContacts epoch；seq 传出。
- `crates/kim-sdk/src/store/outbox.rs` — `mark_sent` / `mark_failed` / `mark_retry` / `requeue` / `cancel` 在事务内返回 dest（0 行 → empty effect）。
- `crates/kim-sdk/src/timeline.rs` — Phase 3 窗口字段；Phase 2 `ContactsSnapshot`。
- `crates/kim-sdk/tests/contacts_settings.rs` — 离线 contacts、epoch。
- `crates/kim-sdk/tests/outbox_kill.rs` — 保持绿；确认泵不再直接 publish。
- `crates/kim-sdk/tests/persist_ack.rs` — hook 不等查询。
- `crates/kim-sdk/tests/query_publish.rs` — **新增** Phase 1 不变量。
- `crates/kim-sdk/tests/store_restart.rs` — 重启后 subscribe 重建。
- `crates/kim-sdk/tests/timeline_watch.rs` — 扩展窗口/并发。
- `sdk/mobile/lib/bridge/kim_bridge.dart` — Phase 2/3 契约。
- `sdk/mobile/lib/design/kim_composer.dart` — Phase 3：提交后不再立刻 clear。
- `sdk/mobile/lib/features/chats/chat_page.dart` — enqueue 成功后再 clear + 现有 scroll。
- `sdk/mobile/lib/features/chats/chat_session.dart` — 命令回执。
- `sdk/mobile/lib/features/chats/messages.dart` — 删除 `_older` / `receiveAll`。
- `sdk/mobile/lib/features/contacts/contacts.dart` — 单一 watch。
- `sdk/mobile/lib/features/session/mutations.dart` — `KimCommandReceipt`。
- `sdk/mobile/rust/src/api/client.rs` — 删 block_on 通讯录；watch_contacts；load_older(dest)。
- `sdk/mobile/rust/src/api/types.rs` — ContactsSnapshot / 窗口字段。
- `sdk/mobile/test/state/contacts_test.dart` — pushContacts。
- `sdk/mobile/test/state/inbox_test.dart` — snapshot 一致性。
- `sdk/mobile/test/state/outbox_account_test.dart` — 发送不插第二份列表。
- `sdk/mobile/test/support/fake_kim.dart` — 新 API。

不改：`services/**`、`crates/kim-tcp`、`crates/kim-protocol`、`frb_generated.*`（生成物）。

---

## Alternatives Considered

1. **继续在每个 `KimSdk` 方法里手工 `publish_*`，只加 checklist。** 成本低、无新模块。否决：HEAD 已漏 `cancel_send` 的 inbox、`persist_talks_for` 按输入 dest 发布、pump 三处复制。新写路径（upsert contact、mark_sent inbox）会再漏。

2. **有界 mpsc 发送 `CommitNotice`，满则丢或反压写 worker。** 实现简单。否决：满则丢违反「最后一次提交必须可见」；反压写 worker 会拖 persist 与 ACK。dirty set 合并是 O(打开 dest) 内存。

3. **把查询刷新放进 `write_worker` 同一任务。** 天然串行、无 epoch 竞态。否决：`load_threads` + 每 dest `load_hot_window` 进写队列，live persist 延迟直接抬高 ACK。读应走 pool 其它连接。

4. **命令也不 wait_applied，完全靠 watch。** PersistHook 友好。否决：`chat_page.dart:328-330` `await sendText` 后 `scrollToBottom` 需要 pending 已在 watch 上；Rust 测试断言 `enqueue_message` 返回 ⇒ subscriber 已有该行。HEAD composer **并不**等 publish（`kim_composer.dart:105-107` 立刻 clear；造出的 `KimChatMsg` 无生产插入）。2s 超时是逃生口。Phase 3 才把 clear 挪到 enqueue 成功之后（Breaking Change 3.5）。

5. **Phase 1 就引入 Delta。** 减 FFI 字节。否决：未测量；`watch` 不能承载必须按序 Delta；09 已要求 Snapshot-first。

6. **Dart 侧做 Repository + 合并适配器（保留 `_older`）。** 否决：违反 09 铁律，第二份事实来源正是本切片要删的。

---

## Security & Privacy

| 威胁 | 缓解 |
|---|---|
| 切账号后旧 dest watch 泄露新账号消息 | dest 键保留 sender；stamp 不匹配则 skip refresh；换账号原地 Resync `"account"`，Dart 重订后盖戳 |
| `ReplaceContacts` 无 epoch 把新会话通讯录写入旧账号 | Phase 1 补 epoch |
| Snapshot 经 FFI 含全部热窗口正文 | 与今日相同；不在日志打印 body。DevPanel 测量字节，不 dump 内容 |
| 查询失败被当成空通讯录/空聊天 | 失败保留上一快照；`history_error`/`sync_error` 分离 |
| ACK 与 UI 绑定导致未落盘却 ACK | PersistHook 不等 publisher；txn fail 不 ACK（现有） |

不新增网络面。不把 JWT/token 放进 Snapshot。contacts 仍是本机账号作用域行。

---

## Observability

现有 `SdkMetrics`：`enqueue_total` / `persist_talk_total` / `epoch_drop_total` / `store_wipe_total`（`metrics.rs:4-10`）。

本切片新增（Rust `tracing` + atomic；Phase 4 再视需要进 `MetricsDto`）：

| 指标 | 触发 |
|---|---|
| `query_refresh_total{query}` | publisher 完成一次 Inbox/Timeline/Contacts 刷新 |
| `query_stale_skip_total` | epoch/account 不匹配丢弃 |
| `query_apply_wait_timeout_total` | `wait_applied` 超时 |
| `query_coalesce_size` | take() 时 queries.len()（histogram via debug log 亦可） |
| `commit_to_snapshot_ms` | record → mark_applied |

日志：

- commit：沿用 `enqueue committed`（`lib.rs:242`）。
- publisher：`debug`(account, epoch, seq, queries)；刷新失败 `warn` + `SdkError`，不 `unwrap`。
- 不在 info 打消息正文。

告警（DevPanel，无运行时 flag）：`apply_wait` 连续超时、`StorageFull`、`epoch_drop` 飙升。

---

## Rollout Plan

本仓库 09 已拍板：**无 feature flag、无双写、无弃用窗口。** 本切片用 **PR 阶段** 做增量，而不是运行时开关。

1. PR 顺序见文末 PR Plan。每 PR 可独立 review / merge / `git revert`。
2. Phase 1 FFI 签名不变：PR 1 无用户可见 diff；PR 2 切 publisher（见 Breaking Change 列表）；PR 3 才让 `mark_read` 离线更快。禁止 PR 2 宣称「完全行为中立」。
3. Phase 2/3 为内部契约破坏：同 PR 改完 Dart/FakeKim/FRB；CI `flutter analyze` + 相关 widget/state 测试。
4. 回滚 = revert 该 PR。禁止留 `KIM_QUERY_PUBLISHER` 之类开关。
5. 不分阶段双发 Snapshot（旧手工 publish + 新 publisher 并存）—— Phase 1b 切换时删除手工调用，测试必须在同一 PR 证明 watch 仍有数据。

---

## Risks

| 风险 | 严重度 | 缓解 |
|---|---|---|
| Phase 1b 删手工 publish 后某路径漏 dirty | 高 | CommitEffect 表作为 review checklist；`query_publish` 覆盖 enqueue/persist/mark_read/pump/cancel/delete/contacts |
| `wait_applied` 死锁（publisher 等写锁、命令等 applied） | 高 | publisher 不持 Store 写通道；只读 pool；ChangeLog Mutex 不跨 await |
| persist 后立刻 ACK、UI 尚未 snapshot | 低 | 规范允许。命令路径仍 wait。慢列表由 watch 追上 |
| `load_hot_window` 读事务与写 IMMEDIATE 互斥抬延迟 | 中 | 读 `BEGIN` 默认 deferred；WAL 下读不挡写。测量 commit_to_snapshot_ms |
| 切账号后 in-flight refresh 打到新 stamp | 高 | `refresh_timeline` 发送前再读 stamp；不匹配不 send。不 drain sender |
| Phase 2 FRB 生成漏改 FakeKim | 中 | 同 PR 编译门槛；`refreshContacts` 返回类型使旧 Fake 无法通过 analyze |
| Phase 3 窗口 Snapshot 变大 | 中 | cap 400；Phase 4 测量后再谈 Delta |
| `ContactsChanged` 与 `watch_contacts` 短暂双源 | 低 | Dart 只订 watch；mpsc 仅测试/haptics |
| mark_read 网络失败导致对端未读不一致 | 中 | 已拍板：接受；下次打开会话再 `mark_read`。本切片不加可靠 outbox |

---

## Open Questions

无未决产品分叉。下列三项均已关闭（2026-09-15）：

1. **对端已读是否持久化？已关闭。** 本切片不落库。保持 HEAD：`receipts.dart` 内存单调合并。重启不保留对端已读水位。若日后要跨进程显示「已读到某条」，另开切片加表/列。
2. **本端已读网络上报是否要可靠队列？已关闭。** PR 3：本地 commit 后返回；`protocol.mark_read` 后台 fire-and-forget + epoch 校验。断线窗口由下次打开会话再 `mark_read` 覆盖。本切片不加可靠 outbox `WriteOp`。
3. **好友事件 vs 全表 refresh？已关闭。** Phase 2 用 `WriteOp::UpsertContact`。Dart 不得写名册。avatar/bio 缺字段等用户刷新。

AI 流式检查点：当前代码无独立 run checkpoint 表，不在本切片发明。

---

## References

- [08-kim-sdk-ownership.md](./08-kim-sdk-ownership.md)
- [09-mobile-production-architecture.md](./09-mobile-production-architecture.md)
- Tokio `watch`：latest-value only，[docs.rs/tokio/.../watch](https://docs.rs/tokio/latest/tokio/sync/watch/index.html)
- Flutter offline-first / data layer（职责在 Rust SDK）

核验过的 HEAD 锚点：`crates/kim-sdk/src/lib.rs`（`Inner:58`、`enqueue_message:233`、`mark_read:291`、`load_older:573`、`persist_talks_for:633`、`persist_inbox_for:685`、`replace_contacts:707`、`subscribe_timeline:945`、`publish_session_snapshot:976`、`publish_timeline:1012`、`SdkPersistHook:1075`）；`session.rs`（`bump_epoch:15-25`、`child_token:27-28`）；`store/mod.rs`（`WRITE_CAP:31`、`write_worker:688`、`ReplaceContacts` 无 epoch `:882`、`persist_enqueue:1145`、`persist_talks_tx:1277`）；`store/messages.rs`（`prune:67`、`load_hot_window:157`、`apply_talk` None 仅空 dest/body `:327-328`、`ApplyOutcome:308`、`merge_stored:517`）；`outbox/pump.rs:6-117`；`supervisor.rs:143`；`sync.rs:191-220`；FFI `client.rs:276,313,361,439,669`；Dart `messages.dart:103,166,179`；`contacts.dart:145,172,253,308,326,347`；`chat_session.dart:294`；`kim_composer.dart:99-107`；`chat_page.dart:328-330`；`mutations.dart:11`；`receipts.dart:22`；`kim_session.dart:78`；`inbox.dart:73`。

---

## PR Plan

每 PR 可独立审查、合入、revert。后一 PR 依赖前一 PR 的模块边界，不依赖运行时 flag。

### PR 1 — `sdk: ChangeLog and CommitEffect on the write worker`

- **依赖：** 无
- **文件：** `crates/kim-sdk/src/store/changes.rs`（新）；`crates/kim-sdk/src/store/mod.rs`；`crates/kim-sdk/src/store/outbox.rs`（mark_* 返回 dest）；`crates/kim-sdk/src/store/messages.rs`（`ApplyOutcome.fields_changed` / `needs_publish`）；`crates/kim-sdk/tests/query_publish.rs`（新，只断言 dirty/seq，不删 publish）
- **说明：** 写 worker 在 COMMIT 后 `record`。`apply_talk`：clone prev；loser 先删；无 loser 且字段相同才跳过 `put_msg`。测试含 mid+client_id 双 key 收成一行。

### PR 2 — `sdk: QueryPublisher replaces hand-called publish`

- **依赖：** PR 1
- **文件：** `crates/kim-sdk/src/query.rs`（新）；`crates/kim-sdk/src/lib.rs`；`crates/kim-sdk/src/outbox/pump.rs`；`crates/kim-sdk/src/store/messages.rs`（一致读）；`crates/kim-sdk/src/metrics.rs`；`tests/query_publish.rs`；`tests/timeline_watch.rs`；`tests/persist_ack.rs`
- **说明：** **无 dual-publish 切点。** spawn：`spawn_query_publisher(..., self.inner.store_life.clone())`（裸 token）。`start_session` 按 prev.account 分支 restamp+record vs Resync。命令 `wait_applied`；hook/pump 不等待；删除手工 `publish_*`（delete Resync 留下）。复用 dest sender。必须绿：`publisher_survives_start_session`、`reconnect_same_account_rebuilds`、`switch_via_start_session_holds_stamp`、`delete_thread_resync_clears_older`。

### PR 3 — `sdk: mark_read returns after local commit`

- **依赖：** PR 2
- **文件：** `crates/kim-sdk/src/lib.rs`（仅 `mark_read`）；`tests/invariants.rs` 或 `query_publish.rs`
- **说明：** 小行为 PR，不与 publisher 切换绑在一起。本地 commit + `wait_applied` 后返回；`protocol.mark_read` spawn + epoch 校验。`load_older` **不动**（仍返回本地页）。分开以便 revert 时序变化而不回滚 QueryPublisher。

### PR 4 — `sdk: contacts subscribe + refresh owned by KimSdk`

- **依赖：** PR 2
- **文件：** `crates/kim-sdk/src/contacts.rs`；`store/contacts.rs`；`lib.rs` / `query.rs` / `timeline.rs`；`sdk/mobile/rust/src/api/{client,types}.rs`；FRB 生成；`kim_bridge.dart`；`contacts.dart`；`fake_kim.dart`；`contacts_test.dart`；`contacts_settings.rs`
- **说明：** Phase 2 破坏契约。`refreshContacts: Future<void>`；`watchContacts`。`UpsertContact` 消化 Friend*/ProfileUpdated。失败 send `sync_error` 并保留缓存。删除 Dart 全部列表写路径（含 `onProfileUpdated`）。同 PR 改完调用点。

### PR 5 — `sdk: timeline window owned by subscribe_timeline`

- **依赖：** PR 2（可与 PR 4 并行，但 FFI 生成冲突则串行）
- **文件：** `query.rs`；`timeline.rs`；`store/messages.rs`；`lib.rs`；FFI `load_older` 签名；`types.rs`；FRB；`kim_bridge.dart`；`messages.dart`；`fake_kim.dart`；`timeline_watch.rs`
- **说明：** Phase 3 `loadOlder(dest)` void。游标从 snapshot oldest 推导。`load_page` 先；SQLite 还有更旧则停；sent≥400 不打 history；&lt;400 且短页且 `before_id≠0` 则 history+Keep。`loading_older` 仅经 publisher。删除 Dart `_older` / `receiveAll`。

### PR 6 — `mobile: enqueue returns receipt only`

- **依赖：** PR 5（列表已是单一来源，删乐观插入才安全）
- **文件：** `chat_session.dart`；`mutations.dart`；`kim_composer.dart`；`chat_page.dart`；`outbox_account_test.dart`；引用 `KimChatMsg` 作为 mutation 结果的 widget 测试
- **说明：** `Mutation<KimCommandReceipt>`。删除二次构造 `KimChatMsg`。Composer clear 从立刻执行改为 enqueue 成功后（Breaking Change 3.5）。

### PR 7 — `test: query-publish invariants and cost baseline`

- **依赖：** PR 2–6
- **文件：** `tests/query_publish.rs` 扩；`store_restart.rs`；`outbox_kill.rs`；`persist_ack.rs`；`inbox_test.dart`；metrics getter → 可选 `MetricsDto` 字段
- **说明：** 并发、重启、切账号、无订阅 ACK、分页中新消息。记录 commit_to_snapshot_ms 与 FFI 字节。**不**在本 PR 引入 Delta。
