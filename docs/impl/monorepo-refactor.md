# Monorepo 依赖重构

分支 `refactor/monorepo`。设计原文留在 `research/monorepo-refactor/plan.md`，不把未落地的目标写成现有行为。

基线是 `origin/main` `5b72e95`。该提交可 `cargo metadata`；调研时工作区里的空 manifest 不在这条基线上。

## 已落地

| 切片 | 结果 |
|---|---|
| R07 | `kim-gateway-runtime` 持有 handler / revoke / hooks / run。`gateway` 只绑 WS，`tgateway` 只绑 TCP/TLS。runtime 不依赖 `kim-ws` |

## 尚未落地

R01 边界门、R02 模型/codec、R03 调度所有权、R04 FFI 合库、R05 认证/BotControl/数据抽库、R06 Royal adapter、R08 Web、R09 Flutter 边界、R10 目录、R11 Docker、R12 协议清理。
