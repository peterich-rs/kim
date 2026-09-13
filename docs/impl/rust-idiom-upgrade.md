# Rust 惯用法升级：对照陈天课的代码审查与渐进式改造

| 字段 | 值 |
|---|---|
| 状态 | 审查完成；按五刀在 `refactor/rust-idiom-upgrade` 落地 |
| 基线 | `origin/main` `75aba92`（worktree `/Users/zhangfan/develop/github.com/im-rust-idioms`） |
| 对照 | `books/陈天 · Rust 编程第一课` 第 9–13、15–21、24–25、29–30、49–53 讲 |
| 不挡 | B0 pending-receipt rollout、ACK 模型、`sdk/web` / Flutter UI |

节奏（已拍板）：大盘点（本文）→ 一刀一切片细化 → 实现 → 对抗审查 → 下一刀。不和 G-03 / 产品缺口搅在一起。

## 结论

通信层已经是陈天课里的「原始但完整的结构」：`Conn` 换传输不改业务；`Channel` 用所有权保护插座（信箱 + 写专员）；`ChannelMap` 是 `DashMap<Arc<str>, Channel>`，查表 clone-out、锁不跨 await。

业务层仍是 Go 形状的 Rust：身份是 `&str`/`String`，指令是 40 个 `CMD_*` 字符串进 `HashMap`，错误有 `Other(String)` 逃逸口，handler 返回 `()`。类型系统和 `Result` 没有当架构工具用。

升级目标不是「写得更像教科书」，而是把身份、指令、错误从运行时约定提升为编译期合同。

## 不要重做（已经对）

| 落点 | 证据 | 对应小册 |
|---|---|---|
| crate 分层 | `docs/architecture.md`；`kim-tcp` 禁止写登录 | 第 24、53 讲分层 |
| `Conn` / `Acceptor` / `Naming` 桥接 | `crates/kim-core/src/conn.rs` | 第 30 讲 trait 桥接 |
| 写路径 mailbox | `Channel` + `WriteShared` | 第 9–13、39 讲所有权当并发 |
| `ChannelMap` DashMap | `crates/kim-core/src/channel_map.rs:18` | 热路径并发切片已合入 |
| `LogicPkt.body: Bytes` | `crates/kim-protocol/src/logic.rs:9-13` | 第 26 讲 / `bytes` 零拷贝 |
| 库 crate `thiserror` | `kim-core::Error`、`ProtocolError`、`TalkError` | 第 21 讲 |
| `unsafe_code = deny` | 根 `Cargo.toml:33` | 第 36 讲 / rust-strict |
| `FilterChain` | `services/chat/src/filter.rs:89-115` | 第 53 讲流水线雏形（仅内容审核） |
| postgres/redis feature | `kim-session`、`chat` store | 第 50 讲特性管理 |

## 审查发现（代码计数）

### 1. 身份全是字符串（第 15、30 讲）

21 个身份相关公开 trait、约 80 个方法把账号、连接、网关、目的地当普通字符串。对调参数编译器无法拒绝。

| Trait | 方法 | 位置 |
|---|---|---|
| `Acceptor` | `accept(...) -> Result<String, Error>` | `kim-core/src/conn.rs:34` |
| `StateListener` | `disconnect(&str)` | `kim-core/src/conn.rs:57` |
| `ChannelHandle` | `id() -> &str` | `kim-core/src/conn.rs:65` |
| `Server` | `push(&str, Bytes)` / `close_channel(&str)` | `kim-core/src/server.rs:20-22` |
| `SessionStorage` | `delete(&str,&str)` / `get(&str)` / `get_locations(&[String])` / `get_location(&str,&str)` | `kim-router/src/storage.rs:23-29` |
| `Dispatcher` | `push(gateway: &str, channels: &[String], ...)` | `kim-router/src/dispatcher.rs:25-30` |
| `Naming` | `find(service_name: &str, ...)` / `deregister(&str)` | `kim-naming/src/naming.rs:15-27` |
| `MessageStore` | 几乎每个方法都是 `app: &str, account: &str, dest: &str` | `services/chat/src/store/mod.rs:306-389` |

`Location` 四个字段全是 `String`（`kim-router/src/location.rs:6-11`）。protobuf `Session` 也是（`pkt.proto:62-74`）——线格式保持字符串没问题，Rust 侧没有包装。

`kim-sdk` 已有 `AccountId(pub String)` / `DestId(pub String)`（`crates/kim-sdk/src/ids.rs:4-7`）。字段公开、无校验。全仓 **零构造点**（只有定义和 `lib.rs` re-export；sdk 内部真正在用的是 `SessionEpoch`）。`kim-sdk` 已依赖 `kim-protocol`，类型源应该在 protocol 层，不要再做第二套。

账号规则写了两遍且未共享：Royal 与 `kim-client` 各自 `ACCOUNT_MIN=3` / `MAX=32` / ascii 字母数字下划线（`services/royal/src/auth.rs:19-42`、`crates/kim-client/src/auth.rs:23-370`）。

指令：40 个 `CMD_*` 字符串常量（`kim-protocol/src/wire.rs:13-52`）。`Router` 用 `HashMap<String, HandlerFn>` 运行时查找（`kim-router/src/router.rs:15-34`）。拼错只得到 `CommandNotFound`。小册 KV 是 protobuf `oneof` + `match`，新命令编译期必补。

`LaneKeyFn = Arc<dyn Fn(&[u8]) -> Option<String>>`（`kim-core/src/channel.rs:26`）。读循环再 `Arc::from(k)`（`channel.rs:315-323`）。`logic_channel_id` 已返回 `Option<String>`（`kim-protocol/src/lib.rs:85-90`）。每帧一次堆分配。

### 2. 错误处理停在能编译（第 21 讲）

字符串逃逸口：

| 类型 | 变体 | 位置 |
|---|---|---|
| `kim_core::Error` | `Other(String)` + `other()` | `kim-core/src/error.rs:41-48` |
| `SessionError` | `Other(String)` | `kim-router/src/storage.rs:11-12` |
| `RouterError` | `Dispatcher(String)` + `Other(String)` | `kim-router/src/dispatcher.rs:9-16` |
| `kim_naming::Error` | `Other(String)` | `kim-naming/src/naming.rs:9-10` |
| `StoreError` | `Backend(String)` + `Invalid(String)` | `chat/src/store/mod.rs:88-98` |
| `LookupError` | `Other(...)` | `services/router/src/lookup.rs:133` |

`Location::decode` 截断 / UTF-8 失败也走 `SessionError::Other`（`location.rs:85-93`），调用方无法 `match`。

8 个二进制入口全部 `Box<dyn std::error::Error>`：五个服务 `main` + `load_config` / `load_tls` / `open_*`，外加三个 examples。workspace 依赖**没有** `anyhow`（只有 `kim-agent-host` 单独用）。小册和仓库 rust-strict：库 `thiserror`、应用 `anyhow`。

`HmacNonceGuard::claim` 返回 `Result<bool, String>`；网关 `RevokeCheck` 也是 `Result<…, String>`。字符串错误从控制面一路渗到通信层。

Handler 吞掉 `Result`：

- `MessageListener::receive` 返回 `()`（`kim-core/src/conn.rs:51`）
- `HandlerFn` 返回 `Pin<Box<dyn Future<Output = ()>>>`（`kim-router/src/router.rs:15`）
- Chat **37** 个 `do_*` 全部返回 `()`；失败路径 `warn` + `return`（`talk.rs:46-67` 是典型）

Workspace clippy **没有** `unwrap_used = "deny"`。生产 `unwrap`/`expect` 很少：`kim-ws` HTTP builder 5 处（空 body 实际不会失败）、`kim-protocol/build.rs` 1 处。锁用 `unwrap_or_else(PoisonError::into_inner)`（可接受）。`Location::encode` 超长 id 截断到 `u16::MAX` 并打 error 日志（`location.rs:41-44`），没有变成错误类型。

`kim_core::Error::other` 约 50 处生产构造（container / tcp / ws / gateway handshake），kim-core 自己不用。`kim-metrics::Error` 只有 `Other(String)`，prometheus register 36 处 `e.to_string()`。

### 3. Trait 浅、动态分发过密（第 16、29、30 讲）

`#[async_trait]`：crates + services **约 195+** 处（生产约 90–110，其余测试 mock）。1.75 起 trait 可写原生 `async fn`；现在每次调用 `Box::pin`。

`Server` 把 setter 和运行时方法揉在一起（`server.rs:10-23`）：`set_acceptor` / `set_*` / `start` / `push` / `shutdown`。未 `start` 也能 `push`。ISP 弱。

`MessageStore` 14 个方法（insert 两种、ack、offline 两种、backfill、gc、stats、inbox、history、mark_read、bot 两种）。单测 mock 必须实现整张表（`talk.rs` 测试里一串 `StoreError::Backend("unused")`）。

Chat 热路径全是 `dyn`：`&dyn MessageStore`、`&dyn ContentFilter`、`&dyn UserDirectory`、`&dyn SocialDirectory`（`talk.rs:48-52`）。这些在进程启动时就定了，不是运行时才知道的 `Conn`。第 29 讲：单实现走泛型。

`Naming::Error` 只有 `Other(String)`，Consul 超时 / 404 / ACL 无法区分。

### 4. 所有权出了通信层就丢（第 9–13、18–20 讲）

`Channel.id` 已是 `Arc<str>`。一出通信层：

`Context::dispatch_cmd`（`kim-router/src/context.rs:110-132`）：

- `HashMap<String, Vec<String>>` 按 `gate_id.clone()` 聚合
- 每个接收者 `channel_id.clone()`
- 每个网关 `packet.clone()` —— `LogicPkt` derive Clone，body 是 `Bytes`（O(1)），但 Header 里 `command`/`channel_id`/`dest`/`meta` 仍是 `String` 深拷贝

`InsertMessage` 的 `sender`/`dest`/`body`/`extra`/`client_id` 全是 `String`（`store/mod.rs:108-117`），而帧 payload 已是 `Bytes`。

Lane 表是 `HashMap<Arc<str>, Sender>`（`channel.rs:160`），默认 SipHash。网关内部表不需要 DoS 抗性。

### 5. 生产卫生（第 50 讲）

| 要素 | 现状 |
|---|---|
| edition | 2021（MSRV 1.95 可上 2024） |
| `unwrap_used` | 未配置 |
| `missing_docs` | 未配置 |
| `cargo-deny` / `deny.toml` | 无 |
| crate README | `crates/*` 均无 |
| doctest | 公开 trait 几乎没有可运行样例 |
| `anyhow` | workspace 未引入 |
| pre-commit | 未在本审查范围强制 |

## 硬性规则（第 53 讲 Decisions）

1. 通信层以外，禁止用裸 `String` 表示 `AccountId` / `ChannelId` / `DestId` / `GatewayId`。
2. 库 crate 禁止把 `*Error::Other(String)` 当作稳定变体；外部错误用 `#[from]` / `#[source]`。
3. Handler 必须返回 `Result`。`Router::serve` 是唯一打日志的地方。
4. 启动时选定的依赖走泛型；只有运行时才知道的（`Conn`、`Naming` 后端）走 `dyn`。
5. 指令在边界解析成枚举；内部不用 command 字符串做控制流。线格式 `Header.command` 保持字符串。
6. 不改 protobuf 字段类型、不改 ACK 语义、不改 `sdk/web` / Flutter UI、不上 yamux / io_uring。

## 五刀（渐进式，每刀可编译可回滚）

### 第 1 刀 — 身份 newtype

**做什么**

- 在 `kim-protocol` 新增不透明 ID：`AccountId` / `ChannelId` / `DestId` / `GatewayId`，内部 `Arc<str>`。
- `AccountId::parse`：trim，3–32，ascii 字母数字下划线。失败 `ProtocolError::InvalidAccount`。Royal 与 `kim-client` 删重复的 `valid_account`。
- `ChannelId` / `GatewayId` / `DestId`：拒绝空串；不做字符集限制（channel 是网关生成的 `wg-1_alice_*`，dest 可以是账号或群 base36）。
- 不提供 `pub` 内部字段。`as_str()` / `as_arc()` 只读。从已校验内部值用 `from_trusted`（`pub(crate)` 或文档标明仅边界转换）。
- 改签名（本刀核心缝）：`SessionStorage`、`Dispatcher`、`Location`、`Server::push` / `close_channel`、`Acceptor::accept` 返回 `ChannelId`。
- protobuf `Session` 仍是 string；`add`/`get` 在存储边界 `parse` / `as_str`。
- `kim-sdk` 的浅 `AccountId(pub String)` 改为 re-export `kim_protocol` 类型（或包装 as_str 兼容 FFI）。**不改** Dart/FRB 对外字符串。

**不做什么**

- 不改 `MessageStore` 全部方法（第 3 刀 / 后续；否则爆炸）。Chat handler 内部可 `.as_str()` 暂接 store。
- 不改 command 字符串。
- 不改 Redis key 布局、Location 二进制编码布局（仍是 u16-LE 长度前缀字符串）。

**测试**

- `AccountId::parse`：过短、过长、非 ascii、合法。
- `Location` encode/decode 仍过现有 roundtrip；构造改用 newtype。
- `SessionStorage` 内存/Redis 实现编译 + 现有单测。
- `cargo test -p kim-protocol -p kim-router -p kim-session -p kim-core -p kim-tcp --offline` 以及 chat/gateway 会因签名变化必须一起改调用点。

**爆炸半径**：`kim-protocol`、`kim-router`、`kim-session`、`kim-core`、`kim-tcp`/`kim-ws`、`kim-container`、`services/{chat,gateway,tgateway}`、相关 tests。约 30–50 个 Rust 文件。

### 第 2 刀 — 错误收口

**做什么**

- 删除 `Error::Other`、`SessionError::Other`、`RouterError::Other`/`Dispatcher(String)`、`Naming::Error::Other` 作为稳定变体。换成有语义的变体（`LocationTruncated`、`ConsulHttp { status }`、`Redis(...)`、`FeatureDisabled`）。
- `StoreError::Backend(String)` 收成 `Sqlx` / `Redis` / `RoyalUnavailable` / `Timeout` / `IdempotencyExhausted` 等。
- workspace 加 `anyhow`。五个服务 `main` + `load_config` 改 `anyhow::Result`，`.context("gateway.toml")`。
- `HandlerFn` → `Result<(), RouterError>`。`MessageListener::receive` → `Result<(), Error>`；读循环对 Err 记 metric / 可关连接。
- `do_user_talk` 等改为返回 `Result`，由 Router 打一条日志。
- clippy `unwrap_used = "deny"`；`#[cfg(test)]` 允许。

**不做什么**

- 不改客户端错误字符串 ABI（`resp_with_error` 仍不把 Display 发给对端）。
- 不引入 `Box<dyn Error>` 的新调用点。

**测试**：现有 `matches!(err, SessionError::Other(_))` 改为新变体；chat e2e 仍过。

### 第 3 刀 — Command 枚举 + talk 流水线

**做什么**

- `kim-protocol::Command` 枚举，40 个 `CMD_*` 只出现在 `Command::parse(&Header) -> Result<Self, ProtocolError>`。
- `Router` 按枚举分发，或 `CommandHandler::execute`。未知 command 仍 `CommandNotFound`。
- 把 `do_user_talk` / `do_group_talk` 收成 Plug：`ParseDest → LoadUsers → SocialGuard → ContentFilter → Persist → Fanout → AckResp`。现有 `FilterChain` 成为其中一环。
- `MessageStore` 可按 ISP 拆 `TalkStore` / `InboxStore` / `BotStore`（若拆开会让 mock 变小再拆；否则留到本刀末尾评估）。

**不做什么**

- 不改线格式。
- 不用 `PlugResult::NewPipe`（talk fast path 固定）。
- 不把 pending receipt 语义改掉；receipt 是 Persist 之后的行为，由已有 store 开关控制。

**测试**：talk 单测按 Plug 切开；e2e 1:1 / 群聊保持。

### 第 4 刀 — 热路径所有权

**做什么**

- `LaneKeyFn` → `Fn(&[u8]) -> Option<Arc<str>>`；`logic_channel_id` 返回 `Option<Arc<str>>`（或 `Option<ChannelId>`）。
- `dispatch_cmd`：按 `GatewayId` 聚合 `ChannelId`（已是 `Arc<str>`），每个网关只 clone `Header` 必要字段 + `Bytes` body，禁止无意义的 `String` 键。
- `InsertMessage.body: Bytes`（或 `Arc<str>` 若必须 UTF-8 文本）；落 PG 时再 `&[u8]`。
- 内部表（`LaneSet`、`ChannelMap` 已是 dashmap、session loc 热表）评估 `ahash`/`FxHashMap`。对外解码仍默认 hasher。
- kimbench 对照；无数字不改算法。

**不做什么**

- reuseport / vectored write / jemalloc / io_uring（gaps 已延后）。
- 不改 `sdk/*`。

### 第 5 刀 — trait 现代化

**做什么**

- 核心 trait 改原生 `async fn`（`Conn`、`Acceptor`、`SessionStorage`、`Dispatcher`、`Naming`）。object-safe 的保留 `dyn`；不 object-safe 的走泛型。
- Chat 内部 `S: MessageStore` 单态，去掉 talk 路径上的 `dyn MessageStore`（`Conn` 仍 dyn）。
- `ServerBuilder` + `ServerHandle` typestate：未 `start` 不能 `push`。setter 只在 builder。
- edition 2024 独立小 PR（`unsafe extern`、RPIT `use<>`），不和业务混。
- `cargo-deny` + 公开 crate `missing_docs` warn。crate README 只给 `kim-core` / `kim-protocol` / `kim-router`。

**不做什么**

- 不把所有 `async_trait` 一次删光；测试 mock 可暂留。
- 不 `panic = "abort"`（网关单连接 panic 仍应 unwind）。

## 与生产缺口的关系

本文是架构债，不是 G-xx。不插到 B0 前面挡 rollout，也不改 ACK。第 3 刀的 Persist Plug 必须调用现有 `MessageStore::insert_*`，开关仍是 `KIM_PENDING_RECEIPT`。

合入后：形状写回 `architecture.md` / `communication-layer.md` / `control-layer-chat.md`；本目录保留本文作为总纲，每刀细化稿合入后可删（与 `docs/impl/README.md` 一致）。

## 验收

每刀独立：

1. `cargo fmt`
2. `cargo clippy --all-targets -- -D warnings`（第 2 刀起含 `unwrap_used`）
3. 受影响 crate `cargo test`
4. 对抗审查无未修复 bug 级问题

五刀全部完成后，下面这些编译期应当成立：

- 把 `AccountId` 传到 `Dispatcher::push` 的 gateway 参数会编译失败
- 新增 `CMD_*` 而不补 `Command` 枚举会编译失败
- handler 漏处理 `Result` 会 `must_use` / clippy
- `main` 不再出现 `Box<dyn Error>`
