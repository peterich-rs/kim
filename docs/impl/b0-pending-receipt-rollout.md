# Roll Out Pending Receipt

| 字段 | 值 |
|---|---|
| 状态 | 执行中（SCAN 门闩须先改 fail-closed，再开生产开关） |
| 日期 | 2026-09-02 |
| 覆盖 | G-03 / G-04 / G-10 运维关门闩 |
| 父规格 | [next-stage.md](./next-stage.md)、[reliable-delivery.md](../reliable-delivery.md) |

## Breaking Change Notice

无公共 crate API 变更。compose 默认开关 **保持 0**。从 [production-gaps.md](../production-gaps.md) 删 G-03 / G-04 / G-10 必须在目标栈三条条件同时成立之后，另提交文档。

`count_empty_jti_locations` 的退出语义变严：WRONGTYPE / 解码失败不再当「已检查且安全」。调用方（`--scan-empty-jti`、example、shell）跟新输出格式。

## Feasibility Assessment

ACK 模型已合入。三开关已在 compose 拆开。SCAN 工具已有，但当前实现会把损坏数据当成安全状态（见决策 2）。本切片仍不改 ACK 热路径。**Feasible with caveats: SCAN 必须先改 fail-closed。**

## Current Surface Inventory

- `deploy/compose.yml` — 三开关默认 0
- `deploy/kim.env.example` — 运维模板
- `crates/kim-session/src/redis.rs:114-149` — `count_empty_jti_locations`：SCAN `login:loc:v2:*`；`HVALS` 遇 WRONGTYPE `continue`；`Location::decode` 失败走 `_ => {}`
- `crates/kim-session/src/lib.rs:39-41` — `empty_jti_gate_code(empty)` 只看 `empty != 0`
- `crates/kim-session/examples/scan_empty_jti.rs` — 打印 `empty_jti=N`，N=0 则 exit 0
- `services/royal/src/main.rs:82-96` — `--scan-empty-jti` 同样只看 `empty_jti`
- `docs/deploy.md` 步骤 1–8 — 已有顺序，缺可执行脚本

## Design

1. **不改 ACK 热路径。** 默认 0 是门闩，禁止把 compose 默认改成 1。三开关顺序与「Chat 先回滚」不变。
2. **SCAN fail-closed（审查修订）。** 现实现忽略 WRONGTYPE 和解码错误，可输出 `empty_jti=0` 而漏检旧 STRING / 损坏 blob。门闩改为输出四项，任一项非零或 Redis 错误都非零退出：
   - `scanned` — 见到的 loc key 数
   - `empty_jti` — 成功解码且 `jti` 为空（含两字段旧 blob：`Location::decode` 对缺 jti 字段返回空串，这是门闩要拦的对象，继续计入）
   - `invalid` — `Location::decode` 返回 `Err`（截断、非法 UTF-8）
   - `wrong_type` — `HVALS` 报 WRONGTYPE（pre-hash STRING 残留）。**不要** `continue` 当没看见
   - SCAN / HVALS 的其它 Redis 错误：直接 `Err`，exit 2
3. **关 gaps 与脚本 PR 可同批。** 但删 G-xx 必须用新门闩在目标栈跑绿之后。

关闭条件（同时满足）：

1. Gateway **持续** `KIM_REQUIRE_JTI=1`
2. SCAN `login:loc:v2:*`：`empty_jti=0` **且** `invalid=0` **且** `wrong_type=0`，且命令 exit 0
3. **先** Royal `KIM_PENDING_RECEIPT=1`，**再** Chat `=1`。禁止 Chat=1 且 Royal=0

```text
empty_jti=0 invalid=0 wrong_type=0 scanned=N
```

`empty_jti_gate_code` 改为看报告：`empty_jti | invalid | wrong_type` 任一非零 → 1；Redis 错误仍由调用方 exit 2。

## Phased Implementation

### Phase 1: SCAN fail-closed

- **File: `crates/kim-session/src/redis.rs`**
  - `count_empty_jti_locations` 返回 `LocationScan { scanned, empty_jti, invalid, wrong_type }`（或同名字段的 tuple + 文档）。WRONGTYPE 计入 `wrong_type` 后继续扫其它 key（为了把损坏面一次报全），不再 `continue` 丢弃。decode `Err` 计入 `invalid`。
  - 测试：构造 TypeError / 非法 UTF-8 blob / 两字段旧 blob / 正常带 jti 的 HASH，断言四计数与 exit code。
- **File: `crates/kim-session/src/lib.rs`** — `empty_jti_gate_code` 吃完整报告。
- **File: `crates/kim-session/examples/scan_empty_jti.rs`** / **`services/royal/src/main.rs`** — 打印四项；exit 0 仅当三项问题计数均为 0。
- **File: `deploy/scan-empty-jti.sh`** — 注释改为四项门闩。
- 验证：`cargo test -p kim-session --features redis && cargo clippy -p kim-session -p royal -- -D warnings`。

### Phase 2: Runbook

- `docs/deploy.md` / `docs/reliable-delivery.md` — 可执行翻转与回滚（Chat 先 0）；SCAN 必须 exit 0 且四项打印如上。
- `deploy/kim.env.example` — 注释指向脚本，默认仍 0

### Phase 3: 验收后文档（另提交）

走完三条后才从 gaps 删 G-03 / G-04 / G-10。漏 Push 仍是 G-14。

## Architectural Notes

- Semver：`count_empty_jti_locations` 返回值变结构，仅 royal / example 调用，同 PR 改齐
- 不改：`services/chat` ACK、`sdk/*`、Snowflake 当 ACK、compose 默认值
- 回滚：Chat 先 0，再 Royal 0
- `hash_locs` 对 WRONGTYPE 仍可 DEL 清理（读路径）；门闩路径只计数、不删，避免扫描工具改生产数据

## File Change Summary

- `crates/kim-session/src/redis.rs` -- SCAN 四计数，WRONGTYPE/decode 失败计入
- `crates/kim-session/src/lib.rs` -- gate code 看完整报告
- `crates/kim-session/examples/scan_empty_jti.rs` -- 打印四项
- `crates/kim-session/Cargo.toml` -- example 声明（已有则不动）
- `deploy/scan-empty-jti.sh` -- 调用 SCAN
- `deploy/kim.env.example` -- 注释顺序
- `docs/deploy.md` / `docs/reliable-delivery.md` -- runbook
- `services/royal/src/main.rs` -- `--scan-empty-jti`
