# Remove Royal as a Write-Path SPOF: Consul Discovery, Circuit Breaker, Friend Cache

| 字段 | 值 |
|---|---|
| 状态 | Draft（审查修订：缓存键分种类、5xx 计熔断、Snowflake 失败即退出、缓存淘汰） |
| 日期 | 2026-09-02 |
| 覆盖 | G-16（Royal 进 Consul 多实例 + Chat 侧熔断 + 好友/黑名单短 TTL 缓存） |
| 父规格 | [next-stage.md](./next-stage.md) B5、[production-gaps.md](../production-gaps.md) G-16；Snowflake 启动失败依赖 G-22 半边 |

## Breaking Change Notice

无公共 API 破坏。`RoyalClient::new/with_hmac` 构造签名不变；新增 `RoyalPool` 作为 `HttpBackends` 的内部实现细节。`http_backends_with_hmac` 签名不变（`royal_url` 参数语义变为「初始/兜底地址，发现开启时仅 bootstrap」）。

## Feasibility Assessment

- Royal **已注册进 Consul**（`services/royal/src/main.rs:302-326`：`public_address` 非空即 register + health_url meta + graceful deregister），compose 里 `KIM_PUBLIC_ADDRESS: royal` 已生效——「Royal 进 Consul」的服务端半边已存在。缺的是 **Chat 侧消费发现** 与 **多实例可行性**。
- Royal 多实例的剩余障碍：`RoyalState` 的 Memory 后端（无 PG 时）不共享——但生产 compose 恒有 `DATABASE_URL`（PG backends + Redis revoke/nonce/device_hot），PG 模式下 Royal 接近无状态（session store 走 Redis、幂等靠 PG、token_epoch 缓存 Redis）。**多实例已可行**，只补 compose 第二实例与 snowflake node 配置（`royal.toml` 已有 `snowflake_node = 10`，第二实例给 11）。
- `kim-naming` 的 `Naming::find/subscribe` 已可用（Consul blocking watch 在 `consul.rs`）；gateway 的 `Container` 已示范 subscribe 消费。
- Chat 对 Royal 的全部调用集中在 `services/chat/src/royal.rs` 的 `RoyalClient`（一个 reqwest::Client + 一个 base URL）——池化的改造面收敛在 `send_pb` / `post_maybe_empty` 两个传输函数。
- 好友/黑名单读集中在 `talk.rs` 的 `social.is_blocked_either` / `is_friend`（每次私聊 2 次 RPC）与 `users.exists`（1 次）。
- Royal / Chat 当前 `SnowflakeGen::try_new` 失败会降 `SequenceIdGen(10001)`（`royal/src/main.rs:177-183`、`chat/src/lib.rs:124-129`）。两实例同时误配置会撞号。**B5 合入 royal-2 之前必须把该降级改成启动失败。**
- **Feasible with caveats: G-22 半边（init 失败退出）是本切片前置。**

## Current Surface Inventory

- `services/royal/src/main.rs:288-326` — Consul register（已有）；`--scan-empty-jti` 子命令
- `services/royal/src/main.rs:108-110` — `open_devices` PG 池
- `deploy/compose.yml:131-160` — royal 单实例；chat `ROYAL_URL: http://royal:8080`（固定 URL）
- `services/chat/src/main.rs:103-110` — `royal_url_from_env_or_cfg`：`ROYAL_URL` env 或 toml
- `services/chat/src/royal.rs:91-125` — `RoyalClient { base, http, hmac_secret }`：单 base
- `services/chat/src/royal.rs:127-190` — `send_pb` / `post_maybe_empty`：传输层（B4 后带退避）
- `services/chat/src/royal.rs:917-947` — `http_backends_with_hmac[_receipt]` → `HttpBackends` 四元组（store/groups/users/social）
- `services/chat/src/talk.rs:82-130` — 私聊写路径：exists → is_blocked_either → is_friend（3 次 Royal RPC）
- `services/chat/src/talk.rs`（群聊段）— members → insert（2 次）
- `crates/kim-naming/src/naming.rs:14-28` — `Naming::find/subscribe/register/deregister`
- `crates/kim-naming/src/consul.rs` — blocking watch 实现
- `crates/kim-session/src/cache.rs` — `CachedSessionStore`（loc cache 先例：Mutex<HashMap> 无 TTL，opt-in）——本切片好友缓存**不复用**它（见决策 4）

## Design

### 决策

1. **`RoyalPool` 替换单 base，`find` + 定时刷新而非 subscribe**：Chat 对 Royal 是请求/响应，不需要 Container 那种长连接 watch。`RoyalPool` 内 `RwLock<Vec<Arc<RoyalClient>>>`，后台 task 每 10s `Naming::find("royal", &[])` 全量替换；`ROYAL_URL` 静态配置时池退化为单地址（本机/e2e 路径完全不变）。拒绝 subscribe 回调——`find` 轮询 10s 对「多实例扩缩容感知」足够，代码面小一半；Consul watch 的价值在 push 及时性，Royal 实例变更不是秒级敏感。
2. **熔断按实例；4xx 不计失败，连接错误/超时/可重试 5xx 计入**（审查修订）。Royal `/health` 恒返回 `"ok"`（`lib.rs:325-327`），Consul passing 不能代表业务 500。连续 5 次「传输错误或 500/502/503/504」→ open，从 RR 摘除。4xx 与 2xx 重置失败计数。半开：每 30s **一个**探测请求，用 `compare_exchange` 抢探测名额（失败的实例保持 open，其它请求仍跳过）。池空 → `StoreError::Backend("no royal available")`（chat 回 99）。不引 `tower`。
3. **选择策略轮询（round-robin）**：`AtomicUsize` 递增取模健康实例列表。拒绝按 least-connections——reqwest 内部连接池已按 host 复用，RR 足够均匀。HMAC 签名与实例无关（同一 secret），无粘性需求。
4. **好友/黑名单/exists 缓存：键带 `SocialQueryKind`，对称账号排序，增量淘汰**（审查修订）。
   - 键：`(kind, app, a, b)`，`kind ∈ {Friend, BlockedEither}`。`a,b` 经 `ordered_pair`（与 PG `is_friend` 一致）。**禁止** friend 与 block 共用 `(app,a,b)`——私聊先 `is_blocked_either` 再 `is_friend`，false 命中会把「未拉黑」写成「不是好友」。
   - TTL 30s + jitter `±20%`，容量 10_000。超限 **增量**删最旧 10%（按过期时间），禁止整表清空（避免雪崩）。miss 用 single-flight（同键并发只打一次 Royal）。
   - 写穿透：本 Chat 上 `request/accept/reject/remove/block/unblock` 成功后 evict 双向键。
   - **不提供 stale-if-error**：过期条目不在 Royal 挂时继续当授权依据。目录 RPC 失败仍 `SystemException`。跨 Chat 30s TTL 陈旧是负载权衡，不是 fail-open；不把「删好友后对端仍能发 30s」当成可用性特性宣传。
5. **多实例 compose 的前置：Snowflake init 失败退出**（审查修订 / G-22 半边）。删掉 `royal/src/main.rs:177-183` 与 `chat/src/lib.rs:124-129` 的 `SequenceIdGen` 降级；`try_new` 失败或 `resolve_snowflake_node` 得到非法值 → 进程退出。`royal-2`：`KIM_SERVICE_ID=royal-2`、`KIM_SNOWFLAKE_NODE=11`。`ROYAL_URL` 仍是 bootstrap。**未做启动失败之前禁止上 royal-2。**
6. **不做**：自定义网关协议合并 RPC（next-stage 明确拒绝）；好友缓存放 Royal 侧（写路径在 Royal 但读热点在 Chat，缓存就近放读方）。

### 类型定义

```rust
// services/chat/src/royal_pool.rs（新）
pub struct RoyalClient {
    base: String,
    http: reqwest::Client,
    hmac_secret: String,
    fails: Arc<AtomicU32>,        // 连续传输失败
    opened: Arc<AtomicBool>,      // 熔断 open
    half_open_at: Arc<AtomicU64>, // unix ms，半开探测时间门
}

pub struct RoyalPool {
    clients: RwLock<Vec<Arc<RoyalClient>>>,
    rr: AtomicUsize,
    bootstrap: Option<Arc<RoyalClient>>,  // ROYAL_URL 静态（无发现时唯一来源）
    refresh: Duration,                    // 10s
}

impl RoyalPool {
    /// naming: None → 静态单地址（本机/e2e 与今完全一致）
    pub fn new(royal_url: Option<&str>, naming: Option<Arc<dyn Naming>>,
               hmac: &str) -> Result<Self, StoreError>;
    pub fn spawn_refresh(self: &Arc<Self>);              // 后台 find 轮询
    pub fn pick(&self) -> Result<Arc<RoyalClient>, StoreError>; // RR + 健康过滤 + 半开
    pub(crate) fn report_success(&self, c: &RoyalClient);
    pub(crate) fn report_failure(&self, c: &RoyalClient); // 计数/熔断/半开调度
}
```

```rust
// services/chat/src/social_cache.rs（新）
enum SocialQueryKind { Friend, BlockedEither }

pub struct CachedSocial {
    inner: Arc<dyn SocialDirectory>,
    entries: Mutex<HashMap<(SocialQueryKind, String, String, String), (bool, Instant)>>,
    inflight: Mutex<HashMap<(SocialQueryKind, String, String, String), oneshot::Sender<bool>>>,
    ttl: Duration,        // 30s ± 20% jitter
    cap: usize,           // 10_000
}
impl CachedSocial {
    pub fn wrap(inner: Arc<dyn SocialDirectory>) -> Arc<Self>;
}
#[async_trait] impl SocialDirectory for CachedSocial {
    // is_friend / is_blocked_either：分 kind 缓存；命中且未过期返回；miss single-flight
    // request/accept/reject/remove/block/unblock：inner 成功后 evict 双向 Friend+Blocked 键
    // list_friends / incoming / list_blocked：直通
}
```

`send_pb` / `post_maybe_empty` 改为 `RoyalPool` 的方法（或 RoyalClient 保留原方法、Pool 包装选择+上报），`HttpMessageStore` 等四适配器持有的 `client: RoyalClient` 改 `pool: Arc<RoyalPool>`。

### 使用示例

```rust
// services/chat/src/main.rs（royal 分支）
if let Some(royal) = royal_url_from_env_or_cfg(&cfg.this.royal_url) {
    let naming = consul.as_deref().map(open_naming_royal_reader);  // 复用进程内 naming
    let pool = Arc::new(RoyalPool::new(Some(&royal), naming, &hmac)?);
    pool.spawn_refresh();
    let (store, groups, users, social) = http_backends_with_pool(pool)?;
    let social = Arc::new(CachedSocial::wrap(social));   // 缓存包装在最外层
    // users.exists 缓存：CachedUserDirectory::wrap(users)（同款，仅 exists 方法）
}
```

## Phased Implementation

### Phase 1: `RoyalPool`（传输层池化 + 熔断）

- **File: `services/chat/src/royal_pool.rs`（新）** — 如上类型；`pick()` 过滤 open 实例，半开 CAS 放行一个探测。`report_failure`：连接错误 / 超时 / 500/502/503/504 计数；4xx/2xx `report_success`。测试：坏实例 5 次 503 后被摘；半开窗口内只有一个探测。
- **File: `services/chat/src/royal.rs`**
  - `RoyalClient` 字段加熔断三元组；`send_pb` / `post_maybe_empty` 移到 `RoyalPool`（内部 `pick` → 执行 → success/failure 上报；退避逻辑沿用 B4）。
  - `http_backends_with_hmac*` 保留旧签名（内部建静态单地址 Pool）+ 新 `http_backends_with_pool(pool)`。
- **File: `services/chat/src/lib.rs`** — `pub mod royal_pool;`。
- **File: `services/chat/src/royal_pool.rs`（tests mod）** — 假 axum server：2 实例一好一坏，断言 5 次后坏实例被摘、请求全落好实例；好实例恢复（半开）后回池；无实例可用时 `Backend` 错误。
- 验证：`cargo test -p chat royal && cargo clippy -p chat -- -D warnings`。

### Phase 2: Consul 发现接线

- **File: `services/chat/src/main.rs`**
  - royal 分支：进程已有 `naming`（Consul 时）——检查 `main.rs` 现 naming 构造顺序（naming 在 royal 分支之后建）；调整为 royal 分支可引用同一 `Arc<dyn Naming>`。`RoyalPool::new` 传 `Some(naming)`；`spawn_refresh` 起后台 task。
  - `ROYAL_URL` 语义写注释：bootstrap + 无 Consul 时唯一地址。
- **File: `services/chat/src/royal_pool.rs`** — refresh task：`Naming::find("royal", &[])` → 构造 `RoyalClient` 列表（`http://{public_address}:{public_port}`，meta `protocol` 非 http 时跳过）→ 全量替换（保留现存实例的熔断状态：按 base 字符串对齐合并）。
- 验证：本机无 Consul 路径回归（`ROYAL_URL` 单地址）；`cargo test -p chat`。

### Phase 3: 好友/黑名单/exists 缓存

- **File: `services/chat/src/social_cache.rs`（新）** — `CachedSocial`（如上）。测试：friend/block 键不互相覆盖；对称账号命中；TTL jitter 过期；写穿透 evict；超限删最旧 10%；同键 concurrent miss 只打一次 inner。
- **File: `services/chat/src/users_cache.rs`（新，或并入 social_cache.rs）** — `CachedUserDirectory` 只包 `exists`（其余直通）；`profile` 不缓存（inbox N+1 属 B7，彼处另行决策）。
- **File: `services/chat/src/main.rs`** — royal 分支与 Memory 分支都包缓存（Memory 路径零成本，行为一致便于测试）；`ChatHandler::with_social` 收到的是缓存包装。
- **File: `services/chat/src/talk.rs`（tests）** — 现有 seed_friends 测试改为注入可计数 FakeSocial，断言同对话第二次 talk 不再打 `is_friend`。
- 验证：`env -u REDIS_URL cargo test -p chat && cargo clippy -p chat -- -D warnings`。

### Phase 3b: Snowflake 启动失败（G-22 半边，royal-2 前置）

- **File: `services/royal/src/main.rs`** — `SnowflakeGen::try_new` 失败 `return Err`；非法 node 不回退 1（`resolve_snowflake_node` 对 `>31` 改 `Err` 或 main 里检查）。
- **File: `services/chat/src/lib.rs` / `idgen.rs`** — 同款：生产路径失败即退出；测试仍可用 `SequenceIdGen`。
- 验证：设非法 node 的进程立刻退出。

### Phase 4: compose 多实例 + 文档（Phase 3b 之后）

- **File: `deploy/compose.yml`** — `royal-2` 服务块（复制 royal，改 `KIM_SERVICE_ID=royal-2`、`KIM_SNOWFLAKE_NODE=11`——注意 snowflake node 经 env `KIM_SNOWFLAKE_NODE`（chat 有此 env；royal 走 toml `snowflake_node` + env 覆盖，核对 royal `resolve_snowflake_node` 的 env 名，同款补 `KIM_SNOWFLAKE_NODE=11`）；不映射端口）。
- **File: `deploy/royal.toml`** — 注释多实例 node 分配（10/11…）。
- **File: `docs/group-royal.md`** — Royal 发现/池化/熔断形状；Chat 侧缓存行为与 30s 陈旧窗口。
- **File: `docs/production-gaps.md`** — G-16 关闭（Royal 不再 SPOF；Redis/PG/Consul 单节点另行运维，不属本条）。
- **File: `docs/impl/README.md`** — B5 记录。
- 验证：全量 fmt/clippy/test；compose `docker compose config` 校验语法（无 VPS 时静态校验）。

## Architectural Notes

- **熔断覆盖可重试 5xx**：`/health` 恒 ok，不能靠 Consul 摘 500 实例。半开 CAS 保证每窗口一个探测，避免惊群打满坏实例。
- **缓存不做 stale-if-error**：TTL 内命中减载；过期或 Royal 失败走错误路径。跨 Chat 最多 30s 见旧 friend/block 结果，是 TTL 缓存的固有窗口，不是 fail-open 授权。
- **HMAC nonce 与多实例**：`HmacNonceGuard` 是 Redis NX（royal 侧防重放）——Chat 侧换实例重试同一请求时 nonce 复用会被拒？核对：nonce 由 Chat 每次请求生成（`sign_internal_hmac` 每次新 nonce），重试是新签名新 nonce，无冲突。**保留每实例独立 nonce 生成，不做跨重试 nonce 复用**。
- **不改**：写扩散语义、ACK、`sdk/*`、Royal handler 逻辑、Consul ACL 模型（royal 复用 `CONSUL_TOKEN_CHAT` 级别的读 token 即可——现有 token 已可 find）。
- **新依赖**：无（moka/lru 不引）。
- **回滚**：`ROYAL_URL` 静态即回到单实例行为；缓存包装配置开关 `KIM_SOCIAL_CACHE_TTL_MS=0` 关闭（0 = 直通）。

## File Change Summary

- `services/chat/src/royal_pool.rs` -- 新：RoyalPool + 熔断 + 发现刷新
- `services/chat/src/social_cache.rs` -- 新：CachedSocial + CachedUserDirectory
- `services/chat/src/royal.rs` -- 传输函数移入池、backends 工厂扩展
- `services/chat/src/lib.rs` -- 模块导出
- `services/chat/src/main.rs` -- 池接线 + 缓存包装
- `services/chat/src/idgen.rs` / `services/chat/src/lib.rs` -- snowflake 失败退出
- `services/royal/src/main.rs` -- snowflake 失败退出（royal-2 前置）
- `services/chat/src/talk.rs` -- 缓存行为测试
- `deploy/compose.yml` -- royal-2 实例
- `deploy/royal.toml` -- node 分配注释
- `docs/group-royal.md` -- 发现/熔断/缓存形状
- `docs/production-gaps.md` -- G-16 关闭
- `docs/impl/README.md` -- B5 记录
