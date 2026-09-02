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

漏 Push 补偿仍是 G-03 / G-14。`ctx.resp` 无超时是 G-30。

Q1 **已拍板**：冻结 `app=kim`。Q2 **已拍板**：Consul 关明文 8500 + 私有 CA HTTPS/mTLS + ACL deny。

剩余阶段合同：[next-stage.md](./next-stage.md)。

## 待写 / 待执行

| 序 | 覆盖 | 依赖 | 规格 |
|---:|---|---|---|
| 6 | 心跳 Redis 有界宽限（G-31） | 无 | 本阶段外 |
| 7 | `TcpConn<S>` + TGateway TLS（G-34） | 形状已在 G-34 拍板 | 本阶段外 |
| 8 | Mobile 成熟化 Phase 3–5：FFI supervisor、SQLite upsert/page、Dart link/outbox（本 PR）。Phase 6 ChatList / Phase 7 theme 仍待做 | 无服务端改动 | [06-mobile-client-maturity.md](./06-mobile-client-maturity.md) |

未列入的 G-29 / 限流等仍按 gaps 总表。G-03 关闭条件见 [reliable-delivery.md](../reliable-delivery.md)，不要在 gaps 里提前删条。
