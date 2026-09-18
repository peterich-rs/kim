# 修复会话未读、前台已读与多端同步

状态：已实施核心路径（协议、Chat/Royal、kim-client、kim-sdk 可见性事务与 ReadSyncWorker、Flutter 可见性协调器、Web 协议与前台判断）。真机双端联调与 Postgres 并发校准仍待验收。

## Breaking Change Notice

线协议采用新增 protobuf 字段、新命令和新增响应体，保留现有字段编号及 `chat.inbox.read` 请求格式。Rust 公共结构体新增字段、`MessageStore::mark_read` 返回值及 `PersistHook` 同步接口调整属于源码兼容性变更；FRB 接口需要一起重新生成。

迁移顺序：

1. 给 Postgres 加列并部署 Royal 存储实现；旧客户端仍可发原来的已读请求。
2. 部署 Chat 的新响应、自身账号已读推送和按会话查询状态的接口。
3. 升级 `kim-client`、`kim-sdk`，更新所有 trait 实现、测试 fixture 和 CLI 构造点。
4. 重新生成 FRB 与 Web protobuf schema，再发布 Flutter / Web 客户端。
5. 新客户端连接旧服务端时继续可靠发送旧已读请求；不能把空响应解码成一份权威的零计数快照。完整多端实时保证仅适用于新服务端与新客户端组合。

## Feasibility Assessment

已有 `Store::write_worker`、账号 epoch、`read_watermarks`、`conversation_reads`、事务后 QueryPublisher 及账号级推送能力，可以复用。直接缺口已经验证：`_start()` 只发送一次已读、重新进入复用 notifier；核心层另外存在零 ID、本地无条件清零、上报无持久重试和本人多端未同步的问题。无需修改 Agent 推理链路、WebSocket framing 或送达 ACK 协议。需要协调 Flutter、SDK、Chat/Royal 和数据库升级，并覆盖两种 inbox 读路径。Feasible with caveats: 跨层接口必须配套发布，升级和旧版本降级需遵守本文数据兼容约束。

## Current Surface Inventory

- `sdk/mobile/lib/features/chats/chat_session.dart:58-84,364-367`：notifier 初始化时上报一次已读，family 默认保活。
- `sdk/mobile/lib/features/chats/chat_page.dart:59-87,109-157`：页面生命周期、timeline 监听；收到消息只触发 Agent 动效。
- `sdk/mobile/lib/features/chats/messages.dart:108-140,181-204`：消息异步加载，倒序取最后一个非零 ID；family 默认保活。
- `sdk/mobile/lib/features/chats/inbox.dart:73-97`：从 Rust snapshot 映射列表，另一条已读入口固定传 0。
- `sdk/mobile/lib/router/app_router.dart:37-99`：嵌套 Navigator 与 indexedStack；挂载不等于当前可见。
- `sdk/mobile/lib/features/session/link.dart:28-86`：只处理 resumed 补拉，未把前后台状态交给未读核心。
- `sdk/mobile/lib/design/chat/chat_list.dart:112-125,381-388`：已有独立 `_unseen` 与“新消息”浮层，可与会话未读分离。
- `sdk/mobile/lib/bridge/kim_bridge.dart:54-96,491-498`、`sdk/mobile/rust/src/api/client.rs:469-483`：Dart / FRB 已读入口。
- `crates/kim-sdk/src/lib.rs:393-418,1397-1447,1628-1645`：本地已读、网络发送、事件消费和会话快照发布。
- `crates/kim-sdk/src/store/mod.rs:1099-1115,1692-1711,1877-1949`：串行写入、已读、消息和 inbox 事务。
- `crates/kim-sdk/src/store/messages.rs:454-558`：去重、首次插入判断、实时未读增量。
- `crates/kim-sdk/src/store/threads.rs:47-164,288-305`：本地计数、inbox 合并和服务端 tip；实时写入当前不更新 `last_message_id`。
- `crates/kim-sdk/src/store/watermarks.rs:45-77`：先无条件清零，再检查 ID、推进水位。
- `crates/kim-sdk/src/store/schema.rs:1-2,12-23,84-90`、`migrate.rs:15-68`：当前 SQLite v7，消息缓存仅保留 400 条。
- `crates/kim-client/src/persist.rs:22-29`、`sync.rs:150-195`：inbox 请求、落库和离线 Keep 策略。
- `crates/kim-client/src/client.rs:245-273`、`wire.rs:492-499`、`events.rs:67-79`：已读请求、回执和 inbox 类型。
- `crates/kim-protocol/proto/pkt.proto:398-405,500-543`：对方已读回执、inbox 快照、已读请求。
- `services/chat/src/inbox.rs:172-214`：已读成功仅通知 dest，没有同步 reader 本人的其他设备。
- `services/chat/src/store/postgres.rs:573-638,1050-1085,1204-1297`：写入累加、GROUP BY / 物化 inbox、已读事务。
- `services/chat/src/store/mod.rs:352-374,1061-1125,1176-1202`：存储 trait 和内存实现。
- `services/chat/src/royal.rs:520-540`、`services/royal/src/product.rs:356-366`：生产 Chat → Royal → Store 通路。
- `sdk/web/app/state/ChatProvider.tsx:192-217,621,720`：Web 的本地 active 判断、inbox 合并和已读调用，同样需要协议适配。

## Design

### 产品语义与目标链路

本文落实用户补充要求：**当前会话处于前台有效页面时，进入时已有的消息和随后收到的消息均视为已读，会话 item 及该会话贡献的总未读保持 0。** 不要求滚到底部；翻看历史时保留现有“新消息”浮层，该数不参与 inbox 未读和应用角标。

| 状态 | 此会话收到消息后的会话未读 |
|---|---|
| 手机当前聊天页，App resumed | 0，推进已读位置并同步 |
| 桌面当前选中会话，窗口处于前台且未最小化 | 0，推进已读位置并同步 |
| 同一会话翻看历史 | 0；可显示独立的新消息浮层 |
| 返回首页、切换会话或切换底部 tab | 正常累积 |
| 聊天页仍 mounted，但被完整页面覆盖 | 正常累积 |
| App 后台、锁屏、桌面最小化或失去前台 | 正常累积，恢复后重新激活并清已知未读 |
| 图片选择、系统权限等令 App 进入非 resumed | 暂停自动已读，恢复后重新处理 |
| 另一个设备读了同一会话 | 合并账号已读水位；该位置之前的未读消除 |

当前断点：

```text
ChatSession.build → _start → markRead（仅一次，可能 ID=0）
                              │
                              ✗ 后续消息没有持续已读
                              ▼
消息落库 → threads.unread +1 → SessionSnapshot → 首页 item
                              │
                              ✗ 再次打开复用 notifier，不再调用 _start
```

目标：

```text
ChatPage / 路由 / App 生命周期
    │
    ▼
★ ConversationVisibility → ★ Store 写队列中的当前可见会话
    │
    ▼
消息 / inbox / 历史补拉落库事务
    ├─ 当前可见：消息 + 已读水位 + 待同步记录 + unread=0
    └─ 当前不可见：去重 + 已读水位过滤 + 未读更新
    │
    ▼
QueryPublisher → SessionSnapshot / TimelineSnapshot → 首页 / 聊天页 / 角标

★ ReadSyncWorker ──► chat.inbox.read ──► Royal / Store 提交
                                            │
                                            ▼
                               ★ 本人 chat.inbox.read.sync 推送
                                            │
                                            ▼
                               其他设备 Store → QueryPublisher
```

### 关键设计决策

1. **可见性由 UI 判断，业务结果由 Rust 事务负责。** `watchThread` 只表示订阅，不表示用户在看。将“消息入库、推进水位、清未读、记录待同步”放在一个事务；不采用每次 Flutter 收到 snapshot 后再清零，因为它会暴露中间计数、依赖 Dart 调度且遗漏后台和路由切换。
2. **显式的页面可见状态取代 notifier 初始化副作用。** 单独加 `autoDispose` 无法处理 indexedStack、覆盖页面和前后台；仍应让页面级会话 notifier 自动释放以清理 typing / room interest。
3. **保留消息 ID 已读水位，区分展示最新消息与最大已知消息 ID。** `last_send_time` 用于预览排序，不能充当已读版本；Dart 列表最后一行也不保证 ID 最大。不引入全新的消息排序协议。
4. **“打开会话读全部已知消息”和“推进到指定 ID”分开。** 前者由 SDK 在事务中确定当前已知上界，后者不得无条件清掉更晚消息。0 不是“清全部”的隐式命令。
5. **已读网络同步持久化。** 本地立即生效，网络按最大水位合并、串行发送、失败重试；保留本地成功不等待网络的现有体验。一次 `tokio::spawn` 丢弃错误不足以保证断线恢复。
6. **本人多端同步与对方已读回执分开。** 保留 `chat.receipt.read` 的气泡语义，新增账号内的 `chat.inbox.read.sync`；群聊同样需要本人同步，不因此增加群成员逐人已读回执。
7. **远端计数是版本化基准，本地已读优先。** 用服务端水位、会话状态版本、请求前的本地变更代数合并；移除 `prev.unread == 0 && prev.last_at >= incoming_at` 的时间猜测。
8. **无法证明覆盖范围时按会话重新获取权威状态。** 本地仅保留 400 条消息，不能用缓存 `COUNT(*)` 代替服务器完整计数。消息 ID 也不等价于事务提交序号；较小 ID 晚到必须触发状态校准，不能凭 `max_message_id` 宣称快照包含了所有较小 ID。
9. **同账号在线推送 + 断线恢复 + 前台定期校准组成收敛路径。** 在线推送正常情况下即时生效；服务端提交后崩溃、推送丢失或客户端广播 lag 时，通过定向查询恢复，不依赖永远成功的 best-effort 通知。

上述第 1 条显式调整 `docs/impl/08-kim-sdk-ownership.md:985-997` 的旧决策：旧文要求只靠逐次 `ReadMarker`，本方案由当前用户要求改为“明确前台可见状态驱动事务内已读”。不是把任何 timeline 订阅都视为正在阅读。

### 新增接口与数据定义

`crates/kim-sdk/src/command.rs` 拟新增；`generation` 由同一个应用级可见性协调器递增，SDK 调用绑定当前账号和 epoch。

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationKey {
    pub dest: String,
    pub kind: i32,
}

#[derive(Clone, Debug)]
pub struct ConversationVisibility {
    pub generation: u64,
    pub foreground: bool,
    pub conversation: Option<ConversationKey>,
}
```

`KimSdk` 新增 `set_conversation_visibility(ConversationVisibility)` 和 `mark_thread_read(ConversationKey)`。协调器提交的是整个当前状态；旧页面只能注销自己的 token，不能直接把新页面的当前会话清空。Store 忽略旧 generation、旧 epoch 的更新。退出登录立即清空内存可见状态；进程启动默认不可见，不能从数据库恢复“正在看”。

`sdk/mobile/lib/bridge/kim_bridge.dart` 拟新增接口，消费示例：

```dart
// 由应用级 ConversationVisibilityNotifier 统一调用；不是每个页面独立维护 generation。
await client.setConversationVisibility(
  generation: generation,
  foreground: appResumed && windowActive,
  dest: currentVisibleChat?.id,
  kind: currentVisibleChat?.kind,
);
```

`crates/kim-protocol/proto/pkt.proto` 拟新增以下消息；字段编号仅针对新消息。已有 `InboxItem` 在 tag 10 增加 `ConversationReadState readState`，保持旧字段兼容。

```protobuf
message ConversationReadState {
  string dest = 1;
  int32 kind = 2;
  int64 lastReadMessageId = 3;
  int64 maxMessageId = 4;
  int32 unread = 5;
  uint64 version = 6;
  bool exists = 7;
}

message ConversationStateKey {
  string dest = 1;
  int32 kind = 2;
}

message ConversationStatesReq {
  repeated ConversationStateKey conversations = 1;
}

message ConversationStatesResp {
  repeated ConversationReadState states = 1;
}

message ConversationReadSyncPush {
  string account = 1;
  ConversationReadState state = 2;
}
```

- `chat.inbox.read`：成功响应携带 `ConversationReadState`，旧请求格式不变。
- `chat.inbox.states`：批量查询当前账号指定会话的状态，每批最多 100 个 key；不是只查询 inbox 首页的 50 个条目。每个合法 key 都返回结果，`exists=false` 明确表示没有该账号的会话数据，不能把缺失条目猜成 unread=0；不存在的 key 不得导致整批其他 key 查询失败。Royal 配套新增 `/api/v1/inbox/states`，账号从经过认证的 Chat 会话绑定，不能让客户端任意读取别人的计数。
- `chat.inbox.read.sync`：只推送 reader 本人其他在线 channel；客户端检查当前账号、kind、epoch。消息里包含快照，但 push 的主要作用是推进水位和触发定向校准，不直接无条件覆盖本地计数。
- `version`：同一 `(app, account, dest, kind)` 内，每次消息入库或有效已读改变，在既有账号锁与事务内递增；与时间戳、消息 ID 不混用。Postgres 使用新建的数据库 sequence 分配版本，并在拿到账号锁后取值；允许空洞，避免会话行被 purge 后重建导致版本回到 1、客户端永远拒绝新状态。Memory 使用 store 级计数器。重复插入和不改变水位的重复已读不得凭空增加计数。
- `maxMessageId`：该账号会话已提交消息的最大 ID，独立于按 `(send_time, message_id)` 排序的展示最新消息。

SQLite v8（实施时确认版本未被其他改动占用）：

- 扩展 `read_watermarks`：记录 `kind`、本地水位、服务端确认水位、重试次数、下次重试时间、最后错误；`local > confirmed` 即待同步，无需另存易失配的 dirty 布尔值。现有 account/dest 唯一性沿用，验证 kind 不可冲突。
- 新建 `conversation_read_state`：按 account/dest/kind 保存 `server_version`、`server_read_id`、`server_max_message_id`、`server_unread`、`local_generation`、`needs_refresh` 和当前已知最大 ID。消息裁剪不删除这些数据。
- 可见会话只保存在写 worker 内存；网络重试信息持久化。账号 epoch 绑定当前执行，不把旧进程 epoch 当成重启后不可恢复的持久任务条件。
- Postgres 给 `conversation_inbox` 增加 `state_version`、`max_message_id`；`conversation_reads.last_read_id` 继续是服务端水位权威。所有插入路径都更新版本与最大 ID，包含普通单聊、群聊、bot reply、发送者镜像。

### 事务与合并规则

**可见状态与消息串行化：** 激活/失活写操作和 `PersistTalks` / `PersistInbox` 共用 Store 队列。激活事务清除已知未读，并将 `max(已知会话最大 ID, 本地消息最大 ID)` 写入本地水位和待同步状态；若没有任何有效 ID，只记录可见性，不向服务端发送 0。随后 inbox、实时消息或最新历史完成时，仍在前台就继续推进。失活完成后的新消息恢复正常计数。队列满时可见性更新必须可重试，不能静默丢弃；切换命令完成是事务顺序边界。

SDK 每次 `start_session` / epoch 变化都使旧 visibility 失效，并通知协调器重新提交当前页面状态；不能因为路由没有变化就省略重报。同一 epoch 内的普通网络重连保留本地前台状态。发送确认 `mark_sent_tx` 也更新已知最大 ID，避免仅收信与 inbox 更新了 tip。

**实时收信：** 对同一事务，先去重并落库，再读有效可见性和已读水位。当前可见则推进已知最大 ID、`unread=0` 并记录待同步，最终只发布这一份状态。不可见时，自己发送、系统消息、已存在消息、ID 不超过有效已读水位的消息均不增未读。同步补拉沿用 Keep，不把服务器计数再累加一遍。

**计数与未知覆盖：** 对不可见会话，首次 live 插入且 ID 大于服务器已知最大 ID、也大于本地已读水位时可本地 +1。首次插入但 ID 位于服务器快照覆盖上界内时，不重复 +1，持久设置 `needs_refresh` 并查询权威状态；它可能已被快照计数，也可能是较小 ID 后提交，必须通过新查询区分。后者允许短暂待校准，不能长期漏计。离线期间保留最后基准和本地增量，上线后收敛。

**请求前的本地变更屏障：** `chat.inbox.list` / `chat.inbox.states` 发出前，为每个会话捕获本地 `local_generation`，不存在的会话视为 0；响应落库前比较。如果收信、已读、删除或账户切换已经改变 generation，则不能用该响应的绝对 unread 覆盖更新后的本地计数，只合并单调水位、标记重新查询。`PersistHook` 增加 `begin_inbox_sync` 返回不透明 token，`persist_inbox` 接收 token；SDK 内部存放对应的账号、epoch 和 generation 集合，结束/错误/取消时释放，不能无限积累 token。没有 store 的 CLI 保持无 hook 路径。

**水位优先：** 本地有效水位为 `max(local_read_id, server_read_id)`。返回 `server_read_id < local_read_id` 的快照不能恢复已经清掉的未读；继续重试本地已读上报。返回版本比已存版本旧的计数丢弃，但水位仍允许取 MAX。若收到的远端水位覆盖全部当前已知消息，立即置 0；否则不能猜测缓存之外有多少未读，保留未覆盖部分并发起定向查询。新查询在 generation 未变化、服务端水位不落后时才替换绝对计数。没有静止窗口的高流量会话合并 dirty 请求，不阻塞实时收信；停止变化后必须完成一次成功校准。

**指定 ID 的兼容入口：** `mark_read(ReadMarker)` 保持含义为读到该 ID，不清掉更大 ID 的未读。新增 `mark_thread_read` 用于页面进入，事务内解析整个已知 tip。已有 0 调用改为后者；保留的指定 ID API 对 0 返回无效参数/明确 no-op，不能一端清零另一端忽略。部分区间无法从已裁剪的缓存精确计算时进入状态查询流程，不伪造总计数。

**可靠发送：** 每个会话最多一个在途请求，其他新水位只合并 MAX；本地写入不节流，网络做 150–250ms 合并，离开会话或进后台时尝试立即 flush。成功只确认本次真实提交的水位，不能把期间新增水位一起标成已同步；失败保留任务，以带抖动的退避重试。断网等待 Link Online，认证错误等待恢复；失败不得回滚本地已读。进程重启、同账号重登、前台恢复、同步完成均唤醒 worker。协议空响应的旧服务端可确认本次成功提交 ID，但不产生虚构状态快照。

对新接口返回的 NotFound/InvalidArgument，不做无限短周期重试：先定向查询并确认会话是否存在、请求 ID 是否已被有效水位覆盖；真实协议/数据错误保留可诊断状态并停止该项自动重试，不能伪造 confirmed。若服务器明确删除了该会话，则终止该项同步，并走既有删除事件清理规则。重试队列错误不阻塞其他会话。

**服务端原子性：** 锁内验证 message_id 属于当前账号的指定会话，防止误传别的会话/未来 ID；已读 MAX 更新、精确 COUNT、物化计数、state_version 和返回状态属于同一事务。响应和自身推送携带实际生效水位，而非原请求较小水位。入库增量同样检查 `message_id > last_read_id`，防止低 ID 晚提交在物化表多 +1。所有物化与 GROUP BY 状态字段必须来自一致快照，不能分几个无事务查询拼接。

**掉推送恢复：** 成功提交后正常推本人其他设备并保留原 peer receipt。客户端在前台上线、恢复、进入列表、广播 lag 时定向刷新；此外前台在线每 30 秒对当前账号的本地会话分批校准，单批上限 100、同一会话单飞、加入抖动，离线/后台停止轮询。只有新版本接口可用时启用该批查询。网络恢复后最终一致；不承诺断网设备实时同步。这样即使服务端提交后在 push 前崩溃，其他一直在线的设备也能修复。

## Phased Implementation

### Phase 1: 固定前台语义并建立可见性入口

#### Issue 1 [P1]: 让每次实际进入、离开与前后台变化驱动会话可见性

File: `sdk/mobile/lib/features/chats/chat_session.dart` — `build` / `_start` / `chatSessionProvider` (lines 58-84,364-367)

Problem: 当前页面收到 Agent 回复后留下未读，再次进入无法清除。`_start:75` 的一次调用既漏掉后续消息，也因 `chatSessionProvider:364` 保活而不再触发。`app_router.dart:69` 的 indexedStack 还会保留不可见页面，单凭 mounted 或监听数量会误标已读。

Approach — 建立唯一的会话可见性协调器：

- 新增 `sdk/mobile/lib/features/chats/conversation_visibility.dart`；每个页面以 token 注册，协调器汇总根/分支 Navigator 顶层路由、当前 tab、selectedId、App lifecycle 和桌面窗口状态，生成单调 generation。
- 修改 `chat_page.dart:59-87`、`app_router.dart:37-99`、`link.dart:82-86`，覆盖 push/pop、didUpdateWidget、App resume/pause、宽窄布局、桌面 A→B 切换。全屏覆盖视为不可见，输入框/键盘和不遮住会话的轻量弹层不自动视为离开。
- `chatSessionProvider` 与页面级 `threadMessagesProvider` 使用 autoDispose，移除 `_start` 中的已读副作用。所有 `ref.read`-only 调用方先审计；需要持续存在的工作迁到应用级服务，而非靠 provider 泄漏维持。
- room interest、typing 的退出随页面会话释放；它们不作为已读权威，尤其 bot 当前不进普通 user room。
- 接口先完整贯通 FFI/SDK 并提供旧已读 API 兼容，保证这一阶段可编译。

拟修改 `chat_session.dart` Provider 定义：

```dart
final chatSessionProvider =
    NotifierProvider.autoDispose.family<
      ChatSessionNotifier,
      ChatSessionState,
      String
    >(ChatSessionNotifier.new);
```

Structural changes:

- `KimClientPort`、`KimBridge`、`KimUiHandle`、Rust command 新增可见性与整会话已读接口；由现有 FRB 流程生成文件。
- 新增 root/branch RouteObserver 或等效的顶层路由通知；若当前窗口集成不能给出桌面前台/最小化状态，应补原生窗口事件，不能只监听文本框 focus。

Existing test adjustments:

- `sdk/mobile/test/state/chat_session_kind_test.dart:12-75`：持有 provider subscription 后验证 kind，适应 autoDispose。
- `sdk/mobile/test/support/fake_kim.dart:342`：记录 visibility generation、read 请求及失败注入；不要让 fake 自己实现全部 Rust 计数算法来替代核心测试。

New tests:

- 新增 `test/state/conversation_visibility_test.dart`：进入→离开→重进；验证第二次激活、旧 token 清理不能关闭新会话。
- 新增 widget 场景：indexedStack 切 tab、根路由覆盖、resume、桌面 selectedId 改变均产生正确 active/inactive；测试 `watchThread` 存在但不可见不激活。
- 翻看历史收到新消息：会话未读保持 0，`chat_list.dart:381` 的 `_unseen` 浮层仍可出现。

### Phase 2: 在 Rust 事务中维护正确本地状态

#### Issue 2 [P1]: 原子处理前台收信和会话已读

File: `crates/kim-sdk/src/store/mod.rs` — `write_worker` / `persist_talks_tx` (lines 1099-1115,1877-1922)

Problem: `messages::apply_talk:528-534` 对所有新的非自身 live 消息 +1，不知道前台会话；在 Flutter 后续清零会把中间未读发布到列表和总角标。

Approach — 把可见性与消息处理放入同一串行写入状态机：

- worker 增加 visibility 写操作，持有 account/epoch/generation/current conversation；事务在提交前决定最终 unread。
- `persist_talks_tx`、`persist_inbox_tx`、最新历史 hydration 均经过一致的 read projection；公开查询不能看到“本事务消息已入库但已读未推进”的状态。
- `messages.rs:528` 增加有效已读水位过滤；去重仍以 message ID / client ID 合并为准。
- `threads.rs:47-104` 传入并维护有效最大 message ID，不再仅保留 inbox 初次提供的旧 tip。
- `store/mod.rs` 的 `mark_sent_tx` 同步维护已知 tip；成功发送的自身消息不因此增加未读。
- 历史分页本身不产生新增未读；active 后的补拉可推进已知 tip。合成 `goose` / `agent:` 仅本地会话不向服务器发已读，`b_` 会话走普通服务端路径；本地合成消息不能伪造服务器 ID。
- 保持消息 commit 后才发 delivery ACK；已读网络请求在事务外执行。

Existing test adjustments:

- `crates/kim-sdk/tests/unread_replay.rs:52,67` 的重复消息和 Keep 策略保持；增加显式不可见前提。
- `invariants.rs:179` 的快照测试断言 inbox/timeline/total 来自最终提交结果。

New tests:

- 新增 `tests/active_read.rs`：active bot/person/group 收到回复→所有发布快照中该会话 unread=0，DB 水位推进、存在待同步。
- active→inactive 与收信双向交错：以写队列顺序为界，离开后消息正常 +1。
- active 时先 inbox 后消息、先消息后 inbox、初始空库晚加载、重复/乱序/裁剪后重放：均不产生残留未读。
- 写事务失败：消息、水位和未读一起回滚，不发送 delivery ACK 或确认已读成功。

#### Issue 3 [P1]: 消除零 ID 和过期标记的错误清零

File: `crates/kim-sdk/src/store/watermarks.rs` — `advance` (lines 45-77)

Problem: line 52 无条件清零，line 58 才判断 ID；`messages.dart:181-187` 在空快照取到 0，或按显示顺序取到非最大 ID。服务端 `postgres.rs:1212` 对 0 直接忽略，产生本地与服务端分裂；旧 read 标记也可能清除更新消息的计数。

Approach — 明确整会话与指定水位两类命令：

- 将 `advance` 拆为纯单调水位更新和调用方的计数投影；显式 read-to-ID 不能无条件清零。
- `mark_thread_read` 在事务内取已知 tip；取消从 Dart `state.items.reversed` 推断已读上界。inbox 的固定 0 入口改走同一个 SDK 命令。
- 对服务端会话所有网络请求满足 `message_id > 0`；空会话保持 active 等待有效数据。
- 部分已读后仍存在高于水位的消息时保留未读；缓存不完整时定向查询服务器，不能用 400 条缓存倒推出全量计数。

Existing test adjustments:

- `invariants.rs:356,409`：保留无协议/协议挂起时本地及时返回；补有效 ID 与较新消息断言。

New tests:

- 初始 timeline 空，inbox 后到：无 ID=0 网络请求，tip 到达后自动推进。
- ID 101、102 的显示时间颠倒：整会话读到 102。
- 读到 101 与 102 新消息并发：102 不被旧标记误清；自己发送 pending ID=0 不影响水位。

### Phase 3: 完成可靠已读传输与服务端契约

#### Issue 4 [P1]: 持久化、合并并重试未确认已读

File: `crates/kim-sdk/src/lib.rs` — `mark_read` (lines 393-418)

Problem: lines 406-416 仅 spawn 一次网络调用且丢弃结果；断线、超时、退出进程后服务端水位可能永久落后。

Approach — 引入 ReadSyncWorker：

- 新增 `crates/kim-sdk/src/read_sync.rs`，复用 epoch/cancel、Link Online、token 生命周期与 Store；数据结构和退避规则见 Design。
- mark_read / active 收信事务持久化待发送水位，worker 从数据库取 due 项。成功通过 Store 确认本次水位，故障记录结构化错误；停止会话取消执行但保留待同步数据。
- `delete_thread` 不再删除尚未确认的 read watermark；删除可见列表和历史不等于撤销已读。read-only push 不重建被本地删除的列表项，只更新读状态；真正新消息仍沿用现有重建逻辑。
- 实际 DELETE 位于 `crates/kim-sdk/src/store/outbox.rs:324-347`，连同新的 `conversation_read_state` 一起审计。保留变更 generation，确保删除前发出的 inbox 响应不能直接复活该项。
- 本地删除与同期 `chat-inbox-hide.md` 方案协调，不能随手把未确认已读任务一起清除。账号永久删除才按账号范围清理。

Structural changes:

- `ProtocolClient` 增加可返回确认状态的接口，保留旧 `mark_read -> ()` wrapper；测试 fake 支持挂起、失败、乱序响应。
- SQLite 升级及任务查询/确认成为 Store 写操作；新增指标 `read_sync_pending`、`read_sync_retry_total`、`read_sync_lag_ms`。

Existing test adjustments:

- `invariants.rs:409`：协议永久挂起时本地依旧完成，pending 保留。
- `tests/delete_thread.rs`：修改读任务生命周期断言，消息/发送 outbox 删除行为保留。

New tests:

- 新增 `tests/read_sync.rs`：断网阅读→重启→重连→服务端确认；不需要再次点击会话。
- 发送 100 期间推进到 120，100 ACK 只确认 100，仍会发送 120；重复、乱序 ACK 不回退。
- 账号 A→B，A 请求迟到不得修改 B；切回 A 后恢复其持久任务。
- 连续 100 条 active 回复合并网络请求，但每次本地快照均为 0。

#### Issue 5 [P1]: 建立本人多端的已读同步

File: `services/chat/src/inbox.rs` — `do_inbox_read` (lines 172-214)

Problem: line 206 只通知聊天对方，本人其他设备保持旧计数；普通 `ReceiptRead` 只用于气泡显示，Flutter `receipts.dart:22-42` 不更新 inbox。

Approach — 提交后返回权威状态并推送 reader 的其他设备：

- 修改 `MessageStore::mark_read` 返回 `ConversationReadState`，贯通 Postgres、Memory、Royal HTTP、Chat 响应。`services/royal/src/product.rs:356-366` 必须同步修改，不能只修内存测试路径。
- `do_inbox_read` 成功后调用现有 `notify_account` 向 reader 账号发送新命令，peer receipt 保留并携带实际生效 ID；重复已读允许重发自身状态以修复上次通知失败。
- `kim-client` 添加 command、解码、Event、SessionEvent；SDK 将本人同步先落库再发布，不能只转成 Dart 离散事件。
- 新增按会话状态查询和前台周期校准，覆盖提交后 push 失败及 broadcast lag。
- Web SDK 添加 callback/状态接口，Web app 使用 document visibility 与实际 active route；读取失败进入重试，而非忽略 status。使用相同账号内水位规则；其持久任务用现有账号隔离 store 扩展，不能仅存在组件内存。

Existing test adjustments:

- `services/chat/tests/e2e_typing_receipts.rs:135`：仍验证 peer receipt，新增本人另一 channel 同步。
- 同文件 `:189`：改为“群聊不发 peer receipt，但发送本人多端 read sync”。
- `kim-client` 中所有 InboxItem 构造点和协议 fixture 显式兼容缺失的新字段。

New tests:

- 双设备同账号：手机读 bot→电脑同会话归零；电脑读→手机归零；无关会话保持。
- 同账号设备混合新旧版本：旧客户端发 read，新服务端依然通知新客户端；新客户端收到旧空响应不把所有会话误清零。
- 服务端提交成功但 push 丢弃：另一台始终在线，前台周期查询能修正。
- 超过 50 个会话、目标不在 inbox 首页：指定会话查询仍能同步。

### Phase 4: 修复快照合并、后提交消息与历史数据

#### Issue 6 [P1]: 用水位和请求代数合并快照

File: `crates/kim-sdk/src/store/threads.rs` — `persist_inbox_item` / `merged_unread` (lines 107-164)

Problem: line 160 用 unread==0 和 last_at 判断本地已读，自己刚发送消息、同毫秒回复、旧 inbox 晚返回均可能误判。当前 snapshot 没有服务端已读水位，也无法区分响应是否早于本地变化。

Approach — 落实 Design 中的版本、水位与请求前变更屏障：

- 扩展 inbox 状态映射、Store 读状态及本地 generation；`PersistHook::begin_inbox_sync` 在 `SyncEngine::run:161` 发请求前调用，成功/失败均释放 token。
- 删除基于时间的 `merged_unread`；用于 latest preview 的时间规则继续独立存在。
- realtime / read / inbox / read response / self push 都走统一读状态合并函数，字段变更通过 CommitEffect 触发 inbox 与 timeline 发布。
- 无屏障的服务器 push 只推进水位并使状态 dirty；水位覆盖全部已知消息可立即归零，否则定向查询，不能强行套用绝对计数。
- `last_read_message_id` 在 timeline 有变化时仍正确发布；Flutter 不再重复构造第二份未读计数。

Existing test adjustments:

- `unread_replay.rs:81` 的“local read wins”改成有真实 ReadMarker / server_read_id 的场景，不能以一份 unread=0 的 inbox 伪装用户已经阅读。
- `unread_replay.rs:94` 保留不可见时不会自动清零；新增显式 active 时归零的对应测试。

New tests:

- 旧 list 请求在本地读之后返回，不复活未读；旧请求发出后新消息到达，不吞掉新未读。
- 快照已计入消息但 live 后到，不重复 +1；低 ID 后提交且未被旧快照包含，刷新后补足计数。
- read sync 到达先于消息内容，随后补拉/重放不再次增加已读消息的未读。
- 501 条以上未读、本地只缓存 400 条：同步后总数正确；不能靠本地 COUNT 得到错误零。

#### Issue 7 [P1]: 保持服务端所有 inbox 读写路径一致

File: `services/chat/src/store/postgres.rs` — `upsert_inbox_rows` / `mark_read` / `inbox` (lines 573-638,1050-1085,1204-1297)

Problem: 物化写入 line 591 无条件给接收消息 +1，而 GROUP BY line 1067 按已读水位过滤。消息 ID 在事务完成前生成，不能假设 ID 顺序等于提交顺序；已读水位之后晚提交的低 ID 会造成两条路径计数分歧。

Approach — 在账号锁与事务内更新计数、版本和最大 ID：

- 新 migration `services/chat/migrations/0016_conversation_read_state.sql`（实施时确认序号）：增加列、版本 sequence、约束和必要索引；回填 `max_message_id` 来自会话索引实际 MAX。
- `upsert_inbox_rows` 仅在接收且 ID 高于有效 last_read 时增加 unread；每个实际改变状态的写操作递增 `state_version`。
- `mark_read` 校验会话归属，MAX 更新水位后精确重算该会话 unread；在同一事务里读取返回值。旧/重复请求返回现有有效状态。
- GROUP BY 和物化 list / states 读取同一套 read state；查询多个字段使用同一 MVCC 快照。缺失物化行在锁内按索引真实聚合补齐，不以 unread=0 的空行替代。
- Memory 实现复刻同样不变量；Bot 回复和发送者镜像必须经过统一路径。

Existing test adjustments:

- `services/chat/tests/e2e_inbox.rs:27`：验证返回服务端水位、版本、计数及无效 ID 行为。
- `store/mod.rs` 现有 inbox/read 测试同步 trait 返回值。

New tests:

- Postgres 并发测试用 barrier 制造高 ID 先提交/先读，低 ID 后提交，物化与 GROUP BY 两种配置计数相同。
- 重复 message_id/client_id、重复 read、并发两端读不同 ID：水位不回退，unread 非负，不重复加。
- 跨会话 message_id、未知 kind、其他账号 key：请求被拒绝，不改变任何水位。

#### Issue 8 [P2]: 迁移并修复已有错误计数

File: `crates/kim-sdk/src/store/migrate.rs` — `run` (lines 15-68)

Problem: 仅修新消息路径会留下旧设备中的错误计数；现有本地水位未记录服务端是否确认。`prepare.rs:31-34` 对高版本库拒绝打开，随意回滚旧二进制不能正常读取新库。

Approach — 非破坏迁移并逐会话校准：

- 新 SQLite 版本保留 messages、outbox、contacts、profiles；已有有效本地水位按“尚未确认”迁移，启动后可靠重报；0 水位不因旧 unread=0 推断已读。
- 对已有会话逐批获取服务端状态；有本地有效水位先补报，有明确 active 会话按已知 tip 清读。没有可证明水位的旧幽灵计数在用户实际打开后修复，不能全账号一键清零吞掉真实未读。
- Postgres 回填计数采用与在线写相同账号锁、精确 COUNT 与现有 read cursor；脚本分批、可重入，不能长时间全表阻塞，也不直接改用户已读位置。
- 待同步水位与 state 不随消息裁剪消失；本地删会话不丢尚未完成的确认任务，自身 push 不凭空恢复被隐藏的列表项。
- 回滚采用能识别新 schema 的兼容构建关闭新行为；不删除数据库，不让旧 `prepare_store_file` 把缓存/未发消息当损坏数据清理。

Existing test adjustments:

- `tests/store_restart.rs` 增加 v7→v8 fixture；旧数据和发送 outbox 原有恢复断言保留。

New tests:

- 旧库有有效水位但服务端未读非零→升级重报后收敛；旧库只有错误 unread 没有水位→打开目标会话后修复，无关会话不变。
- migration 中断回滚、重复执行、消息缓存裁剪及删除会话后重启：已读任务与其他账号数据均保持正确。

### Phase 5: 联调、验证与分步发布

- 先上线兼容服务端，再发布新客户端；保留旧协议回退，但不宣称旧客户端已获得完整同步能力。
- 客户端同时启用可见性事务、持久读同步、合并规则；不要把其中一层当作最终交付。
- 监控 read pending 数量/最大滞后、状态校准失败、旧 generation 丢弃、前台会话非零未读异常；日志只记录 request/account scope 标识、dest、epoch、message ID、version 和计数，不记录聊天正文/token。
- 人工复现严格覆盖原始路径：电脑和手机同账号在线→电脑给 Agent 发消息→手机进入同一 item→手机发送、等待 Agent 回复→返回列表为 0→重新进入仍为 0。再用真人和群聊重复。
- 把“本地立即归零”和“其他在线端最终收敛”分别观测；测试环境单次网络往返完成后同步，掉 push 的前台端在一次定期校准成功后修复。

## Architectural Notes

- 权限：本方案只形成文档，不修改生产实现、不部署服务、不清理用户聊天数据。
- 一致性范围：前台本地事务内已读，跨设备依赖网络的最终一致；离线设备恢复时收敛。后台到达的真实新消息仍要计数。
- 保持 persist-then-ACK：送达 ACK 与用户已读是两个独立概念，不能因为 ACK 成功就视为已读，也不能等待已读网络请求才 ACK 消息。
- 已读逻辑归 `kim-sdk`，Flutter 只提供可见性并展示 snapshot；Web 没有 Rust store，需对等适配，不能复用 Flutter 的状态对象。
- 不复用 typing、room interest 或 Agent busy 信号推断阅读；它们的生命周期、收件人和权限语义不同。
- 不要求变更消息加密/传输、Agent 推理、网络重连协议和既有会话隐藏产品语义；只协调隐藏与已读任务的数据生命周期。
- trait 返回值和 protobuf DTO 构造变化必须全仓编译检查；使用当前 async_trait 模式，保持 `dyn ProtocolClient` / `dyn MessageStore` 可用。
- UI generation、SDK session epoch、服务端 state_version 各自解决不同的乱序问题，不能合并为一个时间戳。
- 不增加跨 crate 循环依赖；持久重试与计数归 SDK，协议层只提供类型、解码与 hook。
- 原工作区已有 agent/skills 功能修改及 `chat-inbox-hide.md` 草案；实施时只处理本文相关交叉点，保留用户其他改动。

## File Change Summary

- `crates/kim-client/src/client.rs` -- 已读带状态响应与指定会话状态查询。
- `crates/kim-client/src/events.rs` -- inbox read state 与本人已读同步事件。
- `crates/kim-client/src/lib.rs` -- 导出新增协议接口类型。
- `crates/kim-client/src/link/machine.rs` -- 转发本人已读同步与恢复信号。
- `crates/kim-client/src/persist.rs` -- 请求前同步屏障 token 契约。
- `crates/kim-client/src/supervisor.rs` -- SessionEvent 新增本人已读状态。
- `crates/kim-client/src/sync.rs` -- inbox 请求前捕获屏障并贯通落库。
- `crates/kim-client/src/tests.rs` -- 协议与 PersistHook fixture 适配。
- `crates/kim-client/src/wire.rs` -- read response / states / read sync 编解码。
- `crates/kim-protocol/proto/pkt.proto` -- 增量 protobuf 契约。
- `crates/kim-protocol/src/command.rs` -- 新命令枚举、解析与全集测试。
- `crates/kim-protocol/src/lib.rs` -- 导出新增命令常量。
- `crates/kim-protocol/src/wire.rs` -- 定义 states/read-sync 命令常量。
- `crates/kim-sdk/src/command.rs` -- 可见性和整会话已读命令。
- `crates/kim-sdk/src/lib.rs` -- 入口、事件持久化、worker 生命周期。
- `crates/kim-sdk/src/metrics.rs` -- 已读可靠性指标。
- `crates/kim-sdk/src/proto.rs` -- ProtocolClient 已读响应和状态查询。
- `crates/kim-sdk/src/read_sync.rs` -- 新增持久同步、退避与定向校准 worker。
- `crates/kim-sdk/src/store/messages.rs` -- 去重后的水位过滤。
- `crates/kim-sdk/src/store/migrate.rs` -- 非破坏 schema 升级。
- `crates/kim-sdk/src/store/mod.rs` -- 可见性、消息、读任务串行事务。
- `crates/kim-sdk/src/store/outbox.rs` -- 删会话保留未确认读任务及变更 generation。
- `crates/kim-sdk/src/store/schema.rs` -- 本地确认水位和读状态元数据。
- `crates/kim-sdk/src/store/threads.rs` -- 移除时间猜测，合并版本化状态。
- `crates/kim-sdk/src/store/watermarks.rs` -- 纯水位更新与同步确认。
- `crates/kim-sdk/tests/active_read.rs` -- 新增前台事务与生命周期交错测试。
- `crates/kim-sdk/tests/delete_thread.rs` -- 保留未确认已读任务。
- `crates/kim-sdk/tests/invariants.rs` -- 本地立即生效、最终快照和网络不阻塞。
- `crates/kim-sdk/tests/read_sync.rs` -- 新增重试、掉电与账号隔离测试。
- `crates/kim-sdk/tests/store_restart.rs` -- 迁移及重启恢复。
- `crates/kim-sdk/tests/unread_replay.rs` -- 水位、乱序与快照竞争测试。
- `docs/impl/08-kim-sdk-ownership.md` -- 标注被替换的可见性决策并链接本文。
- `docs/impl/09-mobile-production-architecture.md` -- 更新 Flutter 可见性命令边界。
- `docs/impl/mobile-data-flow-convergence.md` -- 更新已读事务与网络重试路径。
- `docs/mobile-client.md` -- 前台未读行为与升级说明。
- `docs/presence-room-interest.md` -- 区分 peer receipt 与自身账号读同步。
- `docs/user-social-inbox.md` -- 记录协议、版本、水位与服务端校准。
- `sdk/mobile/lib/bridge/kim_bridge.dart` -- FFI 可见性/整会话已读接口。
- `sdk/mobile/lib/features/chats/chat_page.dart` -- 注册和释放页面可见 token。
- `sdk/mobile/lib/features/chats/chat_session.dart` -- 移除一次性已读，页面级 autoDispose。
- `sdk/mobile/lib/features/chats/conversation_visibility.dart` -- 新增全局可见性协调器。
- `sdk/mobile/lib/features/chats/inbox.dart` -- 移除 messageId=0 已读入口。
- `sdk/mobile/lib/features/chats/messages.dart` -- 不再从显示顺序计算整会话已读 ID。
- `sdk/mobile/lib/features/session/link.dart` -- 向协调器报告完整 App 生命周期。
- `sdk/mobile/lib/router/app_router.dart` -- 根/分支导航可见性通知。
- `sdk/mobile/lib/src/rust/` -- FRB 生成产物，不手改。
- `sdk/mobile/rust/src/api/client.rs` -- SDK 命令薄适配。
- `sdk/mobile/rust/src/api/types.rs` -- 新增可见性 DTO。
- `sdk/mobile/rust/src/frb_generated.rs` -- FRB 生成产物。
- `sdk/mobile/test/state/chat_session_kind_test.dart` -- autoDispose 订阅适配。
- `sdk/mobile/test/state/conversation_visibility_test.dart` -- 新增前台、路由与代数测试。
- `sdk/mobile/test/support/fake_kim.dart` -- 可见性/失败注入记录。
- `sdk/mobile/test/widgets/conversation_unread_test.dart` -- 新增原始用户路径和总角标验证。
- `sdk/web/app/lib/chat.ts` -- read sync、状态查询和有效错误传播。
- `sdk/web/app/state/ChatProvider.tsx` -- active/visibility 与读状态合并。
- `sdk/web/src/client.ts` -- 新响应与自身同步 callback。
- `sdk/web/src/command.ts` -- 新命令。
- `sdk/web/src/proto.ts` -- 新字段编解码。
- `sdk/web/src/proto/pkt.json` -- 从 proto 重新生成。
- `sdk/web/src/store.ts` -- 账号隔离的待同步水位持久化。
- `services/chat/migrations/0016_conversation_read_state.sql` -- 新增服务端状态版本和最大 ID；序号实施时确认。
- `services/chat/src/inbox.rs` -- 已读返回状态、本人推送和状态查询。
- `services/chat/src/lib.rs` -- 注册新命令与处理入口。
- `services/chat/src/royal.rs` -- 生产 RPC 契约适配。
- `services/chat/src/royal_pool.rs` -- 新 RPC 路径分类及契约测试。
- `services/chat/src/store/mod.rs` -- trait 和 Memory 实现。
- `services/chat/src/store/postgres.rs` -- 事务计数、版本和两种读路径。
- `services/chat/tests/e2e_inbox.rs` -- 状态与水位回归。
- `services/chat/tests/e2e_read_sync.rs` -- 新增双设备和掉推送回归。
- `services/chat/tests/e2e_typing_receipts.rs` -- 保持对方回执，增加本人同步断言。
- `services/royal/src/lib.rs` -- 注册批量 states HTTP 入口。
- `services/royal/src/product.rs` -- read/state protobuf 响应映射。

## Verification

以下是实施阶段的验证门槛，不是本次已执行的结果。已有临时诊断验证的是缺陷当前确实存在。

- `cargo check --workspace` -- 全仓类型、trait 与新增 DTO 构造点编译。
- `cargo test -p kim-protocol -p kim-client -p kim-sdk` -- 核心状态机、迁移和掉线/重放测试。
- `cargo test -p chat --test e2e_inbox --test e2e_typing_receipts --test e2e_bot --test e2e_read_sync` -- 服务端既有功能与本人多端同步。
- `cargo test -p royal` -- 实际 Royal 接口返回契约；增加 Chat → Royal → Postgres 集成用例，内存通过不能替代。
- Postgres 实例上分别启用/关闭 `KIM_INBOX_MATERIALIZED` 跑并发 read/insert、回填和双端测试；使用相同输入 oracle 比较。没有数据库环境时明确未验证，不能标记已全部通过。
- `cargo fmt --all -- --check`、`cargo clippy -p kim-client -p kim-sdk -p chat -p royal --all-targets -- -D warnings` -- 格式与静态检查。
- 在 `sdk/mobile` 按 `flutter_rust_bridge.yaml` 执行 `flutter_rust_bridge_codegen generate`；在 `sdk/web` 执行 `npm run gen-proto`，检查生成差异只含本方案接口变化。
- `cargo check --manifest-path sdk/mobile/rust/Cargo.toml` -- workspace 外 FFI crate 实编。
- 在 `sdk/mobile` 执行 `flutter test test/state test/widgets/conversation_unread_test.dart test/agent/chat_page_agent_test.dart`、`flutter analyze --fatal-infos --fatal-warnings`、`dart format --output=none --set-exit-if-changed lib test`。
- 在 `sdk/web` 执行 `npm run typecheck`、`npm test`；增加 active tab、background tab、持久重试及同账号读同步测试。
- 真机手机 + 桌面同时在线完成原始路径，以及后台、重进、断网恢复、重启、切账号、翻历史、群聊、非当前会话、长历史、重复/乱序消息的矩阵。
- 检查首页 item、总未读、系统角标若接入均来自同一最终 snapshot；对 active 收信断言整个发布序列没有短暂非零，而不只检查最终值。
- 同步上述设计文档中的旧可见性规则、消息 ACK / 用户已读语义和升级回滚要求。
