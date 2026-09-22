# 实施设计

节奏（已拍板）：

1. **大盘点** — [production-gaps.md](../production-gaps.md) 列缺口与优先级，不写逐步补丁。
2. **细化设计** — 本目录一份一切片，只在实现它的分支上存在。
3. **执行** — 按该切片落地。合入主干前删掉切片稿；仍要长期遵守的合同写回 `docs/` 专题文档，并关对应 G-xx。

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
| pending receipt 代码 | ACK = id 集合；`acked_at` 不删行；`KIM_REQUIRE_JTI` 前置；Royal writer 先于 Chat reader。**G-03 / G-04 / G-10 仍开**，要等 rollout | [reliable-delivery.md](../reliable-delivery.md)、[web-sdk.md](../web-sdk.md)、[link-layer-login.md](../link-layer-login.md) |
| SIGTERM + 先摘发现再 drain | G-07 / G-32 | [deploy.md](../deploy.md) |
| 心跳 Redis 有界宽限 | G-31 | [link-layer-login.md](../link-layer-login.md)、[observability.md](../observability.md) |
| 串行 lane + 下行 try_send | G-29 / G-30 | [communication-layer.md](../communication-layer.md) |
| B1 改密吊销旧会话 | G-20 会话半边 | [group-royal.md](../group-royal.md)、[link-layer-login.md](../link-layer-login.md) |
| B2 device credential 服务端半边 | 可选 proto 字段；仅 enroll/出示写 `did` | [group-royal.md](../group-royal.md)、[link-layer-login.md](../link-layer-login.md) |
| B3 `TcpConn<S>` + TGateway TLS | G-34 | [communication-layer.md](../communication-layer.md)、[architecture.md](../architecture.md) |
| B4 redis / sqlx / Royal deadline | G-33 热路径 | [perf.md](../perf.md) |
| B5 Royal 发现 + 熔断 + 短缓存 | G-16 | [group-royal.md](../group-royal.md) |
| B6 可观测性剩余 | G-15。跨进程 trace 仍延后 | [observability.md](../observability.md) |
| B7 inbox 物化 | **G-17 仍开**：生产回填后 `KIM_INBOX_MATERIALIZED=1` | [user-social-inbox.md](../user-social-inbox.md)、[deploy.md](../deploy.md) |
| Mobile 成熟化、链接控制、kim-sdk 所有权、Flutter UI 壳 | Phase 3–7、链接保活、消息生命周期下沉、Flutter 只做 UI | [mobile-client.md](../mobile-client.md)、[flutter-layering.md](../flutter-layering.md)、[ffi-oo-contract.md](../ffi-oo-contract.md) |
| Web `isRetryable` | G-14 | [web-sdk.md](../web-sdk.md) |
| 密码信封 | X25519 + HTTPS | [auth-password-envelope.md](../auth-password-envelope.md) |
| Presence 进房 | P1a / P1b | [presence-room-interest.md](../presence-room-interest.md) |
| Android Logic SO OTA | arm64 `libapp.so` + `libkim_client_ffi.so` | [mobile-android-so-ota.md](../mobile-android-so-ota.md) |
| 热路径并发 | Phase 1–7 | [communication-layer.md](../communication-layer.md)、[perf.md](../perf.md) |
| 桌面 Agent（Goose 人设、bot、能力块、生产力、harness、未读同步） | 已合入主干的客户端切片 | [agent-goose.md](../agent-goose.md) |

漏 Push 补偿仍是 G-03。G-20 后半（验证/找回/注销）与 G-13 客户端持久化仍开。

## 未合入

| 切片 | 覆盖 |
|---|---|
| pending receipt rollout | [b0-pending-receipt-rollout.md](./b0-pending-receipt-rollout.md)：G-03 / G-04 / G-10。代码已合入，关 gaps 等运维三条同时成立 |
| 服务端会话隐藏 | [chat-inbox-hide.md](./chat-inbox-hide.md)：Delete conversation for me。尚未合入 |
| Codex harness 嵌入 | [codex-embed.md](./codex-embed.md)：分支 `feat/codex-agent-embed`。Goose 与 Codex 并列，`runtime` 选择 |
| FFI 收进 workspace | [ffi-workspace.md](./ffi-workspace.md)：Phase 1–5 已落地。`KimBridge` 仍是一个类 |

没有对应分支、也不描述当前系统的稿子不放在这里。
