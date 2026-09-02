# Roll Out Pending Receipt

| 字段 | 值 |
|---|---|
| 状态 | 执行中 |
| 日期 | 2026-09-02 |
| 覆盖 | G-03 / G-04 / G-10 运维关门闩 |
| 父规格 | [next-stage.md](./next-stage.md)、[reliable-delivery.md](../reliable-delivery.md) |

## Breaking Change Notice

无公共 crate API 变更。compose 默认开关 **保持 0**。从 [production-gaps.md](../production-gaps.md) 删 G-03 / G-04 / G-10 必须在目标栈三条条件同时成立之后，另提交文档。

## Feasibility Assessment

ACK 模型已合入。`KIM_REQUIRE_JTI` / `KIM_PENDING_RECEIPT_ROYAL` / `KIM_PENDING_RECEIPT_CHAT` 已在 compose 拆开。Royal 已有 `RedisSessionStore::count_empty_jti_locations`。本切片不改热路径。**Fully feasible.**

## Current Surface Inventory

- `deploy/compose.yml` — 三开关默认 0
- `deploy/kim.env.example` — 运维模板
- `crates/kim-session/src/redis.rs` `count_empty_jti_locations` — SCAN `login:loc:v2:*`
- `services/royal/src/main.rs` — 每 60s 打 `kim_location_without_jti`
- `docs/deploy.md` 步骤 1–8 — 已有顺序，缺可执行脚本

## Design

1. **不改 Rust 热路径。** 默认 0 是门闩，禁止把 compose 默认改成 1。
2. **SCAN 复用现有 decode。** 不要在 shell 里重写 Location blob。
3. **关 gaps 与脚本 PR 拆开。** 脚本可先合；删 G-xx 等生产（或目标栈）验收。

关闭条件（同时满足）：

1. Gateway **持续** `KIM_REQUIRE_JTI=1`
2. SCAN `login:loc:v2:*` 空 `jti` = 0
3. **先** Royal `KIM_PENDING_RECEIPT=1`，**再** Chat `=1`。禁止 Chat=1 且 Royal=0

## Phased Implementation

### Phase 1: SCAN 工具

- `crates/kim-session/examples/scan_empty_jti.rs` — `REDIS_URL` 或 argv，打印 `empty_jti=N`，N=0 则 exit 0
- `services/royal` `--scan-empty-jti` — 容器内一次性子命令，读同一 `REDIS_URL`
- `deploy/scan-empty-jti.sh` — 优先 `docker compose exec royal`，否则 `cargo run --example`

### Phase 2: Runbook

- `docs/deploy.md` / `docs/reliable-delivery.md` — 可执行翻转与回滚（Chat 先 0）
- `deploy/kim.env.example` — 注释指向脚本，默认仍 0

### Phase 3: 验收后文档（另提交）

走完三条后才从 gaps 删 G-03 / G-04 / G-10。漏 Push 仍是 G-14。

## Architectural Notes

- Semver：无
- 不改：`services/chat` ACK、`sdk/*`、Snowflake 当 ACK、compose 默认值
- 回滚：Chat 先 0，再 Royal 0

## File Change Summary

- `crates/kim-session/examples/scan_empty_jti.rs` -- SCAN 空 jti
- `crates/kim-session/Cargo.toml` -- example 声明
- `deploy/scan-empty-jti.sh` -- 调用 SCAN
- `deploy/kim.env.example` -- 注释顺序
- `docs/deploy.md` / `docs/reliable-delivery.md` -- runbook
- `services/royal/src/main.rs` -- `--scan-empty-jti`
