# 下一阶段：后台与客户端分轨

| 字段 | 值 |
|---|---|
| 状态 | 通信层 G-29 / G-30 / G-31 / G-34 与 Royal G-16、G-33 热路径、G-15 后台半边已关。本文件只留**后台轨**剩余合同；web / mobile 另轨，不插队 |
| 日期 | 2026-09-02 |
| 父规格 | [production-gaps.md](../production-gaps.md)。已落地形状在专题文档 |

切片 3–6 与通信层硬化（读循环 lane、下行 try_send、心跳 Redis 有界宽限）的逐步补丁已执行并写回专题文档，实施稿不保留。

## 分轨原则

1. **后台切片**不改 `sdk/web`、`sdk/mobile`、聊天页 UI。协议若加字段必须缺省兼容：旧客户端不传则走现路径。
2. **客户端切片**不改 gateway / chat / royal / kim-tcp 热路径，不改 ACK 模型与开关语义。
3. 一份一切片。鉴权、ACK rollout、device credential、`TcpConn<S>` **分属不同 PR**。
4. 客户端优化（Flutter ChatList / theme、Web `isRetryable`）**可以与后台并行**，不作为后台合入门槛。前两项已合入。

`pending_delivery.target_id` 现在是 JWT `jti`。G-03 rollout **不**等待 G-13。Web 只 ACK `lastMessage` 在 receipt 模式下会留下未确认行，离线仍能拉到；比高水位安全，因此 **不**挡 B0。

## 已落地（后台）

| 切片 | 形状 | gaps |
|---|---|---|
| 3 控制面硬化 | [group-royal.md](../group-royal.md)、[deploy.md](../deploy.md) | G-01 / G-12 已关 |
| 4 冻结 `app=kim` | [gray.md](../gray.md)、[link-layer-login.md](../link-layer-login.md) | G-05 / G-06 已关 |
| 5 pending receipt **代码** | [reliable-delivery.md](../reliable-delivery.md) | **代码在，G-03 仍开**（默认门闩） |
| 6 SIGTERM drain | [deploy.md](../deploy.md) | G-07 / G-32 已关 |
| 心跳 Redis 有界宽限 | [link-layer-login.md](../link-layer-login.md)、[observability.md](../observability.md) | G-31 已关 |
| 串行 lane + 下行 try_send | [communication-layer.md](../communication-layer.md) | G-29 / G-30 随 #66 关 |
| B1 改密吊销 | [group-royal.md](../group-royal.md)、[link-layer-login.md](../link-layer-login.md) | G-20 会话半边已关；验证/找回/注销仍开 |
| B2 device credential 服务端 | 同上 | G-13 改为客户端未持久化；`target_id` 仍 jti；登出仍全端踢 |
| B3 `TcpConn<S>` + TGateway TLS | [communication-layer.md](../communication-layer.md)、[architecture.md](../architecture.md) | G-34 已关 |
| B4 redis / sqlx / Royal deadline | [perf.md](../perf.md) | G-33 热路径已关；tower-http / tokio Builder 仍开 |
| B5 Royal 发现 + 熔断 + 短缓存 | [group-royal.md](../group-royal.md) | G-16 已关；G-22 生产 Snowflake 失败退出已关 |
| B6 可观测性剩余 | [observability.md](../observability.md) | G-15 后台半边已关；otel / 跨进程 trace 延后 |
| B7 inbox 物化 **代码** | [user-social-inbox.md](../user-social-inbox.md)、[deploy.md](../deploy.md) | **代码在，G-17 仍开**（默认 0；生产回填后置 1） |

## 客户端轨（独立，不挡后台）

规格已有，不在本文件展开实施步骤。后台合入**不**以这些完成为前提。

| 轨 | 覆盖 | 规格 | 与后台的关系 |
|---|---|---|---|
| mobile | Phase 6 自研 ChatList；Phase 7 theme | [06-mobile-client-maturity.md](./06-mobile-client-maturity.md) | 不改服务端。Phase 3–5（supervisor / outbox）已在 #67 |
| web | `isRetryable`：`ServiceUnavailable=3` 与 3xx 重试；ACK 为 id 集合 | [web-sdk.md](../web-sdk.md) | G-14 已关。ACK 集合只改善 receipt 堆积，不改 B0 开关顺序 |
| kim-client | `talk_to_user` 薄包装仍发一次性 UUID；调用方持 `client_id` | [mobile-client.md](../mobile-client.md) | Flutter outbox 已持稳定 id；其它调用方自行跟 |

G-13 的**客户端持久化**（重装后仍出示同一 device credential）挂在客户端轨，跟在后台 B2 的兼容字段之后，不进 B2 同一 PR。

## 后台轨实施顺序

不改会丢数据的排在运维硬化前面。每条合入后从 [production-gaps.md](../production-gaps.md) 删对应 G-xx（B0 除外：走完 rollout 才删），形状写回专题文档。细化设计仍是一份一切片，本表只定顺序与边界。

### B0 — pending receipt rollout（G-03 / G-04 / G-10）

不是新 ACK 模型。代码已在，compose 默认 0。

关闭条件（同时满足，缺一不可）：

1. Gateway **持续** `KIM_REQUIRE_JTI=1`
2. SCAN `login:loc:v2:*` fail-closed：`empty_jti=0` 且 `invalid=0` 且 `wrong_type=0`，命令 exit 0
3. **先** Royal `KIM_PENDING_RECEIPT=1`，**再** Chat `=1`。禁止 Chat=1 且 Royal=0

步骤与回滚见 [reliable-delivery.md](../reliable-delivery.md)、[deploy.md](../deploy.md)、[b0-pending-receipt-rollout.md](./b0-pending-receipt-rollout.md)。未走完 **不得**从 gaps 删这三条。

SCAN 必须 fail-closed：输出 `empty_jti` / `invalid` / `wrong_type` / `scanned`，任一项问题或 Redis 错误非零退出。不改 ACK 热路径。可附 compose/env、脚本、回滚 Chat 先 0。

不改：`sdk/*`、游标改 `device_id`、Snowflake 当 ACK。

G-17 关 gaps 只看生产回填 + Royal `KIM_INBOX_MATERIALIZED=1`，步骤见 [deploy.md](../deploy.md)。不插到 B0 前面。

### B8 及以后（后台，不插到 B0 前面）

| 项 | 条目 | 备注 |
|---|---|---|
| 限流 governor | G-18 | 放 gateway / Royal，不放 kim-tcp |
| 消息类型白名单；审核不挡 P99 | G-19 | |
| 邮箱验证 / 找回 / 2FA / 注销 | G-20 后半 | 与 B1 拆开 |
| R2 签名 URL + 生命周期 | G-21 | 撤回若做了却不改 R2，只是客户端隐藏 |
| 雪花 `data_center_id`；幂等 TTL | G-22 / G-23 | 生产 init 失败退出已关；机器位仍 5 bit、dc 写死 0 |
| Redis 主挂读路径降级 | G-24 | ConnectionManager 超时已关；主挂仍 SessionNotFound |
| 大群分批 + outbox 成员快照 | G-25 | |
| Header version / trace-id | G-26 | |

vectored write、ChannelMap 分片、jemalloc、一致哈希、io_uring：有 flamegraph 再做。

## 切片纪律

- 后台 PR 的 `git diff --name-only` 不应出现 `sdk/web`、`sdk/mobile/lib`、`sdk/mobile/test`。
- 客户端 PR 不应改 `services/gateway`、`services/chat`、`services/royal`、`crates/kim-tcp`。
- B2 若必须改 proto：只加字段，旧包可解；测试锁「无 device 字段 = 旧互踢 / 旧登出」。
- 关上 G-03 只看 B0 三条运维条件，不看 ChatList、不看 G-13、不看 G-34。
