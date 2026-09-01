# 移动端成熟化：同步引擎、自研聊天列表与 UI 体系

| 字段 | 值 |
|---|---|
| 状态 | Draft |
| 作者 | — |
| 日期 | 2026-09-01 |
| 对照代码 | HEAD `293fe39`。行号以标识符为准。 |
| 父规格 | [next-stage.md](./next-stage.md) 剩余 Phase 4 合同（本文件将其落地并扩展）；UI / 聊天页部分为新范围。 |
| 已拍板方向 | 自研 ChatList（去 flutter_chat_ui）；同步逻辑下沉 Rust kim-client；视觉 Telegram 风、消息布局 Discord 单列；先设计后实施。 |

---

## Breaking Change Notice（仓库内部）

不涉外部 crate / 服务端 / Web SDK。但以下内部契约一次性变更，**必须整组合入**，不允许部分落地：

1. `sdk/mobile/pubspec.yaml` 移除 `flutter_chat_core` / `flutter_chat_ui`；聊天页全部 builder 迁移到自研组件。依赖删除与替换必须同一 PR。
2. `crates/kim-client` 发送 API 重命名并结构化：`talk_to_user` / `talk_image` 删除，统一为 `send_message(dest, kind, content, client_id)`，`OutgoingContent` 枚举承载文本/图片/视频/语音。仓库内调用方（`sdk/mobile/rust`、`kim-client-demo`、`tests.rs`）同 PR 更新。
3. `sdk/mobile` Riverpod 状态形状变更：`inboxProvider` 单巨 state 拆为 `threadsProvider` + `threadMessagesProvider.family`。测试同 PR 迁移。
4. FFI `KimApi` 方法返回值从格式化 `String` 改为结构体（`frb_generated.rs` 重新生成）。

G-13 稳定设备凭证（`target_id` 从 jti 迁 device_id）**不在本文件**，仍按 next-stage PR 4 合同后置。本文件的 sync 循环不假设 `target_id` 格式。

---

## Feasibility Assessment

服务端能力已齐：Chat 长连接已挂 `chat.inbox.list` / `chat.inbox.read` / `chat.history` / `chat.offline.index` / `chat.offline.content`（`services/chat/src/lib.rs` 314–437 行），proto（`InboxItem` / `HistoryItem` / `MessageIndex` / `Message`）齐全，Royal HTTP 同能力仅供 Web。`kim-client` 已有 `encode_dest_cmd` + `write_wait(seq)` 的请求响应通道与登录后 reader/writer pump（`pump.rs`），加命令是增量编码工作，无需服务端改动。Flutter 侧现状是「无分页、无定位、全量 diff」的初级实现，替换面收敛在 `sdk/mobile` 一个目录；`conversation_store.dart` 已有 SQLite 骨架可增量改造。flutter_chat_ui 移除后无其他代码引用。**Fully feasible。**

---

## Current Surface Inventory

### Rust（crates/kim-client + sdk/mobile/rust）

| 路径 | 现状 |
|---|---|
| `crates/kim-client/src/client.rs` | `talk_to_user` 每次新 `Uuid` 作 clientId（G-14）；无 inbox/history/offline 封装；`Io` 状态机 Off→Handshake→Live，无自动重连 |
| `crates/kim-client/src/wire.rs` | `encode_ack` 单 id；无 `encode_inbox_list` / `encode_history` / `encode_offline_*` |
| `crates/kim-client/src/events.rs` | `Event` 无 inbox/history/offline 响应变体、无连接状态事件 |
| `crates/kim-client/src/pump.rs` | reader/writer 分离已就绪；reader 错误只打 `Event::Closed` |
| `sdk/mobile/rust/src/api/client.rs` | 全部 `rt().block_on` + `Result<String, String>`；`talk_to_user` 返回格式化字符串，message_id 拿不回 |
| `sdk/mobile/rust/src/api/auth.rs` | AuthClient 包装，不动 |

### Dart（sdk/mobile/lib）

| 路径 | 现状 |
|---|---|
| `kim_bridge.dart` | `KimClientPort` 抽象（测试可注入）；`KimBridge` 实现；返回值无结构 |
| `state/gateway.dart` | `AsyncNotifier<ConnStatus>`，失败靠 Riverpod retry 重 build |
| `state/live.dart` | `liveEventsProvider` 消费 push；`sessionLinkProvider` 每 8s `ping().timeout(3s)` 轮询死链 |
| `state/inbox.dart` | 单 state 持全部会话消息 map；`_append` 全量 copy；`receive` 无 message_id 去重（`event.messageId == 0` 时退化为 uuid） |
| `state/providers.dart` | 端口注入 overrides |
| `data/conversation_store.dart` | `saveThreads` DELETE 全表重插；`saveMessages` 同；`loadMessages` 一次 400 条无分页 |
| `screens/chat/chat_page.dart` | flutter_chat_ui `Chat` + `InMemoryChatController`；`_sync` 每次 ref.listen 全量 diff；无分页 / 定位 / 键盘补偿 / 新消息浮条 |
| `widgets/kim_composer.dart` | WeChat 式 composer，键盘仅靠 Scaffold resize |
| `widgets/kim_bubble.dart` | Discord 行 + 逐条时间戳；图片尺寸 clamp 写死 |
| `theme/kim_theme.dart` | flex_color_scheme M3；chat 主题转接层 |
| `main.dart` | await 完整 bootstrap（含通知权限弹窗）才 runApp |
| `test/**` | FakeKim + harness 已就绪，随状态形状迁移 |

### 不改

- 服务端（`services/*`）、`crates/kim-protocol` proto、`sdk/web`、`deploy`。
- `flutter_chat_core` 之外 Dart 依赖不新增大件；新增依赖仅 `visibility_detector`（可选，浮条用）——不引入则用 ScrollNotification 手写。

---

## Design

### 1. 总体分层

```
┌─ Flutter UI（壳）────────────────────────────┐
│ ChatPage / ChatList(自研) / Composer / Theme │
├─ Riverpod 状态（薄）─────────────────────────┤
│ threadsProvider / threadMessages.family      │
│ outboxProvider / linkProvider(镜像 Rust 状态)│
├─ Dart 数据层 ────────────────────────────────┤
│ ConversationStore(SQLite, 增量, 分页)         │
├─ FFI(frb 2.13, 结构化) ──────────────────────┤
│ KimApi: startSession/stopSession/history/... │
├─ Rust kim-client（核心逻辑）─────────────────┤
│ SessionSupervisor: 重连/退避/状态广播         │
│ SyncEngine: inbox→offline index/content 循环 │
│   拉取→emit→等 Dart 确认落盘→ACK→翻页        │
│ KimClient: +inbox/history/offline 命令        │
└──────────────────────────────────────────────┘
```

原则（与 README「TDLib 形」一致）：游标、重试、退避、ACK 时序全在 Rust;Dart 只消费事件流 + 读写本地库。

### 2. 关键设计决策

1. **同步走长连接命令，不走 Royal HTTP。** Web 走 HTTP 是历史路径；mobile 复用已登录的 WSS 通道（Chat 已挂全部命令），省一条 HTTP 栈与鉴权面。拒绝项：mobile 直连 Royal `/api/v1/*`——绕过网关会话语义且要多管一份 token 刷新。
2. **落盘确认制（persist-then-ACK）。** Rust 每拉一页 content，emit 给 Dart 后暂停循环，Dart 写 SQLite 成功后调 `sync_confirm(last_message_id)`，Rust 才发 ACK 并翻页。崩溃窗口内消息因未 ACK 会被下次登录重拉，幂等由 message_id 去重兜底。拒绝项：Rust 拿到即 ACK——崩溃即永久丢消息；全量本地库搬 Rust——本阶段不做（见 Architectural Notes）。
3. **重连监督在 Rust。** `sessionLinkProvider` 的 8s ping 轮询删除。`SessionSupervisor` 以 pump reader/writer 错误为断链信号（秒级），指数退避（1s→2s→…→60s 封顶，网络恢复事件立即重试）重连 + 重新 login + 重跑 SyncEngine。Dart 的 `gatewayProvider` 退化为 Rust 状态镜像 + 手动重试入口。
4. **Dart 发送 outbox。** 发送乐观上行入队（sending）→ 落 SQLite → 上行；离线时允许入队（好友门禁仍在线校验），上线后重放，`client_id` 跨重试稳定（G-14）。拒绝项：outbox 放 Rust——本地 UI 状态（failed/retry 按钮）与 Rust 队列对账复杂度更高；SQLite 已在 Dart 侧。
5. **混合内容 = 多条消息批量，不改协议。** wire `MessageReq` 是单内容模型（type=1 文本/2 图片/3 语音/4 视频），一条图文混合输入拆成 N 条独立消息顺序发送（序号稳定：入队顺序即发送顺序），UI 层同一批次的相邻消息用现有 5 分钟分组规则渲染成连续块（视觉上接近“图文一条”）。拒绝项 A：新协议类型 `type=5` 复合消息（body 装富文本 JSON）——服务端 filter/落库/历史/离线索引全链路都要改，且 Web 端无法渲染旧格式，代价不成比例；拒绝项 B：Telegram album 媒体组——需要 album_id 协议字段与服务端分组聚合，同样超出本阶段。多消息方案崩溃窗口内只重放未成功的子项，每条独立幂等（client_id）。
5. **自研 ChatList：reverse CustomScrollView。** reverse 布局天然把「底部 = offset 0」做成稳定锚点：新消息插入、键盘弹出（viewport 缩短）都不动底部锚。向上翻页加载更早历史时用「锚定可见项 + offset 补偿」。拒绝项：继续定制 flutter_chat_ui——键盘补偿、jump-to-message、锚定分页都要绕过它的私有 controller。
6. **消息模型对账以 message_id 为准。** 本地 uuid 只在服务端 message_id 未返回前充当临时 key；发送响应带回 `message_id` 后 patch 本地行。推送 / 离线拉取 / 历史翻页三条入流共用「按 message_id upsert」一条路径。
7. **UI：Telegram 视觉 token × Discord 单列布局。** 设计 token（色板、字阶、圆角、间距、动效曲线时长）统一在 `KimTheme`；消息区保持单列、作者分组、头像左置的 Discord 结构，自己的消息用主色气泡 + 小 tail，对方扁平卡片气泡；时间戳收敛到分组头与 hover 态，不再逐条。混合发送批次在 UI 上用「同批次无间距 + 首尾气泡差异化圆角」表达为视觉一组，但仍是独立消息行（可单独长按/失败/重试）。
8. **启动即渲染。** `main()` 只做 `runApp`（splash theme 占位），bootstrap（路径/设置/SQLite/FFI）进 Riverpod 异步 provider；通知权限延后到首次进会话列表后请求。

### 3. Rust 接口（crates/kim-client）

```rust
// src/events.rs 追加
pub enum Event {
    // ... 现有 ...
    Inbox { sequence: u32, items: Vec<InboxItem> },
    History { sequence: u32, dest: String, messages: Vec<HistoryItem> },
    OfflinePage { indexes: Vec<MessageIndex> },
}

pub struct InboxItem { /* proto InboxItem 字段直译 */ }
pub struct HistoryItem { message_id: i64, msg_type: i32, body: String, extra: String,
    sender: String, send_time: i64, direction: i32 }

/// 发送内容。wire 是单内容 MessageReq（type=1 文本/2 图片/3 语音/4 视频），
/// 一条 OutgoingContent 映射一条 wire 消息；混合输入由调用方拆成多条。
pub enum OutgoingContent {
    Text(String),
    Image { url: String, extra: String },   // extra: {"w":n,"h":n}
    Voice { url: String, extra: String },
    Video { url: String, extra: String },
}

// src/client.rs 追加（全部走 write_wait，同现有模式）
impl KimClient {
    pub async fn inbox_list(&self, limit: i32) -> Result<Vec<InboxItem>, ClientError>;
    pub async fn history(&self, dest: &str, kind: i32, before_id: i64, limit: i32)
        -> Result<Vec<HistoryItem>, ClientError>;
    pub async fn offline_index(&self) -> Result<Vec<MessageIndex>, ClientError>;
    pub async fn offline_content(&self, ids: &[i64]) -> Result<Vec<Message>, ClientError>;

    /// 统一发送入口。替代 talk_to_user / talk_image（服务端指令名不进客户端 API）。
    /// client_id 由调用方持有（G-14 重试幂等键）；1:1 与群聊都是它，
    /// kind 决定服务端 CMD_CHAT_USER_TALK vs CMD_CHAT_GROUP_TALK。
    pub async fn send_message(&self, dest: &str, kind: MessageKind,
        content: OutgoingContent, client_id: &str) -> Result<TalkResult, ClientError>;
}
```

```rust
// src/supervisor.rs 新建
pub enum LinkState { Connecting, Online, Reconnecting { attempt: u32 }, Offline }

pub enum SessionEvent {
    Link(LinkState),
    Inbox(Vec<InboxItem>),
    Talk(IncomingTalk),          // push 与离线补拉统一形状
    SyncProgress { pulled: usize, page_pending: bool },
    SyncDone { pulled: usize },
    SyncFailed(ClientError),
    Kickout { channel_id: String },
    TokenRenew { token: String, exp: i64 },
    FriendRequest { from: String, nickname: String },
    GroupCreate { group_id: String, members: Vec<String> },
}

pub struct SessionSupervisor { /* handle */ }

impl SessionSupervisor {
    /// start = 循环 { connect → login → sync → 伺服 }，退避重连。
    /// stop = 断链并退出循环（登出走它）。
    pub fn start(config: ClientConfig) -> Self;
    pub fn stop(&self);
    /// 事件广播 + 当前 LinkState 快照。
    pub fn events(&self) -> broadcast::Receiver<SessionEvent>;
    pub fn state(&self) -> LinkState;
    /// SyncEngine 页确认：Dart 已把 ≤ cursor 的页落 SQLite。
    pub fn sync_confirm(&self, cursor: i64);
    /// 网络恢复（connectivity 回调）触发立即重试，清退避。
    pub fn notify_radio_up(&self);
}
```

SyncEngine（`src/sync.rs`，被 supervisor 驱动）时序：

```
inbox.list(200)                     → emit Inbox（Dart 重建/合并 threads）
loop {
  idx = offline.index()             → 空/短页即止（兼容今日 send_time 高水位
                                       与 Slice 5 pending_delivery 两种语义）
  ids = idx.message_ids 截 200
  msgs = offline.content(ids)       → emit Talk(...)（含 direction/group 归并）
  emit SyncProgress{page_pending}   → 等待 sync_confirm(max_id)
  ack(max_id)（今日单 id；服务端 batch 落地后改 message_ids）
}
emit SyncDone
```

### 4. FFI（sdk/mobile/rust/src/api/client.rs）

```rust
pub struct KimTalkResult { pub message_id: i64, pub send_time: i64 }
pub struct KimOutgoingContent { /* frb 枚举桥接：Text(String) / Image{url,extra} / ... */ }

pub struct KimApi { /* supervisor 句柄 */ }
impl KimApi {
    pub fn start(url: String, token: String, user_agent: String) -> Self;
    pub fn stop(&self);
    #[frb(sync)] pub fn link_state(&self) -> String;          // Connecting/Online/...
    pub fn session_events(&self, sink: StreamSink<KimSessionEvent>) -> Result<(), String>;
    pub fn sync_confirm(&self, cursor: i64) -> Result<(), String>;
    pub fn notify_radio_up(&self) -> Result<(), String>;
    pub fn send_message(&self, dest: String, kind: i32, content: KimOutgoingContent,
        client_id: String) -> Result<KimTalkResult, String>;
    pub fn history(&self, dest: String, kind: i32, before_id: i64, limit: i32)
        -> Result<Vec<KimHistoryItem>, String>;
    pub fn inbox(&self, limit: i32) -> Result<Vec<KimInboxItem>, String>;
    // friend/profile/search 维持现状（走 supervisor 内的 KimClient 句柄）
}
```

`rt().block_on` 保留给一次性查询；supervisor 自持 tokio task，Dart 不再驱动连接生命周期。`flutter_rust_bridge_codegen generate` 重新生成 `lib/src/rust/**`。

### 5. Dart 端口与状态

```dart
// kim_bridge.dart
abstract class KimClientPort {
  Stream<KimSessionEvent> sessionEvents();
  KimLinkState linkState();
  Future<void> startSession(String url, String token, {required String userAgent});
  Future<void> stopSession();
  Future<void> syncConfirm(int cursor);
  Future<void> notifyRadioUp();

  /// 统一发送入口。多次调用即多条消息（混合输入在上层拆分，见 outbox）。
  Future<KimTalkResult> sendMessage(String dest, ThreadKind kind,
      KimOutgoingContent content, {required String clientId});
  Future<List<KimHistoryMsg>> history(String dest, ThreadKind kind,
      {int beforeId = 0, int limit = 50});
  Future<List<KimInboxThread>> inboxList({int limit = 200});
  // friends / profile / search / logout 不变
}

sealed class KimOutgoingContent {
  const KimOutgoingContent.text(String text);
  const KimOutgoingContent.image({required String url, required int width, required int height});
  const KimOutgoingContent.video({required String url});
}
```

```dart
// state/link.dart（替代 gateway.dart + live.dart 的 sessionLinkProvider）
final linkProvider = NotifierProvider<LinkNotifier, KimLinkState>(...);
// 消费 KimClientPort.sessionEvents()：Link→state；Inbox→threadsProvider 合并；
// Talk→threadMessages(dest).receive() + ConversationStore.upsert；
// SyncProgress(page_pending)→落盘完成后 port.syncConfirm(cursor)；
// Kickout→auth.signOut()；TokenRenew→auth.savePushedToken。

// state/messages.dart
final threadMessagesProvider =
    NotifierProvider.family<ThreadMessagesNotifier, ThreadMessagesState, String>(...);

class ThreadMessagesState {
  final List<KimChatMsg> items;      // 只持当前线程
  final bool loadingOlder; final bool hasMore;
  final String? unreadAnchorId;      // 首次进入时未读分界
}
```

```dart
// data/conversation_store.dart（增量）
Future<void> upsertThread(String account, KimThread t);       // INSERT OR REPLACE
Future<void> upsertMessages(String account, String dest,
    Iterable<KimChatMsg> msgs);                                // ON CONFLICT(key) DO UPDATE
Future<void> markThreadRead(String account, String dest);      // unread=0
Future<List<KimChatMsg>> loadMessagesPage(String account, String dest,
    {int? beforeAt, int limit = 50});                           // WHERE at < beforeAt ORDER BY at DESC LIMIT
```

`KimChatMsg` 增加 `messageId`（int，服务端 id；`key` 保留为本地稳定 key，messageId 到位后与 key 建映射）。

### 6. 自研 ChatList 与聊天页

```dart
// widgets/chat/chat_list.dart
class ChatListController {
  void jumpToMessage(String key, {double alignment = 0.5}); // 定位（引用跳转/搜索）
  bool get atBottomEdge;                                    // 反向列表 offset 阈值
  Future<void> scrollToBottom({required bool animated});
}

class ChatList extends StatefulWidget { /* reverse CustomScrollView */ }
```

行为规格：

- **reverse + 键控**：`CustomScrollView(reverse: true)` + `SliverList`，item 以为 `KimChatMsg.key` 生成的 `GlobalKey`/索引锚。底部（offset 0）即最新消息，新消息 / 键盘弹出（viewport 缩短）不动锚点。
- **键盘**：`Scaffold.resizeToAvoidBottomInset: true`；composer 聚焦时仅当 `atBottomEdge` 才动画跟底；用户在历史区时可见项锚定（viewport 变化前记录首可见项 offset，变化后补偿），不顶飞。
- **向上分页**：滚至距顶（时间上的最早端）阈值内触发 `loadOlder()` → `history(beforeId: oldest)` → store upsert → prepend；用 `SliverLayoutBuilder` 记录锚项 `attachment` 补偿 offset，避免跳动。
- **新消息浮条**：不在底部时来新消息 → 右下角 `↓ N 条新消息` 胶囊（点击 `scrollToBottom` 并清零）；在底部则直接跟随。
- **未读分界**：进入会话时若 `unreadAnchorId != null`，在其上方插「以下未读」分隔行。
- **长按菜单**：复制 / 引用（copy 到 composer 前缀）/ 重试（failed 行），沿用 WoltModalSheet。
- **发送状态**：sending=时钟 icon、sent=对钩、failed=红色感叹 + 点按重试；image/video 行上传后 body 换 URL 再上行（保留现逻辑，接入 outbox）。

### 7. UI 体系（Telegram token × Discord 布局）

`KimTheme` v2 token：

```dart
abstract final class KimTheme {
  // 字阶（Telegram 式信息密度）
  static const double fontTitle = 17, fontBody = 15.5, fontMeta = 12.5;
  // 圆角 / 间距
  static const double radiusBubble = 14, radiusBubbleTail = 6;
  static const double spaceUnit = 4;                       // 4pt 栅格
  // 动效（emphasized spring，进出场 240–320ms）
  static const Duration motionFast = Duration(milliseconds: 180);
  static const Duration motionBase = Duration(milliseconds: 260);
  static const Curve motionEmphasized = Curves.easeOutCubic;
  // 聊天画布：浅色 #F2F5F8 / 深色 #0E1621；自方气泡主色渐变（teal 500→600）
}
```

- 消息行（Discord 单列）：头像 36px 左置，作者名 + 分组时间在组头（同作者 5 分钟内合并），自方消息整行右对齐主色气泡 + 左下 tail；对方扁平 surfaceContainer 气泡。时间戳改为组头与长按详情，不逐条。
- 会话列表 tile：60px 高、头像 48px、未读圆点改 Telegram 式纯色胶囊；进入动画从逐条 slide 改整体 fade（去 stagger demo 感）。
- Composer v2（Telegram 式）：胶囊输入框、`发送/加号` 按钮交叉渐变 + 按压缩放，面板（相册/拍摄）从底部 spring 滑入。
- 顶栏：会话页大标题 + 在线状态副行（Telegram 风双行 AppBar），返回按钮圆形毛玻璃底。
- 全局：M3 语义 token 不变（flex_color_scheme 保留），在其上叠 KIM 产品层 token；暗色模式全套对齐。

### 8. 启动与预加载

- `main()`：`runApp` 立即渲染 splash（theme 已静态可用）；`bootstrapProvider`（AsyncNotifier）完成路径/设置/SQLite/FFI init 后解锁 router。
- 通知权限从 bootstrap 挪到「首次进入会话列表 + 登录成功」后一次性请求（`KimPermissions.requestNotificationsOnce` 逻辑保留，触发点后移）。
- 会话列表首屏：本地 threads 立即渲染（Skeletonizer 骨架仅在无本地数据时），`inbox.list` 到达后合并。
- 聊天页预加载：打开会话即取本地最新 50 条渲染，后台 `history(beforeId:0)` 对账合并 + 未读补拉。

---

## Phased Implementation

每个 Phase 独立可编译可测；Rust 侧先于 Dart 消费方落地。

## Phase 1: kim-client 协议命令 + 统一 send_message

- **File: `crates/kim-client/src/wire.rs`**
  - 追加 `encode_inbox_list(seq, limit)`（空 body `InboxReq`）、`encode_history(seq, dest, kind, before_id, limit)`（dest 走 `pkt.set_dest`，同 `encode_dest_cmd` 模式）、`encode_offline_index(seq)`（`MessageIndexReq{message_id:0}`——游标语义在服务端）、`encode_offline_content(seq, ids)`（`MessageContentReq`，account/app 留空由 handler 覆写）。
  - `encode_user_talk` / `encode_user_image` 收敛为 `encode_outgoing(seq, dest, kind, content, client_id)`：kind（user/group）选 CMD，`OutgoingContent` 映射 `MessageReq{type, body, extra}`。现有两个函数保留为薄包装或直接内联删除。
- **File: `crates/kim-client/src/events.rs`**
  - 追加 `Inbox` / `History` / `OfflinePage` 事件与 `InboxItem` / `HistoryItem` / `OutgoingContent` 结构；`wire.rs::decode_event` 对应命令分支。
- **File: `crates/kim-client/src/client.rs`**
  - `talk_to_user` / `talk_image` 删除，新增 `send_message(dest, kind, content, client_id)`；群聊消息同一入口（kind 参数选 CMD_CHAT_USER_TALK / CMD_CHAT_GROUP_TALK，当前 mobile 无群发 UI，仅打通）。
  - 新增 `inbox_list` / `history` / `offline_index` / `offline_content` 四方法，模式照抄 `friend_list`（`encode_*` + `write_wait` 序列匹配）。
- **File: `crates/kim-client/src/lib.rs`** — 导出新符号；`MessageKind` 复用 `kim_protocol` 的 `INBOX_KIND_USER/GROUP` 常量（避免新类型）。
- **File: `crates/kim-client/src/tests.rs` + `examples/kim-client-demo/`** — 内存 Conn 下测试四命令编解码与响应匹配；demo 调用点改 `send_message`。
- 验证：`cargo test -p kim-client && cargo clippy -p kim-client`。

## Phase 2: SessionSupervisor + SyncEngine

- **File: `crates/kim-client/src/supervisor.rs`（新建）**
  - `LinkState` / `SessionEvent` / `SessionSupervisor`（见 Design §3）。start 持有 tokio task：`connect → login → sync loop → recv 伺服`；reader/writer 错误或 `Event::Closed` 触发退避重连（1s 起步 ×2，60s 封顶；`notify_radio_up` 清零立即试）；重连成功后重跑 sync。
- **File: `crates/kim-client/src/sync.rs`（新建）**
  - SyncEngine：`inbox.list` → emit；`offline.index/content` 分页循环（页大小 200）；每页 emit 后 `sync_confirm` 闸门放行再 `ack`；`message_id` 去重表防 index/content 与 push 三流重复。
- **File: `crates/kim-client/src/lib.rs`** — 导出。
- **File: `crates/kim-client/src/tests.rs`** — 假服务器脚本化测试：断链重连次数、退避不爆炸、confirm 闸门未开不 ack、重复 message_id 只 emit 一次。
- 验证：`cargo test -p kim-client`。

## Phase 3: FFI 结构化 + frb 重生成

- **File: `sdk/mobile/rust/src/api/client.rs`**
  - `KimApi` 改持 `SessionSupervisor`；方法签名见 Design §4（`start/stop/link_state/session_events/sync_confirm/notify_radio_up/send_message/history/inbox`）；`KimPush` 流由 `KimSessionEvent` 取代；friend/profile 系列改走 supervisor 内 `KimClient`。`talk_to_user` / `talk_image` FFI 方法删除。
- **File: `sdk/mobile/rust/src/api/simple.rs` / `frb_generated.rs`** — `flutter_rust_bridge_codegen generate` 重生成。
- 验证：`cargo build -p kim_mobile_ffi`（crate 名以 `sdk/mobile/rust/Cargo.toml` 为准）+ 模板 app 冒烟。

## Phase 4: Dart 数据层增量改造

- **File: `sdk/mobile/lib/models/models.dart`** — `KimChatMsg` 加 `messageId` 与 `batchId`（可空，混合发送分组用）；`KimThread` 加 `avatar`（inbox.list 带回）。
- **File: `sdk/mobile/lib/data/conversation_store.dart`**
  - `upsertThread` / `upsertMessages`（`INSERT ... ON CONFLICT DO UPDATE`）/ `markThreadRead` / `loadMessagesPage`；`saveThreads`/`saveMessages` 全量写法保留但只用于 prefs 迁移路径；messages 表加 `message_id` 列 + 索引（`ALTER TABLE` 兼容旧库，message_id 允许 0）。
- **File: `sdk/mobile/test/conversation_store_test.dart`** — 增量语义与分页测试。
- 验证：`flutter test`。

## Phase 5: Riverpod 状态重排 + outbox

- **File: `sdk/mobile/lib/kim_bridge.dart`** — `KimClientPort` 扩成 Design §5 形状；`KimBridge` 对接新 FFI。
- **File: `sdk/mobile/lib/state/link.dart`（新建，替换 `gateway.dart` + `live.dart` 的 sessionLinkProvider）** — `linkProvider` 消费 `sessionEvents()`；Inbox 合并、Talk 分发、confirm 闸门、kick/token/friend 处理迁入；删除 8s ping 轮询。
- **File: `sdk/mobile/lib/state/messages.dart`（新建）** — `threadMessagesProvider.family`：`receive`（push/补拉统一 upsert）、`loadOlder`（history 翻页 + store 合并）、`markRead`。
- **File: `sdk/mobile/lib/state/inbox.dart`** — 收敛为 threads 语义（`threadsProvider`），删除 messages map 持有。
- **File: `sdk/mobile/lib/state/outbox.dart`（新建）** — 入队即落库（sending）→ 上行（稳定 clientId）→ sent/failed；`linkProvider` 转 online 时重放 sending/failed 队列。**混合发送**：`enqueueBatch(dest, List<KimOutgoingContent>)` 按顺序逐条入队（每条独立 client_id 与行状态），发送层串行处理保持顺序；UI 用批次 id 分组渲染（同批次相邻行去间距）。部分失败只重放失败子项。
- **File: `sdk/mobile/lib/state/providers.dart` / `session.dart` / `app.dart` / `main.dart`** — 接线更新；bootstrap 异步化（splash 先行，通知权限后移）。
- **File: `sdk/mobile/test/support/fake_kim.dart` / 相关 `state/*_test.dart`** — FakeKim 实现新端口；测试迁移。
- 验证：`flutter analyze && flutter test`。

## Phase 6: 自研 ChatList + 聊天页重写

- **File: `sdk/mobile/lib/widgets/chat/chat_list.dart`（新建）** — Design §6 全部行为。
- **File: `sdk/mobile/lib/screens/chat/chat_page.dart`** — 重写：接 `threadMessagesProvider`；`ChatListController` 定位 / 浮条 / 未读分界 / 长按菜单；删除 `InMemoryChatController` 与 `_sync` 全量 diff。
- **File: `sdk/mobile/lib/widgets/kim_bubble.dart`** — 重写为 v2 消息行（组头时间、气泡 tail、发送态 icon）；`retry` 接 outbox。
- **File: `sdk/mobile/pubspec.yaml`** — 移除 `flutter_chat_core` / `flutter_chat_ui`。
- **File: `sdk/mobile/test/kim_page_test.dart` 等 widget 测试** — 迁移到新列表。
- 验证：`flutter analyze && flutter test`。

## Phase 7: UI 体系落地

- **File: `sdk/mobile/lib/theme/kim_theme.dart`** — token v2（Design §7）；删除 flutter_chat_core 主题转接。
- **File: `sdk/mobile/lib/widgets/conversation_tile.dart`** — 新 tile（去 stagger animate）。
- **File: `sdk/mobile/lib/widgets/kim_composer.dart`** — Composer v2。
- **File: `sdk/mobile/lib/screens/**` / `widgets/empty_state.dart` / `widgets/new_chat_sheet.dart` 等** — 按 token 统一（间距、字号、动效时长曲线、暗色）。
- 验证：`flutter analyze && flutter test` + 真机走查清单（浅/深、键盘、动效）。

## Phase 8: 全量验证与文档回写

- `cargo test -p kim-client && cargo clippy --workspace`
- `cd sdk/mobile && flutter analyze && flutter test`
- 手工：本机 Royal/Chat/gateway 起 stack，双端互聊 + 断网 30s 恢复（补洞）+ kill 重启（重拉）+ 离线发送重放。
- **File: `docs/mobile-client.md`** — 回写新架构（supervisor/sync/outbox/ChatList）；**File: `docs/impl/README.md`** — 记录切片合入。

---

## Architectural Notes

- **Semver**：`kim-client` 为 workspace 内部 crate，`talk_*` 加参不发布外部；FFI 由 frb 重新生成，Dart 侧同 PR 对齐。
- **不改服务端**：sync 全走既有长连接命令。`offline.index` 在 Chat `KIM_PENDING_RECEIPT=0` 时仍是高水位，`=1` 时是 pending receipt；对客户端都是「拉到空页即止」。新循环应设 `resume=true` 并 batch ACK `messageIds`（≤200）。
- **G-14 收口、G-13 不动**：`client_id` 稳定由 Dart outbox 持有；device credential 仍按 next-stage PR 4 后置，本文件 sync/ack 不假设 target_id 格式。
- **本地库留 Dart 的边界**：本阶段 Rust 只管「网络 + 时序」，SQLite 仍在 Dart。若后续要完全 TDLib 形（库也进 Rust），`conversation_store` 的接口（upsert/page/read）即迁移面，已按可搬移形状设计。
- **副作用分析**：sync 循环在网络恢复风暴下有退避封顶；`sync_confirm` 闸门防 ACK 超前于落盘；message_id 去重防三流（push/补拉/翻页）重放。
- **Riverpod 3 注意**：`linkProvider` 必须由根（`KimApp`）watch——IndexedStack 标签页 TickerMode 会暂停监听（现有注释同样约束，保留）。
- **明确不做**：消息撤回 / 已读回执 / 输入中状态（协议未定）、群头像九宫格、表情面板、桌面端布局。协议层复合消息（type=5 富文本）与 Telegram 式 album（album_id + 服务端聚合）评估后拒绝：全链路改动不成比例，多消息批次方案已覆盖主要体验；若未来产品要求严格单条图文，另开协议切片。

## File Change Summary

- `crates/kim-client/src/client.rs` — talk_* 删除，新增统一 send_message(dest, kind, content, client_id)；新增 inbox/history/offline 四方法
- `crates/kim-client/src/events.rs` — Inbox/History/OfflinePage 事件与结构；OutgoingContent 发送内容枚举
- `crates/kim-client/src/lib.rs` — 导出
- `crates/kim-client/src/supervisor.rs` — 新建：SessionSupervisor + LinkState + SessionEvent
- `crates/kim-client/src/sync.rs` — 新建：SyncEngine（persist-then-ack 分页循环）
- `crates/kim-client/src/tests.rs` — 命令 / 重连 / 闸门 / 去重测试
- `crates/kim-client/src/wire.rs` — 新命令编码与解码
- `examples/kim-client-demo/` — client_id 调用点
- `sdk/mobile/lib/app.dart` — 根 watch linkProvider；splash
- `sdk/mobile/lib/data/conversation_store.dart` — 增量 upsert / 分页 / message_id 列
- `sdk/mobile/lib/kim_bridge.dart` — KimClientPort v2（结构化 + session 事件 + sendMessage 统一入口）
- `sdk/mobile/lib/main.dart` — runApp 先行、bootstrap 异步
- `sdk/mobile/lib/models/models.dart` — messageId / batchId / thread avatar
- `sdk/mobile/lib/screens/chat/chat_page.dart` — 重写（ChatList + 定位 + 浮条 + 未读分界）
- `sdk/mobile/lib/screens/home/chats_page.dart` — tile/骨架/合并接线
- `sdk/mobile/lib/state/gateway.dart` — 删除（并入 link.dart）
- `sdk/mobile/lib/state/inbox.dart` — 收敛为 threadsProvider
- `sdk/mobile/lib/state/link.dart` — 新建：连接镜像 + 事件分发 + confirm 闸门
- `sdk/mobile/lib/state/messages.dart` — 新建：threadMessagesProvider.family
- `sdk/mobile/lib/state/outbox.dart` — 新建：发送队列、混合批次拆分与重放
- `sdk/mobile/lib/state/providers.dart` / `session.dart` / `live.dart` — 接线（live 并入 link）
- `sdk/mobile/lib/theme/kim_theme.dart` — token v2，删 chat_ui 转接
- `sdk/mobile/lib/widgets/chat/chat_list.dart` — 新建：reverse 列表 / 键盘补偿 / 分页 / jumpTo
- `sdk/mobile/lib/widgets/conversation_tile.dart` — 新 tile
- `sdk/mobile/lib/widgets/kim_bubble.dart` — 消息行 v2（组头时间、气泡、发送态）
- `sdk/mobile/lib/widgets/kim_composer.dart` — Composer v2
- `sdk/mobile/pubspec.yaml` — 移除 flutter_chat_core / flutter_chat_ui
- `sdk/mobile/rust/src/api/client.rs` — KimApi v2（supervisor + 结构化结果 + send_message）
- `sdk/mobile/rust/src/frb_generated.rs` 等 — frb 重生成
- `sdk/mobile/test/**` — FakeKim / harness / widget 测试随迁
- `docs/mobile-client.md` — 架构回写
- `docs/impl/README.md` — 切片记录
