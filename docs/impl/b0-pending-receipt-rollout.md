# Roll Out Pending Receipt

| 字段 | 值 |
|---|---|
| 状态 | 代码与 SCAN fail-closed 已合入（#72）。剩：**运维执行** + **验收后文档 PR**。无代码变更 |
| 日期 | 2026-09-03 |
| 覆盖 | G-03 / G-04 / G-10 运维关门闩 |
| 父规格 | [next-stage.md](./next-stage.md)、[reliable-delivery.md](../reliable-delivery.md) |

## Breaking Change Notice

无。本切片**不改任何代码**：不改 ACK 热路径、不改 `sdk/*`、不把 compose 默认改成 1。
从 [production-gaps.md](../production-gaps.md) 删 G-03 / G-04 / G-10 属**另提交的文档 PR**，且必须三条关闭条件在目标栈同时成立之后。

## Feasibility Assessment

代码侧全部就绪，已逐项核实：`LocationScan` 四计数与 fail-closed 门闩在 `crates/kim-session/src/lib.rs:40-58`、`redis.rs:125-168`；`royal --scan-empty-jti` 打印四项并按 `empty_jti_gate_code` 退出（`services/royal/src/main.rs:83-96`）；example 同语义；`deploy/scan-empty-jti.sh` 注释与 fallback 链就位。compose 三开关已拆：gateway `KIM_REQUIRE_JTI`（compose.yml:327）、royal/royal-2 `KIM_PENDING_RECEIPT_ROYAL`（:157/:203）、chat/chat-gray `KIM_PENDING_RECEIPT_CHAT`（:243/:285），默认全 0。Gateway 在进程启动时读 env（`services/gateway/src/lib.rs:363`），「持续 =1」即 kim.env 保持该值跨重启。runbook 步骤 1–8 已在 [deploy.md](../deploy.md)。**Fully feasible —— 剩余是执行与验收，不是开发。**

## Current Surface Inventory

- `deploy/compose.yml:327` — gateway `KIM_REQUIRE_JTI: ${KIM_REQUIRE_JTI:-0}`（只读引用，不改）
- `deploy/compose.yml:157,203` — royal / royal-2 `KIM_PENDING_RECEIPT: ${KIM_PENDING_RECEIPT_ROYAL:-0}`（不改）
- `deploy/compose.yml:243,285` — chat / chat-gray `KIM_PENDING_RECEIPT: ${KIM_PENDING_RECEIPT_CHAT:-0}`（不改）
- `deploy/kim.env.example:33-39` — 三开关模板与顺序注释（不改；VPS 的 `kim.env` 才是生效处）
- `deploy/scan-empty-jti.sh` — 门闩脚本：优先 `compose exec royal royal --scan-empty-jti`，fallback cargo example
- `services/royal/src/main.rs:275-285` — Royal 每 60s 打 `kim_location_without_jti` / invalid / wrong_type / scanned 日志（观察点，不改）
- `docs/deploy.md` 步骤 1–8 — 翻转与回滚顺序（本切片补验收命令引用，不改语义）
- `docs/production-gaps.md` — G-03（:100）/ G-04（:130）/ G-10（:160）三节 + 顶部列表与修复顺序表（closeout PR 删除）

## Design

1. **不改代码，不改 compose 默认。** 三开关默认 0 是门闩；翻转只发生在 VPS 的 `kim.env`。本切片产物 = 执行记录 + 验收后文档 PR。
2. **关闭条件（同时满足，缺一不可）**：
   1. Gateway **持续** `KIM_REQUIRE_JTI=1`
   2. SCAN `login:loc:v2:*`：`empty_jti=0` **且** `invalid=0` **且** `wrong_type=0`，命令 exit 0
   3. **先** Royal `KIM_PENDING_RECEIPT=1`，**再** Chat `=1`。禁止 Chat=1 且 Royal=0
3. **回滚顺序**：Chat 先 0，再 Royal 0。`KIM_REQUIRE_JTI` 不回滚（无 jti JWT 本就要求重登）。
4. **验收只认门闩输出，不认抽样 gauge**（不用 talk 路径估 loc 健康）。SCAN 必须打印：

   ```text
   empty_jti=0 invalid=0 wrong_type=0 scanned=N
   ```

   且 exit 0。Royal 日志的 60s 扫描行作为持续观察补充。

## Phased Implementation

### Phase 1: 运维执行（VPS，按 deploy.md 步骤 1–8）

每步验收命令（在部署机执行；compose 带 `--env-file deploy/kim.env`）：

- **步骤 4（Gateway =1）**：改 kim.env → `docker compose up -d gateway` →
  - `docker compose exec gateway printenv KIM_REQUIRE_JTI` 输出 `1`
  - 存量无 jti 会话被拒（重登后恢复）；此后每次重启仍读 kim.env，保持开
- **步骤 5（SCAN 门闩）**：`deploy/scan-empty-jti.sh` → exit 0 且四项如上；Royal 日志出现
  `kim_location_without_jti=0 invalid=0 wrong_type=0 scanned=N`。**exit 非 0 或任一项非 0 → 停，先清理（`hash_locs` 读路径可 DEL 残留 STRING），不进步骤 6**
- **步骤 6（Royal writer 先行，两个实例一起翻）**：`KIM_PENDING_RECEIPT_ROYAL=1`（Chat 仍 0）→
  ```bash
  docker compose -f deploy/compose.yml --env-file deploy/kim.env up -d royal royal-2
  docker compose -f deploy/compose.yml --env-file deploy/kim.env exec -T royal printenv KIM_PENDING_RECEIPT
  docker compose -f deploy/compose.yml --env-file deploy/kim.env exec -T royal-2 printenv KIM_PENDING_RECEIPT
  ```
  两处都必须输出 `1`。只 `up -d royal` 会留下 `royal-2` 仍 writer=0；Chat `RoyalPool` 在健康实例间轮询，部分写请求仍走旧路径，「最近 5 分钟有 pending 行」仍可能误通过。
  - 两实例都是 `1` 之后：已知账号/设备发一条 canary，再查
    `SELECT count(*) FROM pending_delivery WHERE created_at > now() - interval '5 minutes'`
    必须有新行，且该 canary 的 receipt 行 `account`/`target_id` 对得上
- **步骤 7（Chat reader）**：`KIM_PENDING_RECEIPT_CHAT=1` → `up -d chat chat-gray` → 客户端离线拉取走 receipt 集合
- **回滚演练（可选但推荐）**：Chat 先 0，再 Royal 0，确认读路径回落高水位后按 6→7 重开

执行日志（时间、四项数字、pending 行数）留档，供 Phase 2 的 closeout PR 引用。

### Phase 2: 验收后文档 PR（三条走完，单独提交）

- **File: `docs/production-gaps.md`** — 删 G-03 / G-04 / G-10 三节；删顶部列表第 1 条与「建议修复顺序」表第 1 行的相关引用（漏 Push 仍是 G-03）；「现状」表 pending receipt 行改为已 rollout
- **File: `docs/reliable-delivery.md`** — 状态改为生产已开（Royal=1、Chat=1、`KIM_REQUIRE_JTI=1`），回滚顺序保留
- **File: `docs/deploy.md`** — 步骤 1–8 标注「已执行」，保留作为回滚 runbook
- **File: `docs/impl/README.md`** — B0 行移入「已合入」表，注明关闭日期与验收输出；「待写 / 待执行」表删 B0 行

验证：文档链接可达；`git diff --name-only` 仅 docs/。

## Architectural Notes

- **明确不改**：`services/chat` ACK 热路径、`sdk/*`、compose 默认值、SCAN 工具语义（#72 已定稿）
- **持续生效的定义**：gateway 启动读 env（非运行时开关），「持续」= kim.env 不再改回 0；bootstrap.sh preflight 不管理该键，靠 review
- **royal-2 同批**：`KIM_PENDING_RECEIPT_ROYAL` 同时作用于 royal 与 royal-2（同一 env 变量）。步骤 6 **必须** `up -d royal royal-2` 并分别 `printenv`，禁止只重启一个实例
- **SCAN 在 royal 容器内跑**：与 `count_empty_jti_locations` 同一 decode 逻辑；fallback cargo example 仅本地调试用
- **G-14 不在本切片**：Web `isRetryable` 已在客户端轨关上；漏 Push 补偿仍等本切片的三条运维条件（G-03）

## File Change Summary

- `docs/production-gaps.md` -- 删 G-03 / G-04 / G-10（验收后）
- `docs/reliable-delivery.md` -- rollout 状态改为已开
- `docs/deploy.md` -- 步骤标注已执行，保留回滚 runbook
- `docs/impl/README.md` -- B0 移入已合入表
