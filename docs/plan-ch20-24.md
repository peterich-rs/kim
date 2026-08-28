# 下一阶段实现规划：可靠投递 → 里程碑 3

| 字段 | 值 |
|---|---|
| 日期 | 2026-08-28 |
| 状态 | 节点 A 已落地；B / C 仍是 Plan |
| 对照 | 小册第 20–24 章 |
| 前置 | 第 19 章在线单聊 / 群聊已落地 |
| 仓库 | 本机三进程：`pkt-client` → `fake-gateway` → `fake-chat` |

本文是**实现规划**，不是小册复述。第 20 章是原理、没有代码；真正动手的下一关键节点是 **20 + 21 合在一起**。工程师应按节点改协议、Handler、存储与测试，无需再翻小册才能开工。

---

## 1. 现在在哪

| 小册 | 题 | 仓库现状 |
|---|---|---|
| 11–13 | 通信层 TCP / WS | **已落地**。验收是 crate 测试，不是 echo 二进制 |
| 14 | 业务包 | **已落地** `kim-protocol` |
| 15–16 | 容器 / 发现 | **已落地** 静态 Naming + Container。Consul 没有 |
| 17–18 | 链路层登录 | **已落地** JWT / 会话 / 互踢 |
| 19 | 控制层在线 talk | **已落地** `chat.user.talk` / `chat.group.talk` / 最小 `chat.group.create`。**at-most-once**，无 ACK、无离线 |
| 20 | 可靠投递原理 | 未做。本章没有独立代码 |
| 21 | 存储与离线同步 | `MessageStore` 只有 `insert_user` / `insert_group`；Memory 各记一条，**不是写扩散**；进程退出即丢 |
| 22 | 服务治理与群管理 | `GroupDirectory` 只有 `create` / `members`。无 join/quit/detail、无 Royal HTTP、无 Consul |
| 23–24 | Web SDK / 里程碑 3 | `pkt-client` 是 Rust CLI，不是 TS SDK |
| 25–32 | 压测、no-copy、监控、部署 | 以后 |

进程外壳不变：网关对 `chat.*` 仍然 `forward(service_name())`。**不要**把 ACK / 离线 / SQL 写进 `TcpServer` / `WsServer`。

### Example 清单（已按现状裁掉）

现状只需要三进程 Demo。通信层回声由 crate 测试覆盖。

| 留 | 角色 |
|---|---|
| `examples/fake-gateway` | WGateway |
| `examples/fake-chat` | Chat（登录 + echo + talk + 本规划的 ACK / 离线） |
| `examples/pkt-client` | Web 客户端 |

已删除：`echo-server` / `echo-client` / `ws-echo-server` / `ws-echo-client`。替代验收：

- `crates/kim-tcp/tests/echo.rs`
- `crates/kim-ws/tests/echo.rs`
- `crates/kim-container/tests/e2e_echo.rs`（容器 + 第一帧名字，不是登录）

不新起 TGateway 二进制。App / TCP 电线仍是 `kim-tcp`，本规划不实现公网 TGateway。

---

## 2. 为什么下一个关键节点是 20 + 21

第 20 章把投递语义从 at-most-once 改成「服务端持久化 + 读索引 + SDK 幂等 ≈ exactly-once」，并选定 **Pull** 离线、**索引与内容分离**、ACK 只带**最大 messageId**。

第 21 章才是代码：写扩散两张表、ACK 写 Redis、离线 index / content。Chat 的 talk 控制流（寻址 → insert → Push → Resp）**已经在第 19 章写好**；本节点只换存储语义、加三条指令。

单独做第 20 章没有可提交的行为；单独做 Postgres 而不做 ACK / 离线，表是死的。所以下一关键节点叫：

**可靠投递 + 持久化离线（小册 20 + 21）**

第 22 章（Royal HTTP + Consul + 群 join/quit）和第 23–24 章（TS SDK）是后面两个节点，**本节点禁止提前做**。

```text
现在                          节点 A（下一阶段）                 节点 B            节点 C
at-most-once                  写扩散 + ACK + Pull 离线         Royal HTTP        浏览器 SDK
Memory 一条记录               Memory 默认可测；Postgres 可选     Consul 可选       里程碑 3
无 chat.talk.ack              三条新指令，talk 控制流基本不改    join/quit/detail
```

---

## 3. 节点 A — 可靠投递 + 持久化离线

### Goals

- 接收方对 Push（或离线拉下来的消息）发 `chat.talk.ack`，服务端只保存该账号的 **latest messageId**。
- 重连后发 `chat.offline.index`，按读索引拉出发给自己、且 `direction=0` 的索引；再按 id 列表发 `chat.offline.content` 拉正文。
- 在线已 ACK 的消息，重连后 **不会** 再出现在离线索引里。
- 在线未 ACK 就断线：重连后能把该消息拉回来（at-least-once）。SDK / pkt-client 按 `messageId` 去重后，对上层是 exactly-once。
- `insert_user` / `insert_group` 改为**写扩散**：1 条内容 + N 条收件箱索引。发送方自己也有 `direction=1` 的一份，供多设备 / 历史；离线拉取只读 `direction=0`。
- 默认 `cargo test --workspace` **不**要求 Postgres / Redis / Docker（与现在会话测试相同）。
- `chat.demo.echo`、`e2e_login.rs`、在线 talk e2e **不回归**。

### Non-Goals

- `examples/fake-royal`、Consul、SRV。
- `chat.group.join` / `quit` / `detail`、`GroupCreateNotify`。
- 把 `pkt-client` 改成 TypeScript。
- 敏感词、多设备 `device != ""`、TGateway、VPS、WSS。
- 把存储或 ACK 写进 `kim-tcp` / `kim-ws` / `kim-core`。
- 新建 `kim-chat` / `kim-store` crate（第二条 HTTP 适配器出现在第 22 章再抽）。

### Key Decisions

1. **Chat 进程不拆。** Handler 仍在 `fake-chat`。`MessageStore` 是唯一存储缝；Memory 与 Postgres 是两条适配器。talk 的寻址 / dispatch / Resp 合同不变。

2. **离线走长连接指令，不走客户端 HTTP。** 小册 SDK 发的是 `chat.offline.index` / `chat.offline.content`；Royal 的 REST 是 Chat → 存储。本节点 Chat 直接调 trait，不经 HTTP。

3. **ACK 只带最大 `messageId`。** 不存 delivered 位，不传 id 列表。`messageId == 0` 是 no-op（对齐小册 `setMesssageAck`）。

4. **读索引默认进程内 HashMap；Redis 可选。** key = `chat:ack:{account}`，TTL 30 天。与 `kim-session` 的 `redis` feature 同一开关习惯：`fake-chat --features redis` 时 ACK 走 Redis，否则 Memory。

5. **Postgres 可选 feature `postgres`。** sqlx 0.8 + Tokio + rustls。无 `DATABASE_URL` 时测试跳过 PG 适配器，不 fail workspace。库是 Postgres，不是小册的 MySQL。协议时钟仍是 UnixNano `i64`，表里 **不要** 改成 `timestamptz`。

6. **群 insert 必须先拿到成员。** 当前 `do_group_talk` 是 insert 再 `members()`。写扩散需要成员列表才能铺索引。改为：decode → `members()` → `insert_group(..., members)` → 寻址 / dispatch → Resp。未知群：`members` 仍返回空列表，insert 只写 content（0 条索引），发送方 Success、无 Push。这是第 19 章已有语义。

7. **不把 `GroupDirectory` 并进 `MessageStore`。** 两条缝保持独立。成员列表由 Handler 传入 `insert_group`。

8. **主键是雪花 `BIGINT`，不是 `GENERATED ALWAYS AS IDENTITY`。** `message_id` 与协议字段是同一个值。`message_index` 行另有自己的雪花主键。

9. **发送方 Success 仍表示「insert 成功」。** 不表示对端已 ACK。dispatch 失败仍 `SystemException`（第 19 章合同）。部分成员已收到、发送方看到失败：at-least-once 下由 ACK + 离线补。

10. **`pkt-client` 默认路径仍是 ping + echo。** HOLD 收到 talk Push 后默认延迟 ACK；`KIM_SKIP_ACK=1` 用于「变成离线」的 e2e。`KIM_SYNC_OFFLINE=1` 在登录后先拉离线再走原路径。

### 协议

`crates/kim-protocol/proto/pkt.proto` 追加（字段名与 JSON / prost 对齐现有 `camelCase` proto）：

```protobuf
message MessageAckReq {
  int64 messageId = 1;
}

message MessageIndex {
  int64 messageId = 1;
  int32 direction = 2;
  int64 sendTime = 3;
  string accountB = 4; // 对话另一方（发送方账号，当 direction=0）
  string group = 5;    // 空 = 单聊
}

message Message {
  int64 messageId = 1;
  int32 type = 2;
  string body = 3;
  string extra = 4;
}

message MessageIndexReq {
  int64 messageId = 1; // 0 = 冷启动，用服务端读索引
}

message MessageIndexResp {
  repeated MessageIndex indexes = 1;
}

message MessageContentReq {
  repeated int64 messageIds = 1;
}

message MessageContentResp {
  repeated Message messages = 1;
}
```

`wire.rs`：

```text
CMD_CHAT_TALK_ACK      = "chat.talk.ack"
CMD_OFFLINE_INDEX      = "chat.offline.index"
CMD_OFFLINE_CONTENT    = "chat.offline.content"
```

账号一律从 `ctx.session().account` 取，**不**信客户端 body 里的 account。`Header.dest` 这三条都为空。

Status：不新增号。非法 body → `InvalidPacketBody=101`；存储失败 → `SystemException=99`；未登录 → 现有 `SessionNotFound=404`。`content` 一次超过 200 个 id → `InvalidPacketBody`（比 HTTP 400 更贴近现有长连接合同）。

### 常量（与小册一致）

| 名 | 值 | 用途 |
|---|---|---|
| 读索引 TTL | 30 天 | Redis / Memory 过期 |
| 单次索引条数 | 2000 | `offline.index` LIMIT |
| 离线消息窗口 | 15 天 | `get_sent_time` 下限 |
| 单次正文条数 | 200 | `offline.content` |

### `MessageStore` 缝（加深，不新开缝）

```rust
pub struct MessageIndexRow {
    pub message_id: i64,
    pub direction: i32,
    pub send_time: i64,
    pub account_b: String,
    pub group: String,
}

pub struct MessageContentRow {
    pub message_id: i64,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
}

#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn insert_user(&self, app: &str, req: &InsertMessage)
        -> Result<InsertResult, StoreError>;
    async fn insert_group(
        &self,
        app: &str,
        req: &InsertMessage,
        members: &[String],
    ) -> Result<InsertResult, StoreError>;
    async fn ack(&self, app: &str, account: &str, message_id: i64)
        -> Result<(), StoreError>;
    async fn offline_index(
        &self,
        app: &str,
        account: &str,
        message_id: i64,
    ) -> Result<Vec<MessageIndexRow>, StoreError>;
    async fn offline_content(
        &self,
        app: &str,
        message_ids: &[i64],
    ) -> Result<Vec<MessageContentRow>, StoreError>;
}
```

`insert_group` 加 `members` 是这个节点对 trait 的**唯一破坏性变更**。调用点只有 `do_group_talk`；单测用 `MemoryGroupDirectory::seed` 的地方跟着改。

写扩散（适配器内部，Handler 看不见）：

| 调用 | content | index 行 |
|---|---|---|
| `insert_user` dest=B, sender=A | 1 条，id=雪花 | A 行 `direction=1` account_b=B；B 行 `direction=0` account_b=A。`group=""` |
| `insert_group` dest=G, members=M | 1 条 | 每个成员一行：`account_a=成员`，`account_b=sender`，`group=G`；发送方 `direction=1`，其余 `0` |
| 成员为空 | 1 条 content | 0 条 index |

`offline_index` 对齐小册 `getSentTime`：

1. `message_id == 0`：读该账号读索引（可能仍是 0）。
2. `message_id > 0`：用 **content 表** 的 `send_time`（不要用 index 表）。找不到 → 起点 = now − 1 天。
3. 再与「now − 15 天」取较晚者。
4. `SELECT ... WHERE app=? AND account_a=? AND send_time>? AND direction=0 ORDER BY send_time ASC LIMIT 2000`。
5. 请求里的 `message_id > 0` 时，在返回前 `ack(account, message_id)`（小册 index handler 会 `setMesssageAck`；0 不改读索引）。

`offline_content`：按 id 列表取 content，**保持请求顺序或按 id 排序，文档里写死一种**（建议按请求顺序，缺失的 id 跳过不报错）。超过 200 个由 Handler 拦截，进不了 store。

读索引可以是 `MessageStore::ack` 的实现细节（Memory 一张 map；Postgres 适配器仍把 ack 放 Redis 或同进程 map）。**不要**给 index 表加 `delivered` 列。

### Handler

| command | 行为 |
|---|---|
| `chat.talk.ack` | 解 `MessageAckReq` → `store.ack(app, session.account, messageId)` → Success 空 body |
| `chat.offline.index` | 解 `MessageIndexReq` → `offline_index` → `MessageIndexResp` |
| `chat.offline.content` | 解 `MessageContentReq`；len>200 → InvalidPacketBody；否则 `offline_content` → `MessageContentResp` |
| `chat.user.talk` | **不改顺序**：寻址 → insert_user（现在会写 2 条索引）→ dispatch → Resp |
| `chat.group.talk` | **只改一步**：members 提前到 insert 之前；insert_group 传入 members |

网关零改动。`service_name("chat.talk.ack")` 是 `"chat"`，`service_name("chat.offline.index")` 也是 `"chat"`。给 `wire.rs` 补单元测试。

### Postgres 表（feature `postgres`）

小册是 MySQL `t_message_*`。本仓库用 Postgres、snake_case、无 `t_` 前缀。`send_time` 保持 `BIGINT` UnixNano，因为查询与协议用同一把尺。

```sql
CREATE TABLE message_content (
    id BIGINT PRIMARY KEY,          -- 雪花 = 协议 messageId
    app TEXT NOT NULL,
    type SMALLINT NOT NULL,
    body TEXT NOT NULL,
    extra TEXT NOT NULL DEFAULT '',
    send_time BIGINT NOT NULL
);

CREATE TABLE message_index (
    id BIGINT PRIMARY KEY,          -- 另一颗雪花，不是 content.id
    app TEXT NOT NULL,
    account_a TEXT NOT NULL,        -- 收件箱主人
    account_b TEXT NOT NULL,        -- 对方
    direction SMALLINT NOT NULL CHECK (direction IN (0, 1)),
    message_id BIGINT NOT NULL REFERENCES message_content (id),
    "group" TEXT NOT NULL DEFAULT '',
    send_time BIGINT NOT NULL
);

CREATE INDEX message_index_inbox
    ON message_index (app, account_a, direction, send_time);

CREATE INDEX message_index_message_id
    ON message_index (message_id);
```

- 单聊 / 群聊 **同一对表**。群 id 放 `"group"`（Postgres 保留字，必须双引号或改名为 `group_id`；**推荐列名 `group_id`**，proto 字段仍叫 `group`）。
- insert 一条消息 = **一个事务**：先 content，再全部 index。失败 → Handler `SystemException`，不 dispatch。
- 迁移：sqlx `migrations/`，CI 不跑 PG；本机 / 可选 job 用 `DATABASE_URL`。
- 连接池：`PgPool`，max/acquire/idle 来自 `fake-chat/config.toml`，禁止硬编码。
- 生产路径禁止 `unwrap` / `expect`。sqlx 错误进 `StoreError::Backend`。

### Redis 读索引（feature `redis`）

```text
SET chat:ack:{account} {messageId} EX 2592000
```

`messageId==0` 不写。与会话 Redis 共用 `REDIS_URL`，不要第二套连接配置。无 Redis 时 Memory map：进程退出丢读索引（与现在丢会话一样可接受）。

### `pkt-client`

| 条件 | 行为 |
|---|---|
| 默认 | 不变：JWT → ping → echo |
| `KIM_HOLD=1` 收到 talk Push | 打日志；**除非** `KIM_SKIP_ACK=1`，否则延迟 `KIM_ACK_DELAY_MS`（默认 200）后发一条 `chat.talk.ack`，body 为目前收到的最大 `messageId` |
| `KIM_SYNC_OFFLINE=1` | 登录成功后先 `offline.index`（messageId=0 或 `KIM_ACK_FROM`）再按返回 id 分批 `offline.content`（每批 ≤200），然后才走 HOLD / talk / echo |
| 其它路径 | 不自动拉离线 |

去重：pkt-client 用 `HashSet<i64>` 记已展示的 `messageId`。Push 与离线 content 撞号只打一次。这就是小册「SDK 幂等」。

### 测试

**单测（Memory，默认 CI）**

- 写扩散：`insert_user` 后 index 恰好 2 行，方向相反。
- 群：3 个成员 → 1 content + 3 index；发送方 direction=1。
- ACK 0 不改索引；ACK 较大 id 后 `offline_index` 不再包含更早的 direction=0 行。
- 冷启动 messageId=0 且从未 ACK：窗口裁到 15 天（单测用可注入时钟或直接插旧 `send_time`）。
- content 请求 201 个 id：Handler 返回 101，store 不被调用。

**e2e（`e2e_offline.rs`，仍是单 `fake-gateway`）**

1. alice→bob 在线，bob ACK，bob 重连 + `offline.index` → 不含该 id。
2. alice→bob，bob **不 ACK** 即断，bob 重连 + sync → 有该 id 且 content.body 一致。
3. 三人群，carol 离线；alice 群聊；carol 上线 sync → 有 Push 里同一条；alice 自己 direction=1 **不**出现在 carol 的 index 里，**出现**在 alice 自己的历史口径（本节点 e2e 只断言接收方）。
4. 登录 + echo 回归仍绿。

**Postgres**（`#[ignore]` 或 `DATABASE_URL` 才跑）：同一套写扩散 / 离线查询断言。

### 文档

新 `docs/reliable-delivery.md`（已落地规格，风格对齐 `control-layer-chat.md`）。改：

- `docs/control-layer-chat.md`：删「没有 chat.talk.ack」；指向新文。
- 根 README / glossary / architecture：离线从「以后」改为「已落地（Memory；PG/Redis 可选）」。
- `docs/plan-ch20-24.md`：节点 A 完成后把状态改成节点 A done。

### PR 切分（节点 A）

每个 PR 可独立 `cargo test --workspace`。

**PR1 — `feat(protocol): talk ack and offline packets`**

- Files: `pkt.proto`、`wire.rs`、生成代码。
- 不改 Handler。补 `service_name` 单测：`chat.talk.ack` / `chat.offline.*` → `"chat"`。

**PR2 — `feat(chat): write-fanout memory store and ack index`**

- Files: `store.rs`（trait 扩展、Memory 写扩散 + 读索引 map）、`talk.rs`（群 members 提前）、store 单测。
- 仍不注册 ACK / 离线指令。在线 talk e2e 必须仍绿。

**PR3 — `feat(chat): ack and offline handlers`**

- Files: 新 `ack.rs` / `offline.rs`，`lib.rs` 注册三条指令。
- pkt-client：HOLD ACK、`KIM_SKIP_ACK`、`KIM_SYNC_OFFLINE`。
- `tests/e2e_offline.rs`。
- `docs/reliable-delivery.md` 与交叉链接。

**PR4 — `feat(chat): optional postgres message store`**

- Files: `store/postgres.rs`、sqlx 迁移、`Cargo.toml` feature `postgres`、config 里 `database_url`。
- 依赖 PR2。无 `DATABASE_URL` 时 feature 仍能编译，测试 `#[ignore]`。

**PR5 — `feat(chat): optional redis read index`（可与 PR4 并行，依赖 PR2/PR3）**

- ACK 在 `redis` feature 下写 `chat:ack:{account}`。
- 与会话共用 `REDIS_URL`。无 Redis 时行为与 Memory 相同。

建议落地顺序：**PR1 → PR2 → PR3** 是可演示的最小闭环（全 Memory）。PR4 / PR5 不阻塞「下一章已经能跑离线」。

---

## 4. 节点 B — 服务治理与群管理（小册 22）

在节点 A 之后做。Chat 的 talk / ACK / 离线 Handler **不再改控制流**，只换适配器。

### Goals

- 新进程 `examples/fake-royal`：axum HTTP。`Content-Type` / `Accept`：`application/x-protobuf`（JSON 可后做，本节点不必须）。
- 路径对齐小册：

```text
POST   /api/:app/message/user
POST   /api/:app/message/group
POST   /api/:app/message/ack
POST   /api/:app/offline/index
POST   /api/:app/offline/content
POST   /api/:app/group
POST   /api/:app/group/member
DELETE /api/:app/group/member
GET    /api/:app/group/members/:group
```

- Chat 侧 `HttpMessageStore` / `HttpGroupDirectory` 实现已有 trait；`config.toml` 里 `royal_url = "http://127.0.0.1:8080"`。默认仍可 Memory（无 Royal 也能跑现在的测试）。
- 长连接补：`chat.group.join` / `chat.group.quit` / `chat.group.detail`。create 已有，改为走 Royal。
- `GroupCreateNotify`：本节点才发。join 成功后给在线成员 Push。
- dest / 成员表里的群 id **继续 base36**（第 19 章已拍板）。Royal HTTP 若返回 groupId，与 dest 同一字符串。**禁止**把 dest 改成小册 REST 的 Base32。

### Non-Goals

- 改 talk 的 insert/dispatch 顺序。
- 本机改系统 DNS、53 端口 Consul。Consul 是 **Naming 的第二条适配器**，默认测试仍 StaticNaming。
- Service Mesh、sidecar。
- Web SDK。

### Consul（可选，第二条 Naming 适配器）

小册用 SRV 调 Royal，几乎不改业务代码。本仓库对应：

- `kim-naming` 增加 `ConsulNaming`（feature `consul`），Chat / Royal 启动时注册。
- HTTP 客户端用 `LookupSRV` 或静态 URL；静态 URL 是默认，Consul 是可选。
- **不要**为了 Consul 改 `TcpServer`。

没有第二条调用方之前，不抽 `kim-royal` 库 crate：REST 先放 `examples/fake-royal`，与 `fake-chat` 对称。

### PR 切分（节点 B，到点再开设计）

1. `fake-royal` + 消息三条 REST，Chat `HttpMessageStore`，静态 URL。
2. 群 REST + `HttpGroupDirectory`；create 改走 HTTP。
3. 长连接 join/quit/detail + `GroupCreateNotify` + e2e。
4. 可选 `ConsulNaming`。

---

## 5. 节点 C — Web SDK / 里程碑 3（小册 23–24）

### Goals

- 新目录 `sdk/web`（TypeScript）。指令集合对齐小册：signin/signout、user/group talk、talk.ack、offline.index/content、group create/join/quit/detail。
- 浏览器 Demo：login、收发、断线重连、离线拉、被踢。
- 事件：Closed / Reconnecting / Reconnected / Kickout；`onmessage` 与 `onofflinemessage` 分开。
- protobuf 与 `pkt.proto` **同一份**（buf / protobufjs）。不要手写一份 Header。

### Non-Goals

- 删 `pkt-client`。Rust CLI 继续给仓库 e2e 用。
- 浏览器本地 SQLite。Web 端去重用内存 Set（小册已说明换浏览器等于没存储）。
- 公网 WSS / Cloudflare。本机 `ws://127.0.0.1:8001/`。

里程碑 3 的验收：浏览器走同一套 fake-gateway + fake-chat（+ 可选 fake-royal），完成登录、1:1、群、ACK、离线，而不是「把 pkt-client 翻译成 TS」本身。

### PR 切分（到点再开设计）

1. 协议层 + ping/pong + login。
2. 连接状态机 + 心跳重连。
3. talk + ack + 在线 Push。
4. 离线 index/content + 群指令。
5. 静态 Demo 页 + 对照文档。

---

## 6. 更后（小册 25–32）

到里程碑 3 再排。不要在节点 A 掺进去。

| 章 | 题 | 对本仓库意味着什么 |
|---|---|---|
| 25 | 基准测试 | 单独 bench 包；先测网关转发与 talk 写扩散，不拿 echo 二进制当基线 |
| 26 | no-copy | `Bytes` 切片；热路径少 `to_vec` |
| 27 | 缓冲 | 写合并 / writev；`TcpClient` 写侧 Mutex → 信箱 |
| 28 | 存储优化 | 索引、池、批量 insert |
| 29 | 智能路由 | 独立 Router 服务；现在 StaticNaming 全拨号 |
| 30 | 多租户 / 灰度 | JWT `app` 已有；灰度是 Naming / 标签 |
| 31 | 监控 | tracing + 指标；里程碑 4 |
| 32 | 部署容灾 | TGateway TLS、WSS、多实例 |

并行但不在 20–24 关键路径上的缺口：**TGateway**（App TCP 入口）。现在只有 WGateway。需要时新 `examples/fake-tgateway`，复用登录 Accept 合同、换 `TcpServer`。不要复活 echo 二进制来冒充它。

---

## 7. 节点之间的缝（不要拆掉）

```text
pkt-client / 以后的 TS SDK
        │  LogicPkt
        ▼
fake-gateway          仍然只 forward(service_name())
        │  TCP InnerHandshake
        ▼
fake-chat Router
        ├─ login.*          kim-session
        ├─ chat.demo.echo
        ├─ chat.user.talk  ─┐
        ├─ chat.group.talk ─┼─ MessageStore      Memory | Postgres | 以后 HTTP Royal
        ├─ chat.talk.ack   ─┤
        ├─ chat.offline.*  ─┘
        └─ chat.group.*  ──── GroupDirectory     Memory | 以后 HTTP Royal
```

节点 A 只加深左边 Chat 与 `MessageStore`。节点 B 在右边加进程。节点 C 只换最上面的客户端。

---

## Open Questions

本规划把能拍板的都拍了。若要改方向，只剩产品选择，不是技术阻塞：

1. 节点 A 是否要在 PR3 之后立刻做 Postgres，还是 Memory 闭环就算「第 21 章 Demo 完成」、PG 放到节点 B 与 Royal 一起。
2. 群 id 列名用 `group_id`（推荐）还是双引号 `"group"` 以贴近小册字段名。

默认：**Memory 闭环先合并；Postgres 作为节点 A 的 PR4，不挡演示。列名 `group_id`。**
