# 第 1 刀：身份 newtype

父规格：[rust-idiom-upgrade.md](./rust-idiom-upgrade.md)。基线 `origin/main` `75aba92`。

本刀必须让 **整个 workspace 编译**。`MessageStore` / `UserDirectory` / command 字符串本刀不改；handler 从 session/location 取出 newtype 后 `.as_str()` 再进 store。

## Types API

新文件 `crates/kim-protocol/src/ids.rs`，从 `lib.rs` `pub use`。

```rust
use std::borrow::Borrow;
use std::fmt;
use std::sync::Arc;

use crate::ProtocolError;

pub const ACCOUNT_MIN: usize = 3;
pub const ACCOUNT_MAX: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AccountId(Arc<str>);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ChannelId(Arc<str>);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DestId(Arc<str>);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct GatewayId(Arc<str>);
```

规则：

| 类型 | `parse(&str)` | `from_trusted` |
|---|---|---|
| `AccountId` | trim；长度 3–32；每个 char 为 ascii 字母数字或 `_`；失败 `ProtocolError::InvalidAccount` | 仅内部已校验（JWT / 已 parse 的 session） |
| `ChannelId` | trim 后非空 | Location 解码、网关生成的 id（允许历史空串） |
| `GatewayId` | 同 ChannelId | 同 |
| `DestId` | trim 后非空 | talk dest 已非空 |

禁止：`pub` 内部字段、`Deref`、`From<String>` 自动转换（避免把任意字符串悄悄变成 ID）。

每个类型实现：`as_str() -> &str`、`as_arc() -> &Arc<str>`、`AsRef<str>`、`Borrow<str>`、`Display`、`From<AccountId> for String`（`as_str().to_owned()`，仅 Redis/proto 出站）。

`ProtocolError` 增加：

```rust
#[error("invalid channel id")]
InvalidChannelId,
#[error("invalid gateway id")]
InvalidGatewayId,
#[error("invalid dest")]
InvalidDest,
```

`InvalidAccount` 已有。

`kim-sdk/src/ids.rs`：删除 `AccountId(pub String)` / `DestId(pub String)`，改为 `pub use kim_protocol::{AccountId, DestId}`。保留 `ClientMessageId` / `SessionEpoch`（本刀不迁）。Dart/FRB 继续收字符串。

Royal `valid_account` 与 `kim-client` `valid_account` 改为 `AccountId::parse`。

## 核心缝（签名必须改）

### SessionStorage (`kim-router/src/storage.rs`)

```rust
async fn delete(&self, account: &AccountId, channel_id: &ChannelId) -> Result<(), SessionError>;
async fn get(&self, channel_id: &ChannelId) -> Result<Session, SessionError>;
async fn get_locations(&self, accounts: &[AccountId]) -> Result<Vec<Location>, SessionError>;
async fn get_location(&self, account: &AccountId, device: &str) -> Result<Location, SessionError>;
async fn list_locations(&self, account: &AccountId) -> Result<Vec<Location>, SessionError>;
```

`add(&Session)` 不变（proto 仍是 String）。实现里 `loc_of` 用 `ChannelId::from_trusted` / `GatewayId::from_trusted`。Redis key 仍用 `account.as_str()`。

`list_locations` default：`self.get_locations(std::slice::from_ref(account))`，**禁止**再 `account.to_string()` 包一层 Vec。

### Dispatcher

```rust
async fn push(&self, gateway: &GatewayId, channels: &[ChannelId], pkt: LogicPkt) -> Result<(), RouterError>;
```

`dest.channels` 线格式仍是逗号拼接字符串：`channels.iter().map(ChannelId::as_str).collect::<Vec<_>>().join(",")`。

### Location

```rust
pub struct Location {
    pub channel_id: ChannelId,
    pub gate_id: GatewayId,
    pub device: String,
    pub jti: String,
}
```

`encode`/`decode` 二进制布局不变（u16-LE 长度前缀）。decode 用 `from_trusted`（兼容测试里的空 channel）。比较改成 `l.channel_id.as_str() != channel_id.as_str()` 或直接 `PartialEq`。

### Server / Acceptor

```rust
// Server
async fn push(&self, channel_id: &ChannelId, payload: Bytes) -> Result<(), Error>;
async fn close_channel(&self, channel_id: &ChannelId) -> Result<(), Error>;

// Acceptor
async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<ChannelId, Error>;
async fn on_channel_ready(&self, channel_id: &ChannelId) -> Result<(), Error>;
async fn on_accept_abandoned(&self, channel_id: &ChannelId);
```

`StateListener::disconnect` 改为 `&ChannelId`（与 accept 对称，否则 serve_conn 还要来回转）。

`ChannelHandle::id` **保持** `-> &str`。`ChannelMap` 键保持 `Arc<str>`：`push` 里 `self.channels.get(channel_id.as_str())`。`Channel::pair` 继续 `impl Into<Arc<str>>`；accept 得到 `ChannelId` 后 `id.as_arc().clone()` 传入。

`ChannelNotFound` / `ChannelExists` 继续 Display 字符串（本刀不改 Error 枚举形状）。

## 文件清单

### 新文件

- `crates/kim-protocol/src/ids.rs` — 类型 + 单测（parse 过短/过长/非法字符/合法；空 ChannelId parse 失败、from_trusted 成功）

### kim-protocol

- `src/lib.rs` — `mod ids; pub use ids::{AccountId, ChannelId, DestId, GatewayId, ACCOUNT_MIN, ACCOUNT_MAX};`
- `src/error.rs` — 三个 Invalid* 变体

### kim-sdk

- `src/ids.rs` — re-export AccountId/DestId；删浅 newtype
- `src/lib.rs` — 导出路径保持，避免破坏 `kim_sdk::{AccountId, DestId}`

### kim-router

- `storage.rs` — 签名
- `dispatcher.rs` — 签名
- `location.rs` — 字段 + encode/decode + 测试字面量改 `from_trusted`
- `context.rs` — `dispatch_cmd` 用 `GatewayId` 做 HashMap 键、`ChannelId` 做 vec；`delete`/`get_location`/`list_locations` 接 newtype。HashMap 可用 `HashMap<GatewayId, Vec<ChannelId>>`（已 Hash+Eq）。`packet.clone()` 本刀不优化。
- `router.rs` — 测试 session 构造可仍用 proto String
- `test_support.rs` — RecordingDispatcher / NoopStorage 签名
- `lib.rs` — re-export 新类型（若对外方便）

### kim-session

- `memory.rs` — Inner map 键可用 `String`（Redis key 兼容）或 `AccountId`；`delete`/`get` 用 `as_str()` 查。`loc_of` 转 Location newtype。测试 helper 继续造 proto Session。
- `redis.rs` — 同上，命令参数 `account.as_str()`
- `cache.rs` / `dual.rs` — 转发签名
- `lib.rs` — 测试

### kim-core / kim-tcp / kim-ws / kim-container

- `conn.rs` / `server.rs` — 签名
- `kim-tcp/src/server.rs` — `ServeConnCtx`、`serve_conn`：accept 得 `ChannelId`，`Channel::pair(id.as_arc().clone(), …)`，`channels.add` 不变
- `kim-tcp/src/server.rs` `TcpServer::push` / `close_channel` + `impl Server`
- `kim-ws/src/server.rs` — 同构
- `tgateway/src/tls.rs` — `impl Server`
- DefaultAcceptor / EchoHandler / Probe / SlowEcho：`accept` 返回 `ChannelId::from_trusted(...)` 或 `parse`
- `kim-container` DownlinkHook 若带 channel_id：改 `&ChannelId`（`after_push`）
- `kim-tcp/tests/echo.rs` `tls.rs`；`kim-ws/tests/echo.rs` `wss.rs`；`kim-container/tests/e2e_echo.rs`

### 服务

- `services/chat/src/lib.rs` — `ChatHandler::accept` 返回 `ChannelId`；`ContainerDispatcher::push`；`StateListener::disconnect`
- `services/gateway/src/lib.rs` — 同上；`RecordingServer` 测试 mock
- `services/chat/src/login.rs`、`talk.rs`、`presence.rs`、`admin.rs`、`store/postgres.rs` 测试 mock：`impl SessionStorage` / `Dispatcher`
- 所有 `ctx.delete(account, channel)`：account 从 `session.account` `AccountId::from_trusted`（登录已校验）或 `parse`

Session proto 出站仍 String：`session.account` 不要改 prost 生成代码。从 `Context::session().account` 得到 `&str` 再 `AccountId::from_trusted`。

### 客户端测试

- `crates/kim-client/src/tests.rs` — FakeGw 等 Acceptor
- `crates/kim-client/src/auth.rs` — `valid_account` → `AccountId::parse`

### Royal

- `services/royal/src/auth.rs` — `valid_account` → `AccountId::parse`；`ACCOUNT_MIN/MAX` 改用 protocol 常量

## 转换策略

```text
JWT / 注册表单  --parse--> AccountId
网关生成 channel --from_trusted--> ChannelId
proto Session     --from_trusted--> 进 Location / Storage 查询
Location encode   --as_str--> 字节（布局不变）
Redis key         --as_str--> 现有 key_location
MessageStore      --as_str--> 本刀不改签名
Dart/FRB          仍是 String
```

禁止在循环里 `AccountId::parse` 热路径失败后静默 `from_trusted`。登录成功后的 session 字段用 `from_trusted`。

## 测试

新增（`kim-protocol`）：

- parse alice 成功；`ab` 失败；33 字符失败；`alice-bob` 失败；`Alice_01` 成功
- ChannelId::parse("") 失败；from_trusted("") 成功（Location 遗留）
- Borrow：`HashMap<AccountId, i32>` 可用 `.get("alice")`（若 Borrow 实现正确）

更新：所有 Location 字面量、SessionStorage mock、Acceptor 测试。`matches!(Location::decode(...), Err(SessionError::Other(_)))` 本刀不改（第 2 刀）。

## 不做

- MessageStore / UserDirectory / SocialDirectory / GroupDirectory 签名
- Command 枚举、HandlerFn 返回值、Error::Other
- LaneKeyFn、InsertMessage.body、dispatch 去深拷贝
- protobuf 字段类型、Redis key 布局、Location 二进制布局
- Dart/FRB、sdk/web
- ACK / pending receipt

## 验收命令

在 worktree 根目录：

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --offline
```

（若 `--offline` 缺依赖则去掉。）至少：

```bash
cargo test -p kim-protocol -p kim-router -p kim-session -p kim-core -p kim-tcp -p kim-ws -p kim-container -p kim-client --offline
cargo test -p chat -p gateway --offline
```

## 风险

- `Borrow<str>` + `Hash` 必须与 `str` 一致（hash 的是字符串内容，不是 Arc 指针）。
- Location 测试有空 `channel_id`：必须 `from_trusted`，不能 `parse`。
- `get_locations(&[String])` 改 `&[AccountId]` 会碰到 `vec!["alice".into()]` 测试；改成 `AccountId::from_trusted("alice")`。
- Chat e2e 若手写 session.account，from_trusted 即可。
- 不要给 ChannelId 实现 Deref，否则 `&ChannelId` 会偷偷变成 `&str` 让旧签名继续编译，审查会打回。
