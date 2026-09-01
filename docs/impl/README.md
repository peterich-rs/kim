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

漏 Push 补偿仍是 G-03 / G-14。`ctx.resp` 无超时是 G-30。G-01 剩余：Chat `/internal/kick`、Redis 密码、Consul ACL。

## 待写

| 序 | 覆盖 | 依赖 |
|---:|---|---|
| 3 | Chat kick HMAC；Redis 密码；Consul ACL | 部署密钥 |
| 4 | 单租户冻结 **或** SessionStorage 带 app（G-05） | **先拍板** |
| 5 | receipt / delivery_seq，不用 Snowflake 高水位（G-03、G-04、G-10） | 无 |
| 6 | SIGTERM + 先摘发现再 drain（G-07、G-32） | 无 |
| 7 | JWT fail-fast；心跳 Redis 有界宽限（G-12、G-31） | 无 |
| 8 | `TcpConn<S>` + TGateway TLS（G-34） | 形状已在 G-34 拍板 |

未列入的 G-06 / G-29 / 限流等仍按 gaps 总表，不插到切片 3 前面。
