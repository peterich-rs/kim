# 下一阶段：剩余生产缺口

| 字段 | 值 |
|---|---|
| 状态 | 切片 3–6 **代码已落地**。G-03 / G-04 / G-10 要等 rollout 才从 [production-gaps.md](../production-gaps.md) 删除 |
| 日期 | 2026-09-01 |
| 父规格 | 本文件只留未做合同。已落地形状在专题文档 |

切片 3–6 的逐步补丁已执行并写回专题文档，实施稿不保留。

## 已落地

| 切片 | 形状 | gaps |
|---|---|---|
| 3 控制面硬化 | [group-royal.md](../group-royal.md)、[deploy.md](../deploy.md) | G-01 / G-12 已关 |
| 4 冻结 `app=kim` | [gray.md](../gray.md)、[link-layer-login.md](../link-layer-login.md) | G-05 / G-06 已关 |
| 5 pending receipt | [reliable-delivery.md](../reliable-delivery.md)、[web-sdk.md](../web-sdk.md) | **代码在，G-03 仍开** |
| 6 SIGTERM drain | [deploy.md](../deploy.md) | G-07 / G-32 已关 |

关上 G-03 / G-04 / G-10 必须同时满足：Gateway `KIM_REQUIRE_JTI=1` 持续生效；SCAN `login:loc:v2:*` 空 jti = 0；先 Royal `KIM_PENDING_RECEIPT=1`，再 Chat `=1`。不是镜像版本一致。步骤见 [reliable-delivery.md](../reliable-delivery.md) 与 [deploy.md](../deploy.md)。

## 未做（本阶段外）

| 序 | 覆盖 | 规格 |
|---:|---|---|
| 4 | kim-client / Flutter 登录后 sync；G-13 稳定 `device_id` 替换 jti | [06-mobile-client-maturity.md](./06-mobile-client-maturity.md)；G-13 / G-14 仍在 gaps |
| — | 读循环隔离 / 下行 try_send | G-29 / G-30 |
| — | `TcpConn<S>` + TGateway TLS | G-34 |

其余 G-15～G-18、G-20、G-33 仍按 [production-gaps.md](../production-gaps.md) 总表，不插到 G-03 rollout 前面。
