# 实施设计

节奏（已拍板）：

1. **大盘点** — [production-gaps.md](../production-gaps.md) 列缺口与优先级，不写逐步补丁。
2. **细化设计** — 本目录一份一切片。可编译、可测、文件清单齐。格式对齐 tech-design-to-docs。
3. **执行** — 按该切片落地代码与测试；合入后从 gaps 删对应 G-xx，已拍板形状写回专题文档。

切片不要混：鉴权、ACK 模型、控制面密钥、`TcpConn<S>` 分属不同 PR。

| 序 | 文档 | 覆盖 | 依赖 |
|---:|---|---|---|
| 1 | （已合入）Chat 离线正文与群指令鉴权 | G-02、G-08 Chat 长连接边界 | 仍依赖 G-01 才算关洞 |
| 2 | [02-persist-first.md](02-persist-first.md) | G-09 错误语义 + identical 重放；落库后立刻 Success | 合入后 **不删** G-09；漏 Push 补偿仍依赖 G-03/G-14 |
| 3 | （未写）内部控制面 HTTP HMAC；Redis 密码；Consul ACL | G-01 | 部署密钥 |
| 4 | （未写）单租户冻结 **或** SessionStorage 带 app | G-05 | **先拍板** |
| 5 | （未写）receipt / delivery_seq，不用 Snowflake 高水位 | G-03、G-04、G-10 | 建议在 G-09 之后 |
| 6 | （未写）SIGTERM + 先摘发现再 drain | G-07、G-32 | 无 |
| 7 | （未写）JWT fail-fast；心跳 Redis 有界宽限 | G-12、G-31 | 无 |
| 8 | （未写）`TcpConn<S>` + TGateway TLS | G-34 | 形状已在 G-34 拍板 |

未列入的 G-06 / G-29 / 限流等仍按 gaps 总表，不插到切片 1 前面。
