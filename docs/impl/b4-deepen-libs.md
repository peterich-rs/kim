# Deepen Redis, SQLx, and Royal HTTP Client Usage on Hot Paths

| 字段 | 值 |
|---|---|
| 状态 | Draft（审查修订：六个 ConnectionManager、目录校验总预算、迁移独立连接） |
| 日期 | 2026-09-02 |
| 覆盖 | G-33（热路径半边：pipeline 复核、ConnectionManager 超时、sqlx 会话/池参数、Royal 重试退避） |
| 父规格 | [next-stage.md](./next-stage.md) B4、[production-gaps.md](../production-gaps.md) G-33 |

## Breaking Change Notice

无公共 API 变更。`PoolConfig` / `PoolOpts` 增加字段。`PoolOpts { ... }` 字面量目前 5 处（`store/postgres.rs` 测试 2、`store/mod.rs` 2、`directory/postgres.rs` 1），加上 `PoolConfig` 的 `Default`/拼装，**全部同 PR 改齐**。

## Feasibility Assessment

- `get_locations` 多账号路径**已是 pipeline**（`crates/kim-session/src/redis.rs:248-254`，`pipe().cmd("HVALS")` 一次往返，失败逐个回退）——gaps「逐账号 HVALS」的描述已过时，本切片复核该路径后**不重写**，仅补单账号与 pipeline 行为锁测试。
- redis 0.32.7 提供 `Client::get_connection_manager_with_config`（`ConnectionManagerConfig::set_connection_timeout` / `set_response_timeout`），已核实存在于本机 registry 源码。
- sqlx `PgPoolOptions` 支持 `min_connections` / `max_lifetime` / `acquire_timeout`；`statement_timeout` 经 `options(.options([("statement_timeout", …)]))`（`PgConnectOptions`）设置，`PgPoolOptions::options` 接受它。
- `RoyalClient`（`services/chat/src/royal.rs:127-163`）现 3 次立即重试无退避；`send_pb` / `post_maybe_empty` 两个循环改造点集中。
- **Fully feasible.**

## Current Surface Inventory

- `crates/kim-session/src/redis.rs:45` — `RedisSessionStore::open`：裸 `ConnectionManager::new`
- `crates/kim-session/src/redis.rs:236-266` — `get_locations`：单账号 `hash_locs`；多账号 pipeline，失败后 `out.extend(self.hash_locs(account).await?)`（**已经**首错短路，不是缺口）
- `services/gateway/src/lib.rs:161` — `RevokeStore::open`：裸 `ConnectionManager::new`
- `services/chat/src/store/redis_ack.rs:14` — ACK 热路径裸 `ConnectionManager::new`
- `services/chat/src/hmac_nonce.rs:67` — HMAC nonce 裸 `ConnectionManager::new`
- `services/royal/src/revoke.rs:94` — 吊销/epoch 裸 `ConnectionManager::new`
- `services/royal/src/device.rs:308` — device hot 裸 `ConnectionManager::new`
- `services/chat/src/store/mod.rs:1167-1182` — `PoolConfig` 三字段
- `services/chat/src/store/postgres.rs:22-40` — `PoolOpts` 三字段；`connect_pool` 同一 pool 上 `sqlx::migrate!`
- `services/chat/src/talk.rs:82-131` — 私聊顺序 `exists` → `is_blocked_either` → `is_friend`（3 次 Royal RPC，无总 deadline）
- `services/chat/src/royal.rs:31,75,92-95,126-157` — `RETRIES=3` 立即重试；reqwest 5s；`retry_http` 5xx 且非 503
- `services/chat/src/store/mod.rs:33` — `LIST_LOCATIONS_BUDGET = 500ms`（只包 insert 里的 Redis location，**不**包上面 3 次 RPC）
- `services/chat/src/talk.rs:291` — push collect 200ms（同样不包目录 RPC）
- `deploy/chat.toml` — 现有三键
- `crates/kim-naming/src/consul.rs:26-32` — Consul reqwest 有 timeout；watch 不重试，不动

## Design

### 决策

1. **六个 `ConnectionManager::new` 全部走同一 helper**（审查修订）。漏掉 ACK / nonce / 吊销 / device hot 就不能宣称 G-33 的 Redis 超时缺口已关：
   - `crates/kim-session/src/redis.rs:45`
   - `services/gateway/src/lib.rs:161`
   - `services/chat/src/store/redis_ack.rs:14`
   - `services/chat/src/hmac_nonce.rs:67`
   - `services/royal/src/revoke.rs:94`
   - `services/royal/src/device.rs:308`
   helper：`kim_session::open_connection_manager(url)`，`connection_timeout=3s`、`response_timeout=3s`。每个点测：连不上、连上后命令超时。
2. **`get_locations` 不重写，不把「首错短路」当新工作**：`out.extend(self.hash_locs(account).await?)` 已经在第一个错误返回。本切片只补「已是 pipeline + 回退首错」的锁测试。
3. **sqlx 会话级 `statement_timeout`，池参数可配；迁移用独立连接**（审查修订）。运行时 `statement_timeout=5s` / `idle_in_transaction_session_timeout=15s` 经 `PgConnectOptions` 下发。`sqlx::migrate!` **不要**走带 5s timeout 的业务池：`connect_pool` 先用无 statement_timeout 的短命连接跑 migrate，再建立业务池。`PoolOpts` 五个字面量同 PR 改齐。`min_connections` 默认 0，`max_lifetime` 默认 30min。
4. **Royal 重试有端到端 deadline，不是 4s×3**（审查修订）。私聊目录校验是顺序三次 RPC（`talk.rs:82-131`），每次最多 3 次尝试。单次 reqwest 4s 时 Royal 全挂可到 ~3×3×4s + 退避。改为：
   - `send_pb` / `post_maybe_empty` 接受「剩余预算」；每次尝试 `timeout = remaining.min(per_attempt)`（per_attempt 默认 400ms）
   - `do_user_talk` 目录阶段一个 `directory_deadline`（默认 800ms），三次 RPC 共享；超时 → `SystemException`，与今 Redis 挂一致
   - 退避 `100ms * 2^n + jitter(0..50)`，n 从 0，仍最多 3 次，但被剩余预算截断（预算不够则不再睡、不再试）
   - B5 池化后每次重试 `pick()` 新实例
   - `retry_http` 判定不变（503 不重试）
5. **royal 的 PG 池与 chat 统一走 `PoolConfig`**：`open_devices` 与 `open_pg_backends` 都可配。

### 类型与用法

```rust
// crates/kim-session/src/redis.rs
use ::redis::aio::ConnectionManagerConfig;

pub async fn open_connection_manager(url: &str) -> Result<ConnectionManager, SessionError> {
    let client = Client::open(url).map_err(redis_err)?;
    let config = ConnectionManagerConfig::new()
        .set_connection_timeout(Duration::from_secs(3))
        .set_response_timeout(Duration::from_secs(3));
    client.get_connection_manager_with_config(config).await.map_err(redis_err)
}
// RedisSessionStore::open / gateway RevokeStore::open（pub(crate) 复制或 re-export）都走它
```

```rust
// services/chat/src/store/mod.rs
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,          // 新，默认 0
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,        // 新，默认 30min
    pub statement_timeout: Duration,   // 新，默认 5s
    pub idle_in_tx_timeout: Duration,  // 新，默认 15s
}

// services/chat/src/store/postgres.rs
let opts = PgConnectOptions::from_str(url)
    .options([
        ("statement_timeout", &stmt_ms.to_string()),
        ("idle_in_transaction_session_timeout", &idle_tx_ms.to_string()),
    ])?;
PgPoolOptions::new()
    .max_connections(..).min_connections(..)
    .acquire_timeout(..).idle_timeout(..).max_lifetime(..)
    .connect_with(opts)
```

```rust
// services/chat/src/royal.rs —— 重试退避
async fn backoff(attempt: usize) {
    let base = 100u64 << attempt.min(2);          // 100 / 200 / 400ms 档
    let jitter = rand::random::<u64>() % 50;
    tokio::time::sleep(Duration::from_millis(base + jitter)).await;
}
// send_pb / post_maybe_empty 循环内：非 success 且 retry_http(status) 或 transport err
// → if attempt + 1 < RETRIES { backoff(attempt).await } else break
```

（`rand` 已在 workspace（royal 用 rand_core）；chat 侧改用 `uuid::Uuid` 无 rand——直接 `std::time::SystemTime::now().subsec_millis() % 50` 作 jitter，不引新依赖。）

## Phased Implementation

### Phase 1: Redis ConnectionManager 超时（六个点）

- **File: `crates/kim-session/src/redis.rs`** — `pub async fn open_connection_manager(url)`；`RedisSessionStore::open` 走它。`get_locations` 不改行为，加测试锁「pipeline 失败后 hash_locs 首错即返回」。
- **File: `crates/kim-session/src/lib.rs`** — 导出 helper。
- **File: `services/gateway/src/lib.rs`** — `RevokeStore::open`
- **File: `services/chat/src/store/redis_ack.rs`** — `RedisAckIndex::open`
- **File: `services/chat/src/hmac_nonce.rs`** — Redis nonce open
- **File: `services/royal/src/revoke.rs`** — `RedisRevocation::open`
- **File: `services/royal/src/device.rs`** — `RedisDeviceHot::open`
- 验证：每个点至少编译路径覆盖；`cargo clippy -p kim-session -p gateway -p chat -p royal -- -D warnings`。

### Phase 2: sqlx 池与 statement timeout

- **File: `services/chat/src/store/mod.rs`** — `PoolConfig` 扩字段 + `Default` 更新。
- **File: `services/chat/src/store/postgres.rs`** — 业务池 `connect_with(PgConnectOptions + statement_timeout)`；migrate 用**另一条**无 statement_timeout 的连接（或临时把 timeout 调到 10min 再还原）。五个 `PoolOpts { ... }` 字面量补新字段。
- **File: `services/chat/src/main.rs`** — `SelfSection` 加 `db_min_connections` / `db_max_lifetime_secs` / `db_statement_timeout_ms` / `db_idle_in_tx_timeout_secs`（serde default 对齐默认值）；拼 `PoolConfig` 处补字段。
- **File: `services/royal/src/main.rs`** — `open_devices` 与 `open_pg_backends` 走同一 `PoolConfig`（royal.toml 同款键；`SelfSection` 补字段）。
- **File: `deploy/chat.toml` / `deploy/royal.toml`** — 新键显式列出（值=默认），运维可见。
- 验证：`env -u REDIS_URL cargo test -p chat`（PG 路径由 e2e mock 覆盖编译；如仓库有 `DATABASE_URL` 集成测试开关则本地跑）+ `cargo clippy -p chat -p royal -- -D warnings`。

### Phase 3: Royal 重试退避 + 目录总预算

- **File: `services/chat/src/royal.rs`**
  - `backoff(attempt)`（SystemTime jitter，不引依赖）。
  - `send_pb` / `post_maybe_empty`：传入 `deadline: Instant`；每次 `http` 请求 timeout = `deadline.saturating_duration_since(now).min(PER_ATTEMPT)`；预算耗尽不再重试。
  - 测试：假 axum 500 两次后 200；另测 deadline 已过则一次都不睡。
- **File: `services/chat/src/talk.rs`** — `do_user_talk` 在 exists/block/friend 外包 `directory_deadline = Instant::now() + 800ms`，传入 Royal 调用（`HttpUserDirectory` / `HttpSocialDirectory` 需要能把 deadline 传进 `send_pb`：用 `RoyalClient` 上的 `with_deadline` 或 task-local。选 `RoyalClient` 加 `deadline: Option<Instant>` 的 scoped 方法，避免全局状态）。
- 验证：`cargo test -p chat royal && cargo clippy -p chat -- -D warnings`。

### Phase 4: 文档与验收

- **File: `docs/perf.md`** — 池参数、statement_timeout、Redis response timeout、Royal 退避形状。
- **File: `docs/production-gaps.md`** — G-33 表内 redis/sqlx/reqwest 行标已关（pipeline 复核结论写明「本就 pipeline，回退短路」）；axum tower-http 行保留（属 Royal/Router REST 面，不在本切片）。
- **File: `docs/impl/README.md`** — B4 记录。
- 验证：全量 `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && env -u REDIS_URL cargo test --workspace`。

## Architectural Notes

- **不换 fred、不换 `prometheus-client`、不做 sqlx 离线 `.sqlx`**（子任务，next-stage 明确排除）。
- **`get_locations` 单账号路径不加 timeout 包装**：response_timeout 已在 ConnectionManager 层生效。
- **目录 RPC 总预算与 insert 的 500ms location 预算分开**：前者拦 Royal 挂死 talk；后者拦 Redis SCAN/HVALS。
- **tower-http / 显式 tokio Builder / otel**：不在 B4（gaps 把 axum 中间件列在 G-33 但 next-stage B4 未含；排 B6 评估）。
- **mirror 双写 fail-open 不动**（`kim-session/src/dual.rs`），G-24 单列。
- **Semver**：`PoolConfig` / `PoolOpts` 字段增加，全部字面量同 PR 改齐。

## File Change Summary

- `crates/kim-session/src/redis.rs` -- open_connection_manager + 超时
- `crates/kim-session/src/lib.rs` -- 导出 helper
- `services/gateway/src/lib.rs` -- RevokeStore::open
- `services/chat/src/store/redis_ack.rs` -- ACK Redis 超时
- `services/chat/src/hmac_nonce.rs` -- nonce Redis 超时
- `services/royal/src/revoke.rs` -- 吊销 Redis 超时
- `services/royal/src/device.rs` -- device hot Redis 超时
- `services/chat/src/store/mod.rs` -- PoolConfig 扩字段 + 五处 PoolOpts 对齐
- `services/chat/src/store/postgres.rs` -- 业务池 statement_timeout；migrate 独立连接
- `services/chat/src/directory/postgres.rs` -- PoolOpts 字面量
- `services/chat/src/main.rs` -- toml 新键
- `services/chat/src/royal.rs` -- deadline + backoff
- `services/chat/src/talk.rs` -- 目录校验 800ms 总预算
- `services/royal/src/main.rs` -- 池配置统一 PoolConfig
- `deploy/chat.toml` -- 新池键
- `deploy/royal.toml` -- 新池键
- `docs/perf.md` -- 参数形状
- `docs/production-gaps.md` -- G-33 部分关闭
- `docs/impl/README.md` -- B4 记录
