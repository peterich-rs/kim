# 充实热路径并发、读写分离与通知缝合

状态：设计稿；Phase 1–7 已落地（含 `kim:loc:inv` 失效频道）。Phase 8（push 基准）未实施。compose 仍默认 `KIM_LOC_CACHE=0`；设 `1` 时启动必须订到失效频道。对照当前源码把「框架雏形」补成可继续长业务、可长跑的运行时骨架。

本文回答四个问题：

1. 除 `ChannelMap` 外，还有哪些数据结构/锁/读写模型配错了访问模式；
2. 读写分离在服务端 Channel 已落地，内部 `TcpClient` 与若干表上还缺哪一刀；
3. 订阅/通知缝（缓存失效、presence debounce、连接表插入）缺什么，不补会挡住后续业务；
4. 按什么顺序改，才能每步可编译、可回滚，且不和 G-03 / B0 产品缺口搅在一起。

实施顺序：Phase 1（ChannelMap）起，每相独立可编译。不和 G-03 / B0 搅在一起。

## Breaking Change Notice（实施时）

`ChannelMap` 的全部方法从 `async fn` 变为同步 `fn`（锁内不再 await）。kim-core 是 workspace 内部 crate，`kim-tcp` / `kim-ws` / gateway 同步改调用点即可。

额外破坏面（相对原稿多一处）：

- `ChannelMap::add` 改为返回 `Result<(), Error>` 或 `Option<Channel>`（占用则失败），消灭 `contains` + `add` 两拍竞态。`serve_conn` 调用点改为一次插入。

迁移步骤：

1. 替换 `crates/kim-core/src/channel_map.rs`；
2. 全仓去掉 `channels.xxx(...).await`（约 15 处），并把 `contains`/`add` 收成一次 `add`；
3. `TcpClient` 写路径改为与 Channel 相同的 mailbox（`kim-tcp` 内部，`Client` trait 签名不变）；
4. `cargo test --workspace`。

其余替换（ArcSwap / moka / LaneSet）不改公开 trait。

## Feasibility Assessment

对照代码后，改造点全部落在已有类型内部或 workspace 调用点：

- `ChannelMap` 临界区从不跨 await（hash 查找 + `Channel` clone）。同步化安全。`serve_conn` 的 `contains` + `add` 是两拍 TOCTOU（`crates/kim-tcp/src/server.rs` 232–243 行），dashmap `insert` 正好一并修掉。
- 服务端 `Channel` 已经是「写信箱 + 写专员」（`channel.rs` `WriteShared` / `pair`）。内部 `TcpClient` 仍对写半边持 `tokio::sync::Mutex` 并在 `send` / 心跳 / 读循环回 Pong 三条路径上跨 `write_frame`+`flush` 持锁（`kim-tcp/src/client.rs` 109–150、164–168 行）。通信层文档已点名「TcpClient 写侧还是 Mutex」。改成同一套 mailbox 不改 `Client::send` 签名。
- `Container.clients` 只在 reconcile 写、每次 RPC 读，表很小，`ArcSwap` 可行。
- `CachedSessionStore` 双 `Mutex<HashMap>` 无 TTL、无容量、无跨实例失效；`KIM_LOC_CACHE` 因此默认关（`kim-session/src/lib.rs` 74–77、89–99 行）。moka 先把「无界 + 永不过期」关掉；跨实例失效是后续通知缝，不是第一刀。
- `LaneSet` lane 不回收、`lane_id` 每帧 `String`、进程级 `in_flight=64` 均在 `channel.rs`。
- Presence `gens: Mutex<HashMap<String, u64>>` 只增不删（`presence.rs` 42、73–78 行）。debounce 退出时再读 Redis 位置，跨 Chat 实例靠二次确认兜底，不靠进程内 gen。
- 生产房间兴趣走 `RedisRoomInterest`（`interest.rs` 234 行起）；`MemoryRoomInterest` 仅本机/单测。DashMap 化 Memory 是顺手，不是吞吐关键。
- 依赖版本满足 `rust-version = 1.95`：dashmap 6、arc-swap 1.9、moka 0.12。

Fully feasible。不碰 G-03 / B0、不改 ACK 模型、不改 `sdk/*`。

## Current Surface Inventory

热路径表与锁（原稿已列，行号以当前工作区为准）：

- `crates/kim-core/src/channel_map.rs` — `Arc<tokio::sync::RwLock<HashMap<String, Channel>>>`，7 个 async 方法。
- `crates/kim-tcp/src/server.rs` — `contains`/`add`/`get`/`remove`/`all`；`serve_conn` 232–264 行两拍插入。
- `crates/kim-ws/src/server.rs` — 同构调用点；生命周期字段未抽成 `FrontendState`。
- `crates/kim-core/src/channel.rs` — 服务端写路径已 mailbox；`LaneSet { txs: Mutex<HashMap<String, Sender>>, workers: Mutex<Vec<JoinHandle>> }`；`lane_id` → `String`；`DEFAULT_MAX_IN_FLIGHT = 64`。
- `crates/kim-tcp/src/client.rs` — `writer: Option<Arc<Mutex<PlainTcpWriteHalf>>>`；`send` / 心跳 / 读循环 Pong 三处 `lock().await` 跨 IO。
- `crates/kim-container/src/container.rs` — `clients: Arc<RwLock<HashMap<String, ClientMap>>>`；读 184/220/329/355/364/376/420；写 162/342/441/475。`wanted` 仍 RwLock，`dialing` Mutex，`wake: Notify`。
- `crates/kim-session/src/cache.rs` — 双 `Mutex<HashMap>`；`get_locations` miss 逐账号 `list_locations`，不走 inner 批量 `get_locations`。
- `crates/kim-session/src/memory.rs` — 单把 `RwLock<Inner>`，clone-out 后放锁（测试后端，保持）。
- `services/router/src/lookup.rs` — `pick_idc` 每次重建 `slots`（164–171 行）。对照 `services/gateway/src/selector.rs` 已预计算。
- `services/chat/src/royal_pool.rs` — `clients: RwLock<Vec<Arc<RoyalClient>>>`，30s 刷新。
- `services/chat/src/interest.rs` — `MemoryRoomInterest` 双 `Mutex<HashMap>`；生产 `RedisRoomInterest`。
- `services/chat/src/presence.rs` — `gens: Mutex<HashMap<String, u64>>` 无退休；`on_location_added` / `on_location_removed` 由 `login.rs` 112–131 行调用。
- `services/chat/src/social_cache.rs` — 自研 TTL + cap + inflight 合并（模式正确，与 session cache 不统一）。
- `services/chat/src/notify.rs` — `notify_locations` 按 `gate_id` 聚合再 `Dispatcher::push`（广播正确落点，禁止改走 `ChannelMap.all()`）。
- `services/gateway/src/lib.rs` — `GatewayHandler::disconnect` 转发 `login.signout`（891–903 行）；`ChatHandler::disconnect` 只打日志（内部链路断开，不清理会话）。

## Design

### 现状与目标（一图）

```text
现状
  客户端 ══► Gateway Channel（mailbox + 写专员）
                 │
                 ├─ ChannelMap  tokio RwLock 全表
                 └─ 转发 ──► Container.clients RwLock
                                └─ TcpClient  Mutex 跨 write_frame ✗

目标
  客户端 ══► Gateway Channel（mailbox + 写专员）
                 │
                 ├─ ChannelMap  DashMap 分片  ★
                 └─ 转发 ──► Container.clients ArcSwap  ★
                                └─ TcpClient  同一套 mailbox  ★
```

通知缝（现状 → 目标）：

```text
Consul watch_loop ══► Container reconcile          （现有，不动）
login / signout   ──► PresenceHub.fanout
                      ├─ RoomInterest.viewers      （生产 Redis，不动）
                      └─ gens HashMap 只增不删 ✗   ★  debounce 后退休

CachedSessionStore
  本进程 write-through
  跨实例永不失效 ✗ → 先 moka TTL+容量 ★
                   → 再 Redis Pub/Sub 失效频道 ★（后相，解开 KIM_LOC_CACHE）

广播
  ChannelMap.all() 仅 shutdown drain（现有）
  业务扇出走 Location 子集 + Dispatcher   （现有，禁止改 all()）
```

### 诊断：质量问题按层，不是一张待办

骨架分层是干净的（`Conn` / `Channel` / `ChannelMap` / `Dispatcher` / `SessionStorage` / `RoomInterestStore`）。生产级差距不在「缺功能」，而在运行时合同没写完：

1. **数据结构与访问模式不匹配**
   - 连接表：高频点查 + 连接级 churn → 不该用一把 tokio RwLock。
   - 服务目录：读多写极少、表极小 → 不该用 dashmap，该 copy-on-write。
   - 会话缓存：要有界 + TTL + 负缓存 → 不该用裸 HashMap。
   - Royal 实例列表、IDC slots：读多写极少 / 配置期可算死。

2. **锁粒度与「锁跨 await」**
   - 服务端 Channel 已经把插座锁换成 mailbox，这一刀是对的。
   - 内部 `TcpClient` 还握着写半边 Mutex 做 syscall（心跳、业务 send、读循环回 Pong 抢同一把锁）。这是通信层文档点名的剩余违反。
   - `ChannelMap` / `Container.clients` 用 async RwLock 包纯 CPU 临界区，是调度器税，不是同步。
   - `FrontendState` 上 `acceptor`/`opts`/`drain_wait` 各一把 `StdMutex`：启动后几乎只读，过细且无必要。本轮不拆，避免和热路径搅在一起。

3. **读写分离不完整**
   - 网关连接：读专员 / 写专员 / 表锁 三件事已分开。
   - 内部 TCP：读半边 Mutex + 写半边 Mutex，读循环回 Pong 还要抢写锁 → 慢写会堵住心跳和读循环。
   - 查表：clone-out 再 await 的合同要写成类型契约（dashmap `Ref` 不得过 await）。

4. **订阅 / 通知缝没闭合**
   - Consul `subscribe` → `Notify` 唤醒 reconcile：正确。
   - 房间兴趣：生产 Redis SET，进房/退房/断连 `clear_channel`：正确。业务广播已经按 interest + `get_locations`，不是 `ChannelMap.all()`。
   - 会话缓存：没有失效频道，所以 `KIM_LOC_CACHE` 默认关。moka TTL 是上限，不是正确性。要开生产缓存必须有 `PUBLISH kim:loc:inv {account}`（或等价 watch）。
   - Presence debounce 的 gen 是进程内取消令牌；跨实例靠 debounce 后再 `list_locations`。gen 表不退休会随账号基数涨。
   - `ChannelMap.contains` + `add` 不是通知问题，是插入契约问题：同 id 双连可双双通过。

5. **无界表**
   - `CachedSessionStore` 无 cap / 无 TTL。
   - `LaneSet.txs` / `workers` 随 payload 派生的 lane key 涨。
   - `PresenceHub.gens` 随 `app:account` 涨。
   - `MemoryAckIndex` 有 TTL 无扫表（仅测试后端）。
   - `social_cache` 已有 cap + jitter TTL + inflight 合并——这是本仓库已经写对的缓存模板，session cache 应对齐而不是再手写一套。

6. **明确后置、本次不做**
   - G-03 / B0 pending receipt rollout。
   - `SO_REUSEPORT`、writev、jemalloc、io_uring。
   - `ChannelId`/`AccountId` newtype 扫全仓（`Channel.id` 已是 `Arc<str>`，表键先改 `DashMap<Arc<str>, Channel>` 吃掉热路径分配；业务 `Location` 字段保持 `String`）。
   - 把 `WsServer` 抽成 `FrontendState`（重复但正确；热路径之后单独切片）。
   - `async_trait` → native `async fn` in trait（版图大，不并入）。

### 关键设计决策

1. **`ChannelMap` = `DashMap<Arc<str>, Channel>`，API 全同步、全 clone-out。**
   否决 flurry（Guard 生命周期）；否决 ArcSwap（连接 churn 写放大）；否决保留 async。键用 `Arc<str>`：`Channel.id` 已是 `Arc<str>`，插入零拷贝，`get(&str)` 走 `Borrow<str>`。
2. **`add` 原子占用。** `insert` 若已有值，把旧 Channel 放回去（或用 `entry`），返回 `ChannelExists`。`serve_conn` 删除单独的 `contains` 拍。否决「先 contains 再 add」——两拍窗口在 dashmap 下仍然存在。
3. **`TcpClient` 写路径复用 Channel mailbox。** 握手 `into_split` 后，写半边交给独立写任务；`send` / Ping / Pong / Close 全部 `try_send`/`send` 进有界队列。读循环不再锁写半边。否决继续 Mutex：内部链路流量低于网关，但这是唯一还在「锁跨 IO」的长连接写路径，不补齐读写分离合同后面每个内部 RPC 都要重新解释。
4. **`Container.clients` 用 `ArcSwap<HashMap<String, ClientMap>>` + `clients_w: Mutex<()>`，不用 dashmap。** 读原子 load。写必须串行 clone-modify-store：`dial_and_insert` 与 `read_loop` 移除并发，两路同时 store 会丢更新。`wanted` / `dialing` 保持原锁（低频）。
5. **`CachedSessionStore` 用 `moka::sync::Cache`（TTL 60s + 容量 + jitter）。** `SessionStorage` 不变。`get_locations` miss 改为对 inner **一次** `get_locations(&misses)`，不再 N 次 `list_locations`。`locs` 值存 `Arc<Vec<Location>>`。负缓存（NotFound 短 TTL）本轮做，防穿透。跨实例失效 **本轮不接 Pub/Sub**，因此生产默认仍 `KIM_LOC_CACHE=0`；文档写明「开缓存的前提是 Phase 通知缝」。
6. **`in_flight` 分档。** 客户端 64，网关服务端默认 512，配置可调。否决每连接独立信号量。
7. **`LaneSet` 空闲回收 + `Arc<str>` key。** `retire` 必须 `Sender::same_channel` 比对。`workers` 不再全量收集 JoinHandle。
8. **Presence `gens` 在 debounce 任务结束时 `remove` 该 key（仅当仍是自己的 gen）。** 否决 moka 包 gen：这是取消令牌，不是缓存。
9. **`pick_idc` slots 预计算进 `Region`。** 对齐 gateway `Route.slots`。
10. **RoyalPool `ArcSwap<Vec<Arc<RoyalClient>>>`。** `lock_read`/`lock_write` 删除。
11. **`MemoryRoomInterest` → `DashMap`/`DashSet`。** 生产恒走 Redis 时收益为 0，但单测与本机 Memory 模式会用到；改动量小。不把 Redis 兴趣改成本地缓存——跨 Chat 分区必须 Redis 为真相。
12. **广播合同写进 ChannelMap 文档：** `all()` 弱一致，仅 shutdown；热路径扇出走 `RoomInterestStore` + `get_locations` + `Dispatcher`。

### 目标类型（核心）

`crates/kim-core/src/channel_map.rs`：

```rust
use dashmap::DashMap;
use std::sync::Arc;

/// 当前进程里所有活着的连接。
///
/// 取连接时先 clone 出 Channel（里面是 Sender），再 await 写网络。
/// 不要把 dashmap `Ref` 拿过 await——持守卫再碰同分片会死锁。
/// API 全部同步、全部 clone-out。`all()` 是弱一致快照，只给 shutdown drain。
#[derive(Clone, Default)]
pub struct ChannelMap {
    inner: Arc<DashMap<Arc<str>, Channel>>,
}

impl ChannelMap {
    pub fn add(&self, channel: Channel) -> Result<(), crate::Error> {
        let id = channel.id_arc();
        match self.inner.entry(id.clone()) {
            dashmap::mapref::entry::Entry::Occupied(_) => {
                Err(crate::Error::ChannelExists(id.to_string()))
            }
            dashmap::mapref::entry::Entry::Vacant(v) => {
                v.insert(channel);
                Ok(())
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<Channel> {
        self.inner.get(id).map(|r| r.value().clone())
    }

    pub fn remove(&self, id: &str) -> Option<Channel> {
        self.inner.remove(id).map(|(_, v)| v)
    }
}
```

需要给 `Channel` 加 `id_arc(&self) -> Arc<str>`（已有字段，只暴露）。

`TcpClient` 写侧（`crates/kim-tcp/src/client.rs`）：

```rust
// 握手后：
let (reader, writer) = conn.into_split();
let write = WriteShared::spawn(writer, options.write_wait, WriteFullPolicy::Block);
self.reader = Some(Mutex::new(reader));
self.write = Some(write);

// send / ping / pong / close 全部：
self.write.push_frame(opcode, payload).await
```

优先把 `WriteShared` 从 `channel.rs` 抽到 `kim-core` 可复用模块（`write_loop.rs`），TcpClient 与 Channel 共用，避免两套 mailbox。若抽取让 Phase 过大，Phase 2 先在 `kim-tcp` 复制最小写循环，Phase 后续再合并——**优先抽取**：两套写循环会再分叉策略。

`CachedSessionStore`：

```rust
pub struct CachedSessionStore {
    inner: Arc<dyn SessionStorage>,
    sessions: MokaCache<String, Session>,
    locs: MokaCache<String, Arc<Vec<Location>>>,
    neg: MokaCache<String, ()>, // NotFound，TTL ~5s
}
```

`get_locations`：锁外收集 miss → `inner.get_locations(&misses)` 一次 pipeline → 回填。`add`/`delete` 仍 write-through，并 `locs.invalidate(account)` + `neg.invalidate(account)`。

### `all()` 与广播

现状：`all()` 只在 tcp/ws shutdown drain。弱一致可接受。

约束（写入 ChannelMap 文档，代码用 `rg "\\.all\\(\\)"` 守门）：

- 不要用 `ChannelMap.all()` 做业务广播。
- 房间/群/presence：interest 成员索引 → `get_locations` → 逐个 `ChannelMap.get()`。
- 超大房间二级扇出仍属 `group-royal.md` / `reliable-delivery.md`，不并入。

## Phased Implementation

每相独立可编译。产品缺口（B0）不插队，也不被本切片挡住。

### Phase 1: ChannelMap → dashmap + 原子占用

**File: `Cargo.toml`（workspace）**

- `[workspace.dependencies]` 增加 `dashmap = "6"`。

**File: `crates/kim-core/Cargo.toml`**

- `dashmap.workspace = true`。

**File: `crates/kim-core/src/channel.rs`**

- 增加 `pub fn id_arc(&self) -> Arc<str>`。

**File: `crates/kim-core/src/channel_map.rs`**

- 整文件替换为 DashMap；方法同步；`add` 返回 `Result<(), Error>`。
- 单测覆盖：占用失败、get clone-out、remove。

**File: `crates/kim-tcp/src/server.rs`、`crates/kim-ws/src/server.rs`**

- 去掉 `.await`。
- `serve_conn`：删除 `contains` 拍，`add` 失败则 Close + `on_accept_abandoned`。

验证：`cargo test -p kim-core -p kim-tcp -p kim-ws`。

### Phase 2: 抽 WriteShared，TcpClient 去写锁

**File: `crates/kim-core/src/write_loop.rs`（新）**

- 从 `channel.rs` 移出 `WriteOp` / `WriteShared` / 写任务 spawn。
- `Channel::pair` 改为调用它。

**File: `crates/kim-tcp/src/client.rs`**

- `writer: Arc<Mutex<WriteHalf>>` → `WriteShared`。
- 心跳任务、`send`、读循环 Pong、`shutdown` 全部走 mailbox。
- 读循环只持读半边 Mutex（或 `Mutex<ReadHalf>` 仅因 `&self read`；长期可改成独占读任务，本相不强制——`Container.read_loop` 已是每连接一任务）。

**File: `crates/kim-tcp/tests/echo.rs`**

- 已有 `send_while_read_pending`：确认心跳 + send 并发不再互相卡住。补一条「慢写时 ping 仍能入队」。

验证：`cargo test -p kim-core -p kim-tcp -p kim-container`。

### Phase 3: Container clients → ArcSwap

**File: `Cargo.toml`、`crates/kim-container/Cargo.toml`**

- `arc-swap = "1"`。

**File: `crates/kim-container/src/container.rs`**

- `clients: Arc<ArcSwap<HashMap<String, ClientMap>>>`。
- 读：`load()`。写：clone → 改 → `store`。
- `slot_state` / `try_promote` / `force_adult` 不再 async（若签名允许）。

验证：`cargo test -p kim-container`。

### Phase 4: CachedSessionStore → moka；get_locations 批量回源

**File: `Cargo.toml`、`crates/kim-session/Cargo.toml`**

- `moka = { version = "0.12", features = ["sync"] }`。

**File: `crates/kim-session/src/cache.rs`**

- 双 Mutex → 双 moka + 负缓存。
- `get_locations` miss 走 `inner.get_locations(&misses)`。
- 容量/TTL 文件顶 `const`；淘汰时 `tracing::debug`（不要 warn 刷屏）。

**File: `docs/perf.md`**

- 写明：TTL 60s 是陈旧上限；跨实例写仍可能脏读；生产默认关缓存，直到通知缝落地。

验证：`cargo test -p kim-session`。

### Phase 5: LaneSet 回收 + in_flight 分档

**File: `crates/kim-core/src/channel.rs`**

- `txs: Mutex<HashMap<Arc<str>, Sender>>`；`lane_id` → `Arc<str>`（无 lane_key 时 `self.id.clone()` 零分配）。
- `lane_idle` 默认 30s；超时 `retire`（`same_channel` 比对）。
- 去掉 `workers` 全量 JoinHandle；drain = `txs.clear()` + 有界等待。

**File: `crates/kim-core/src/lib.rs`**

- `DEFAULT_SERVER_MAX_IN_FLIGHT = 512`。客户端常量保持 64。

**File: `crates/kim-tcp/src/server.rs`、`crates/kim-ws/src/server.rs`**

- `FrontendState::new` / `WsServer::bind` 用服务端档。

**File: `services/gateway/src/run.rs`**

- `GatewayConfig.max_in_flight` 默认 512，启动 `set_max_in_flight`。

验证：`cargo test -p kim-core`；新增 lane 空闲回收单测。

### Phase 6: 通知缝（presence gens 退休 + 零散读路径）

**File: `services/chat/src/presence.rs`**

- debounce 任务结束（无论是否 fanout）若 `gens[key]==gen` 则 `remove`。
- 单测：同一账号反复上线下线，表长度不单调涨。

**File: `services/router/src/lookup.rs`**

- `Region` 增加 `slots: Vec<usize>`，加载时 `build_slots`。`pick_idc` 只索引。

**File: `services/chat/src/royal_pool.rs`**

- `clients` → `ArcSwap<Vec<Arc<RoyalClient>>>`。

**File: `services/chat/src/interest.rs`**

- `MemoryRoomInterest` → `DashMap` + `DashSet`（或 `DashMap<String, HashSet<_>>` 值仍短锁）。`RedisRoomInterest` 不变。

验证：`cargo test -p kim-router -p chat`（以实际 package 名为准）。

### Phase 7: 会话缓存失效频道（已落地）

- Redis `add`/`delete` 成功后 `PUBLISH kim:loc:inv {account}`（`RedisSessionStore`，含 Royal 未缓存路径）。
- `KIM_LOC_CACHE=1` 时 `listen_cached`：启动必须 `SUBSCRIBE` 成功，否则进程拒绝起来；消息到达则 `invalidate_account`（loc + neg + 该账号下的 session 行）。订阅断开：`invalidate_all` 再退避重连。
- Royal 仍 `open_uncached_session_store`（事务内读位置）；只发 PUBLISH，自己不订。
- compose 保持 `KIM_LOC_CACHE=0`。要开：Chat 全部实例同时 `=1`，确认 Redis 允许 Pub/Sub。

### Phase 8: 基准

**File: `examples/kimbench/src/main.rs`**

- `--profile push`：已登录长连接 + 单向下行。口径 p50/p99、msg/s。

验收（相对值）：

- Phase 1：push p99 无回归；高并发 sender 下 msg/s 不差。
- Phase 2：内部转发在慢对端时心跳仍活（echo 单测锁住）。
- Phase 5：`in_flight` 等待时间在饱和前趋零。
- `rg "\\.all\\(\\)"` 无新的热路径广播。

## Architectural Notes

- **Semver：** `ChannelMap` async→sync 且 `add` 改签名；workspace 内迁移，kim-core minor bump。`WriteShared` 抽到 kim-core 是内部可见（`pub(crate)` 或 `pub` 但文档标明非业务 API）。
- **Trait 不动：** `SessionStorage`、`Conn`、`MessageListener`、`LaneKeyFn`、`Dispatcher`、`RoomInterestStore`、`Client`。
- **副作用：**
  - `all()` 弱一致：仅 shutdown。
  - moka TTL 引入最长 60s 跨实例陈旧窗口；**仍严于现状无界永不过期**；生产默认关缓存。
  - ArcSwap 写克隆整表：几十条、30s 一次，可忽略。
  - TcpClient mailbox：内部链路默认 `WriteFullPolicy::Block`，与现 `send().await` 背压语义一致。
  - lane 回收不改变同 lane FIFO；空闲 worker 退出，下一条消息重建。
  - presence gen 退休不改变 debounce 语义。
- **不做：** SO_REUSEPORT、writev、Redis 连接池、flurry、MemorySessionStore 生产化、WsServer/FrontendState 合并、ID newtype 全仓、G-03。
- **新增依赖：** dashmap 6、arc-swap 1.9、moka 0.12。均不要求调用方 `unsafe`（workspace `unsafe_code = "deny"`）。
- **与 production-gaps 的边界：** 本切片提高运行时质量与后续业务余量；不关闭任何 G-xx。漏 Push 补偿仍是 G-03。

## File Change Summary

- `Cargo.toml` -- workspace 增加 dashmap、arc-swap、moka
- `crates/kim-core/Cargo.toml` -- dashmap
- `crates/kim-core/src/channel_map.rs` -- DashMap + 同步 API + 原子 add
- `crates/kim-core/src/channel.rs` -- `id_arc`；WriteShared 外移；LaneSet 回收；lane key `Arc<str>`
- `crates/kim-core/src/write_loop.rs` -- 从 Channel 抽出的写专员（新）
- `crates/kim-core/src/lib.rs` -- 导出 write_loop；`DEFAULT_SERVER_MAX_IN_FLIGHT`
- `crates/kim-tcp/src/server.rs` -- ChannelMap 去 await；原子 add；服务端 in_flight 默认档
- `crates/kim-tcp/src/client.rs` -- 写路径改 mailbox
- `crates/kim-tcp/tests/echo.rs` -- 慢写 + ping 入队
- `crates/kim-ws/src/server.rs` -- 同 kim-tcp 的 ChannelMap / in_flight
- `crates/kim-container/Cargo.toml` -- arc-swap
- `crates/kim-container/src/container.rs` -- clients ArcSwap
- `crates/kim-session/Cargo.toml` -- moka
- `crates/kim-session/src/cache.rs` -- moka + 批量 miss + 负缓存 + invalidate_account
- `crates/kim-session/src/keys.rs` -- `LOC_INV_CHANNEL`
- `crates/kim-session/src/redis.rs` -- add/delete PUBLISH；`listen_cached` SUBSCRIBE fail-fast
- `crates/kim-session/src/lib.rs` -- `KIM_LOC_CACHE=1` 走 `listen_cached`
- `services/gateway/src/run.rs` -- `max_in_flight` 接线
- `services/router/src/lookup.rs` -- Region.slots 预计算
- `services/chat/src/royal_pool.rs` -- ArcSwap
- `services/chat/src/interest.rs` -- MemoryRoomInterest 分片
- `services/chat/src/presence.rs` -- gens 退休
- `examples/kimbench/src/main.rs` -- push 场景
- `docs/impl/hot-path-concurrency.md` -- 本文
- `docs/perf.md` -- 缓存 TTL / 失效前提
- `docs/communication-layer.md` -- TcpClient 不再 Mutex；ChannelMap 同步 clone-out
