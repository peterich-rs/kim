# 实施设计

节奏（已拍板）：

1. **大盘点** — [production-gaps.md](../production-gaps.md) 列缺口与优先级，不写逐步补丁。
2. **细化设计** — 本目录一份一切片。可编译、可测、文件清单齐。
3. **执行** — 按该切片落地；合入后从 gaps 删对应 G-xx，形状写回专题文档。切片实施稿不长期保留。

切片不要混：鉴权、ACK 模型、控制面密钥、`TcpConn<S>` 分属不同 PR。

## 已合入

形状在专题文档，本目录不再留实施稿。

| 切片 | 覆盖 | 形状 |
|---|---|---|
| Chat 离线正文与群指令鉴权 | 原 G-02 / G-08 长连接 | [reliable-delivery.md](../reliable-delivery.md)、[group-royal.md](../group-royal.md) |
| persist-first | 原 G-09 错误语义；identical `clientId` 从落库重放 | [control-layer-chat.md](../control-layer-chat.md)、[reliable-delivery.md](../reliable-delivery.md) |
| Royal HTTP HMAC | 除 `/health`、`/api/v1/auth/*` 外内部口要签 | [group-royal.md](../group-royal.md) |
| 控制面硬化 | Chat kick HMAC；nonce NX EX 121；Redis 密码 + `noeviction`；Consul mTLS+ACL；G-12 fail-fast | [group-royal.md](../group-royal.md)、[deploy.md](../deploy.md) |
| 冻结单租户 `app=kim` | 原 G-05 / G-06：拒非 kim JWT；loc+session v2；Chat 拒非 kim session；account 灰度；loc cache opt-in | [gray.md](../gray.md)、[link-layer-login.md](../link-layer-login.md)、[deploy.md](../deploy.md) |
| pending receipt | 代码已合入：ACK = id 集合；`acked_at` 不删行；`KIM_REQUIRE_JTI` 前置；Royal writer 先于 Chat reader。**G-03 / G-04 / G-10 仍开**，要等 [reliable-delivery.md](../reliable-delivery.md) rollout | [reliable-delivery.md](../reliable-delivery.md)、[web-sdk.md](../web-sdk.md)、[link-layer-login.md](../link-layer-login.md) |
| SIGTERM + 先摘发现再 drain | G-07 / G-32：unix SIGTERM+SIGINT；Container 先 deregister 再 JoinSet drain；Royal/Router HTTP graceful | [deploy.md](../deploy.md) |
| 心跳 Redis 有界宽限 | G-31：仅确认吊销立刻关；存储错误连续 3 次后断开；期内不续签 JWT；登录仍 fail-closed | [link-layer-login.md](../link-layer-login.md)、[observability.md](../observability.md) |
| 串行 lane + 下行 try_send | G-29 / G-30：per-`channel_id` 串行 lane；网关 Disconnect + `kim_mailbox_full_total`。#66 合入后本行生效 | [communication-layer.md](../communication-layer.md) |
| B1 改密吊销旧会话 | G-20 会话半边：`token_epoch` + `live_claims` + kick；改密不发新 token | [group-royal.md](../group-royal.md)、[link-layer-login.md](../link-layer-login.md) |
| B2 device credential 服务端半边 | 可选 proto 字段；仅 enroll/出示写 `did`；logout 仍全端踢；`target_id` 仍 jti | [group-royal.md](../group-royal.md)、[link-layer-login.md](../link-layer-login.md) |
| B3 `TcpConn<S>` + TGateway TLS | G-34：`FrontendState`、`try_acquire`、keepalive、进程内 rustls；明文 `new(stream)` 保留。reuseport / vectored 仍延后 | [communication-layer.md](../communication-layer.md)、[architecture.md](../architecture.md) |
| B4 redis / sqlx / Royal deadline | G-33 热路径：ConnectionManager 3s 超时、sqlx `statement_timeout`、migrate 独立连接、目录 RPC 800ms。tower-http / tokio Builder 仍开 | [perf.md](../perf.md) |
| B5 Royal 发现 + 熔断 + 短缓存 | G-16：`RoyalPool` RR + 5xx 熔断 + Consul `find`；好友/block/`exists` 30s 缓存；royal-2；生产 Snowflake 失败退出 | [group-royal.md](../group-royal.md) |
| B6 可观测性剩余 | G-15：send→ack、Royal RPC、backlog gauge、royal `/metrics`、告警规则。跨进程 trace 仍延后 | [observability.md](../observability.md) |
| B7 inbox 物化 | 群 `summaries` 批量、advisory lock、回填脚本、Memory 双索引。**G-17 仍开**：生产回填后 `KIM_INBOX_MATERIALIZED=1` | [user-social-inbox.md](../user-social-inbox.md)、[deploy.md](../deploy.md) |
| Mobile 成熟化 Phase 3–7 | FFI supervisor / SQLite upsert / Dart outbox；自研 ChatList（去 flutter_chat_ui）；KimTheme v2 | [mobile-client.md](../mobile-client.md)、[06-mobile-client-maturity.md](./06-mobile-client-maturity.md) |
| Web `isRetryable` | G-14：`ServiceUnavailable=3` 与 3xx 重试；99 / 1xx / 111 不重试。漏 Push 见 G-03 | [web-sdk.md](../web-sdk.md) |

漏 Push 补偿仍是 G-03。G-03 要等 rollout，不是再写一套 ACK。G-20 后半（验证/找回/注销）与 G-13 客户端持久化仍开。

Q1 **已拍板**：冻结 `app=kim`。Q2 **已拍板**：Consul 关明文 8500 + 私有 CA HTTPS/mTLS + ACL deny。

剩余阶段合同：[next-stage.md](./next-stage.md)（后台轨与客户端轨分开，web / mobile 不挡后台）。

## 待写 / 待执行

后台轨顺序与边界见 [next-stage.md](./next-stage.md)。不要把客户端优化写进后台 PR。

| 序 | 轨 | 覆盖 | 依赖 | 规格 |
|---:|---|---|---|---|
| B0 | 后台 | pending receipt rollout（G-03 / G-04 / G-10） | SCAN fail-closed 已合入；**关 gaps 等运维三条同时成立** | [b0-pending-receipt-rollout.md](./b0-pending-receipt-rollout.md) |
| — | 客户端 | Mobile Phase 8 手工走查 | 无服务端改动 | [06-mobile-client-maturity.md](./06-mobile-client-maturity.md) |
| — | 客户端 | 链接控制域（keepalive / CODE_PING / 看门狗 / 退避复位） | 不改 gateway ACK | [07-mobile-link-control.md](./07-mobile-link-control.md) |

G-03 关闭条件见 [reliable-delivery.md](../reliable-delivery.md)，不要在 gaps 里提前删条。G-17 关 gaps 等生产回填 + `KIM_INBOX_MATERIALIZED=1`。剩余后台不插到 B0 前面。
