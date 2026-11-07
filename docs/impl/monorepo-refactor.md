# Monorepo 依赖重构

分支 `refactor/monorepo`。设计原文留在 `research/monorepo-refactor/plan.md`，不把未落地的目标写成现有行为。

基线是 `origin/main` `5b72e95`。该提交可 `cargo metadata`；调研时工作区里的空 manifest 不在这条基线上。

## 已落地

| 切片 | 结果 |
|---|---|
| R07 | `kim-gateway-runtime` 持有 handler / revoke / hooks / run。`gateway` 只绑 WS，`tgateway` 只绑 TCP/TLS。runtime 不依赖 `kim-ws` |
| R02 | `kim-agent-model` 持有配置数据与 `ApprovalMode`。codec 不再依赖 host / Goose。Host 在调用 Goose / Codex 时做引擎转换 |
| R05a schema | `kim-server-schema` 嵌入 0001–0016。Chat 不再 `migrate!`。Compose `migrate` 与测试调用外部 runner |
| R05a auth | `AccountAuthStore` 只在 Royal。公开 `UserDirectory` 没有 create/password/epoch 方法。认证 SQL 不在 Chat |
| R05c | Chat 删除经 `BotControl` 协调器，purge 核对 `StoreScope`。生产 Chat 无 Royal endpoint 时 bot 写返回不可用。测试栈注入 directory control |
| R01 | `tools/check_crate_boundaries.py` 检查 normal/build 角色边界；schema 闭包包含 dev |

## 尚未落地

R03 调度所有权、R04 FFI 合库、R05b 数据抽库、R06 Royal adapter、R08 Web、R09 Flutter 边界、R10 目录、R11 Docker 输入、R12 协议清理。Royal HTTP 的 identity/resume/complete 仍走旧 DELETE 加自身 purge；Chat 删除已走协调器。
