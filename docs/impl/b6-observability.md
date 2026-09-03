# Close Observability Gaps: Command Whitelist, Delivery Latency, and Alert Rules

| 字段 | 值 |
|---|---|
| 状态 | Draft（白名单 29 条 + `kim_send_to_ack_seconds` 已落地；Royal RPC / backlog gauge / 告警仍开） |
| 日期 | 2026-09-02 |
| 覆盖 | G-15 后台半边（command 白名单补齐、send→ack 延迟、Royal RPC 指标、pending backlog 指标化、告警规则） |
| 父规格 | [next-stage.md](./next-stage.md) B6、[production-gaps.md](../production-gaps.md) G-15 |

## Breaking Change Notice

无公共 API 破坏。`KimMetrics::new` 签名不变；新增观测方法为加法。`deploy/prometheus.yml` 增加 rule_files 与 royal scrape job（royal 需暴露 `/metrics`——见决策 1，royal 进程新增 metrics listener，配置可选，缺省不开，非破坏）。

## Feasibility Assessment

- `COMMANDS` 白名单（`crates/kim-metrics/src/lib.rs:12-26`）停在 13 条，好友/inbox/history/user/block 共 16 条命令落 `other`——补齐是常量数组扩容，`observe_handler`（`services/chat/src/lib.rs:603`）以 `&cmd` 直查，白名单未命中走 `other` 分支的逻辑已有。
- `pending_delivery_stats`（`services/chat/src/store/postgres.rs:763`）已在 Royal 后台 task（`services/royal/src/lib.rs:347`）周期查询但只打日志——指标化只需把查询结果写 gauge，Royal 暴露 metrics 即可。
- Royal 无 `/metrics`（grep 无 KimMetrics 引用）；`kim_metrics::router(registry)` 是可 merge 的 axum Router，royal 的 axum 栈直接挂载即可。
- Prometheus 规则文件：现 `deploy/prometheus.yml` 无 `rule_files`；alerting 需要 rules 文件 + 可选 Alertmanager（本切片只做 rules 与 `deploy/prometheus/rules/`，Alertmanager 接入留运维）。
- **Feasible with caveats: send→ack 必须用 ACK 路径直方图，不能靠 pending stats 的 AVG。**

## Current Surface Inventory

- `crates/kim-metrics/src/lib.rs:12-26` — `COMMANDS` 白名单（13 条）
- `crates/kim-metrics/src/lib.rs:57+` — `KimMetrics`：channel/bytes/login/handler_duration/talk/session_not_found/dispatch_fail/heartbeat_revoke_error/mailbox_full
- `services/chat/src/lib.rs:595-605` — `observe_handler(&cmd, dt)` 调用点（唯一）
- `services/royal/src/lib.rs:340-363` — GC + stats 后台 loop（60s，日志 only）
- `services/royal/src/lib.rs:210-247` — `router(state)`：无 metrics 路由
- `services/royal/src/main.rs` — 无 metrics 监听配置
- `deploy/prometheus.yml` — scrape gateway/chat/router；无 rule_files、无 royal
- `services/chat/src/ack.rs:13+` — `do_talk_ack`（send→ack 延迟的观测终点）
- `services/chat/src/talk.rs` — insert 成功点（send 侧时间戳来源 = message send_time）
- `services/chat/src/royal.rs` — Royal RPC 调用点（B5 后经 RoyalPool，计时点集中）

## Design

### 决策

1. **Royal 暴露 `/metrics`（可选配置）**：royal.toml 加 `metrics_listen`（默认空 = 不开，与 chat/gateway 模式一致）。挂 `kim_metrics::router` + royal 专属 registry（见决策 3）。健康检查 `/health` 已有。compose royal 服务加 `9003` metrics 端口（toml 配 `0.0.0.0:9003`）。
2. **command 白名单补齐为全量 29 条 + `other` 保留**：`kim-protocol` 的 `CMD_*` 常量共 29 条（`wire.rs:13-41`），白名单数组直接对齐枚举全集；新命令遗漏时落 `other` 的兜底保留。拒绝按 `Router` 注册表动态生成——metrics crate 不该依赖 chat 的 router 实例（crate 边界：kim-metrics 不知道谁在用）。
3. **新增指标（按消费者命名，全带 `kim_` 前缀）**：
   - `kim_send_to_ack_seconds`（**HistogramVec**，labels: `service_id`,`service_name`）——父规格要直方图，不要长期平均 gauge。现 `pending_delivery_stats`（`postgres.rs:763-774`）`WHERE acked_at IS NULL`，在这上面加 `AVG(...) FILTER (WHERE acked_at IS NOT NULL)` 结果恒空；即使重写，15 天全表 AVG 会掩盖偶发延迟并周期性扫大表。改为在现有 ACK UPDATE 上：

     ```sql
     UPDATE pending_delivery
        SET acked_at = now()
      WHERE app = $1 AND account = $2 AND target_id = $3
        AND message_id = ANY($4::bigint[])
        AND acked_at IS NULL
     RETURNING EXTRACT(EPOCH FROM (acked_at - created_at))
     ```

     只观测**本次新确认**的行；不回查 `message_content`，不增加往返（仍一条 UPDATE）。`IntGauge` 会丢亚秒，必须 Histogram。兼容模式（pending 关）不产该序列。
   - `kim_royal_rpc_seconds`（histogram）+ `kim_royal_rpc_errors_total` —— B5 `RoyalPool` 传输层埋点。`path_group` 静态归并，**必须覆盖**现有 Royal 路由前缀：`message` / `group` / `friend` / `user` / `block` / `offline` / `delivery` / `inbox` / `history` / `internal` / `other`。`classify` 用 `starts_with` 前缀表，不是「无通配 match 穷尽」——`&str` 路径做不到编译期穷尽；新增端点靠测试锁前缀表与 `router()` 里的 HMAC 路径列表对齐。
   - `kim_pending_delivery_backlog` + `kim_pending_delivery_oldest_age_seconds`（gauge）——周期任务**只**保留这两项，SQL 维持 `WHERE acked_at IS NULL`。不要在 stats 里算 ack 平均。
   - `kim_offline_pull_total` —— **成功**计数。`ChatHandler::receive` 里 `observe_handler` 在 `router.serve` 之后无条件调用（`lib.rs:593-604`），不知道 offline handler 是否成功。counter 放进 `offline.rs` handler / store 成功分支，不要放 router 外层。
4. **告警规则（rules 文件，表达式用现成指标）**：
   - `KimDispatchFailHigh`：`sum(rate(kim_dispatch_fail_total[5m])) > 0.1`（5 分钟持续，for: 5m）
   - `KimHeartbeatRevokeErrors`：`sum(rate(kim_heartbeat_revoke_error_total[5m])) > 1`（for: 5m）
   - `KimMailboxFull`：`sum(rate(kim_mailbox_full_total[5m])) > 0.05`
   - `KimPendingBacklogGrowing`：`kim_pending_delivery_backlog > 100000` 且 `deriv(kim_pending_delivery_backlog[30m]) > 0`（for: 15m）
   - `KimRoyalRPCErrors`：`sum by (path_group) (rate(kim_royal_rpc_errors_total[5m])) > 0.5`
   - `KimServiceDown`：`up{job=~"gateway|chat|royal|router"} == 0`（for: 1m）
5. **不做跨进程 trace / otel**（next-stage 明确本切片不做）；`#[instrument]` 覆盖 accept/forward/talk 属 H6 记忆点，不进本切片。

### 用法示例

```rust
// services/royal/src/lib.rs —— metrics registry 与后台 loop
pub struct RoyalMetrics {
    pub registry: Registry,
    backlog: IntGauge,
    oldest_age: IntGauge,
    send_to_ack: HistogramVec, // ACK 路径 observe，不在 loop 里 set
}
// 后台 loop：只写 backlog / oldest_age
```

```rust
// services/chat/src/store/postgres.rs —— ACK 成功路径
let latencies: Vec<(f64,)> = sqlx::query_as(
    "UPDATE pending_delivery SET acked_at = now()
      WHERE ... AND acked_at IS NULL
     RETURNING EXTRACT(EPOCH FROM (acked_at - created_at))"
).bind(...).fetch_all(&self.pool).await?;
for (secs,) in latencies {
    metrics.observe_send_to_ack(Duration::from_secs_f64(secs.max(0.0)));
}
```

```rust
// services/chat/src/royal_pool.rs（B5 后）—— RPC 埋点
let started = Instant::now();
let path_group = classify(path); // message|group|friend|user|block|offline|delivery|inbox|history|internal|other
match pool.send_pb(...).await {
    Ok(_) => metrics.observe_royal_rpc(path_group, started.elapsed()),
    Err(StoreError::Backend(_)) => { metrics.on_royal_rpc_error(path_group, "transport"); ... }
    Err(StoreError::Http { status, .. }) => metrics.on_royal_rpc_error(path_group, "http"),
}
```

## Phased Implementation

### Phase 1: 白名单 + offline counter

- **File: `crates/kim-metrics/src/lib.rs`**
  - `COMMANDS` 扩到 29 条（对齐 `wire.rs` 全部 `CMD_*`）。
  - 新增 `on_offline_pull(&self)`（IntCounterVec labels: svc）。
- **File: `services/chat/src/offline.rs`** — index/content **成功**分支调 `on_offline_pull`。不改 `ChatHandler::receive` 外层。
- 验证：`cargo test -p kim-metrics -p chat && cargo clippy -- -D warnings`。

### Phase 2: Royal metrics + pending backlog + send→ack 直方图

- **File: `services/chat/src/store/postgres.rs`** — `ack` UPDATE 加 `RETURNING EXTRACT(EPOCH FROM (acked_at - created_at))`；对返回行 `observe_send_to_ack`。`pending_delivery_stats` **保持** `(count, oldest_age)`，不扩 AVG。Memory `ack` 用 `created_at`/`acked_at` Instant 差 observe。
- **File: `services/royal/src/lib.rs`** — `RoyalMetrics`；后台 loop 只写 backlog / oldest_age。ACK 走 HttpMessageStore 时 histogram 在 Chat 侧 Royal HTTP 的 store 实现里 observe（生产 insert/ack 在 Royal 进程：`open_pg_backends` 的 `PostgresMessageStore::ack`）。所以 **histogram 注入 PostgresMessageStore**（Royal 持有），不是 Chat 的 `do_talk_ack`。Chat 走 `ROYAL_URL` 时 ack RPC 成功不在 Chat 进程写 histogram，避免双计。
- **File: `services/royal/src/lib.rs`** — `router_with_metrics`。
- **File: `services/royal/src/main.rs`** — toml 加 `metrics_listen`（默认空）；非空时 `RoyalMetrics::new` + `router_with_metrics` + `tokio::spawn(kim_metrics::serve)`。
- **File: `deploy/royal.toml`** — `metrics_listen = "0.0.0.0:9003"`。
- **File: `deploy/compose.yml`** — royal 服务暴露 9003（容器网络内，不映射宿主机——prometheus 是容器内 job）。
- 验证：`cargo test -p royal -p chat && cargo clippy -- -D warnings`。

### Phase 3: Royal RPC 埋点（依赖 B5 的 RoyalPool）

- **File: `crates/kim-metrics/src/lib.rs`** — `observe_royal_rpc(group, dt)` / `on_royal_rpc_error(group, cause)`。
- **File: `services/chat/src/royal_pool.rs`** — 传输函数埋点（如上示例）。metrics 实例经 `Arc<KimMetrics>` 注入 Pool 构造（Option，None = 不埋点，测试路径零开销）。
- **File: `services/chat/src/main.rs`** — royal 分支把 `KimMetrics`（chat 已建）传给 Pool。
- 验证：`cargo test -p chat && cargo clippy -- -D warnings`。本阶段若 B5 未合入，独立可编译：埋点函数先落在 `royal.rs` 的 `send_pb`（单客户端路径），B5 合入后平移——**顺序上 B6 排在 B5 后，直接落 Pool 版**。

### Phase 4: 告警规则 + 文档

- **File: `deploy/prometheus/rules/kim.yml`（新）** — 决策 4 的 6 条规则。
- **File: `deploy/prometheus.yml`** — `rule_files: ["/etc/prometheus/rules/kim.yml"]`；royal job（`royal:9003`、`royal-2:9003`——B5 后双实例）。
- **File: `deploy/compose.yml`** — prometheus 服务挂 rules 卷。
- **File: `docs/observability.md`** — 新指标语义、labels、告警阈值依据、`avg ack latency` 在 pending receipt 关闭时 absent 的说明。
- **File: `docs/production-gaps.md`** — G-15 关闭（剩余「跨进程 trace」移「延后」表——本来就在 otel feature）。
- **File: `docs/impl/README.md`** — B6 记录。
- 验证：全量 fmt/clippy/test；`docker run --rm -v ./deploy/prometheus.yml:/etc/prometheus/prometheus.yml prom/prometheus --config.file=... --dry-run`（或 `promtool check config` 若本机可用；否则 CI 镜像验证）。

## Architectural Notes

- **send→ack 直方图来自 ACK UPDATE RETURNING**：不回查 content，不扫 15 天已确认行。周期任务只 gauge backlog / oldest-age。
- **histogram 打在写 `pending_delivery` 的进程**（生产是 Royal 的 `PostgresMessageStore`）。Chat HTTP 适配器不重复 observe。
- **指标 absent vs 0**：pending 关时 backlog 恒 0 无害；send→ack 无 RETURNING 行则不 observe。
- **`path_group` 前缀表 + 测试对齐 `router()` 路径**：`&str` 无法编译期穷尽。
- **不做**：客户端埋点、ChatList（客户端轨）、otel/tracing 跨进程、Alertmanager 部署（rules 先行，收件人属运维）。
- **新依赖**：无。

## File Change Summary

- `crates/kim-metrics/src/lib.rs` -- COMMANDS 29 条 + send_to_ack HistogramVec + offline/royal_rpc
- `services/chat/src/store/postgres.rs` -- ACK RETURNING 延迟；stats 仍 backlog/age
- `services/chat/src/store/mod.rs` -- Memory ack 观察直方图
- `services/chat/src/offline.rs` -- 成功路径 offline counter
- `services/chat/src/royal_pool.rs` -- RPC 埋点（B5 后）
- `services/chat/src/main.rs` -- metrics 注入 Pool
- `services/royal/src/lib.rs` -- RoyalMetrics + router_with_metrics + loop 写 gauge
- `services/royal/src/main.rs` -- metrics_listen 配置
- `deploy/royal.toml` -- metrics_listen
- `deploy/compose.yml` -- royal 9003、prometheus rules 卷
- `deploy/prometheus.yml` -- royal job + rule_files
- `deploy/prometheus/rules/kim.yml` -- 新：6 条告警
- `docs/observability.md` -- 指标与告警形状
- `docs/production-gaps.md` -- G-15 关闭
- `docs/impl/README.md` -- B6 记录
