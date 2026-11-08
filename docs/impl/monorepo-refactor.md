# Monorepo 依赖重构

分支 `refactor/monorepo`。设计原文留在 `research/monorepo-refactor/plan.md`，不把未落地的目标写成现有行为。

基线是 `origin/main` `5b72e95`。该提交可 `cargo metadata`；调研时工作区里的空 manifest 不在这条基线上。

## 已落地

| 切片 | 结果 |
|---|---|
| R07 | `kim-gateway-runtime` 持有 handler / revoke / hooks / run。`gateway` 只绑 WS，`tgateway` 只绑 TCP/TLS。runtime 不依赖 `kim-ws` |
| R02 | `kim-agent-model` 持有配置数据与 `ApprovalMode`。codec 不再依赖 host / Goose。Host 在调用 Goose / Codex 时做引擎转换 |
| R05a schema | `kim-server-schema` 嵌入 0001–0018。Chat 不再 `migrate!`。Compose `migrate` 与测试调用外部 runner |
| R05a auth | `AccountAuthStore` 只在 Royal。公开 `UserDirectory` 没有 create/password/epoch 方法。认证 SQL 不在 Chat |
| R05c | Royal 私有 bot 身份 writer 和事务删除回执；Chat 装配 HTTP BotControl。v1 内部合同支持 resume/complete，实际消息 store 校验持久化 scope 并阻止迟到写入。失败恢复、跨 Royal PG 实例、Memory 生命周期和真实 Chat HTTP 路径均有测试。长期合同见 `docs/group-royal.md` |
| R08a | 根 npm workspace 登记 `@kim/web-sdk` 与 `@kim/media`，lock 记录本地 link |
| R01 | `tools/check_crate_boundaries.py` 按 Cargo package id 及真实依赖 pkg 解析 normal/build 边界，兼容 normal kind=null 和别名；schema 闭包包含 dev。共享目录禁止 bot 身份 writer |
| R05b | `kim-server-data` 持有 MessageStore、公开目录、社交、IdGen、nonce、AgentSpec 及其 Memory/PG/Redis adapter。Chat 与 Royal 都依赖它；Royal 已无 Chat 编译依赖 |
| R12 | 部署检查（strict runtime、Redis 密码、Consul HTTPS）移入 `kim-server-config`。`kim-protocol` 只留 wire 协议与 HMAC 签名。IDs 类型两侧都用，单独成库没有收益，保留在 protocol |
| R11a | `tools/build/prepare-server-context.py` 按服务端 normal/build/dev/target 本地依赖闭包生成独立 workspace，完全不复制 client/agent/Codex manifest。服务端锁快照沿用并校验根锁版本/来源/checksum。实际 `--locked --all-features` 已编译六个服务端产物；编译清单无桌面或 SQLite。Server isolation CI 不检出子模块；Image 工作流已接生成上下文，根 Dockerfile 是唯一配方；PR 构建镜像且不推送 |
| R10a | crate 按 `crates/shared`、`crates/server`、`crates/client`、`crates/agent` 分组。workspace、FRB、hook、proto 生成、OTA 路径同步更新 |
| R09 | 手写 Flutter 只在 `lib/bridge` 内引用生成 FFI。`bridge/projection.dart` 是唯一的生成类型出口；错误映射与密钥执行器移入 bridge；边界脚本拒绝 bridge 之外的引用 |
| R08b | `sdk/web/app` 只从 `@kim/web-sdk` 入口读 SDK，不再跨路径引用 `src`。入口补齐应用实际用到的 proto、密码信封和类型；Vite、tsconfig、vitest 共用同一 alias |
| R04 | catalog 与 skill 查询并入 `kim_client_ffi`。Flutter 不再初始化第二套 FFI；`kim-agent-ffi`、其 FRB 配置和 `lib/src/rust_agent` 已删除。产物名 `kim_client_ffi` 不变 |
| R06 | `kim-royal-client` 持有 HTTP adapter 与 endpoint pool。Chat 装配层推 endpoint 快照并桥接 KimMetrics；client 不依赖 naming、metrics、chat、royal |
| R03 | `MobileAgent` 持有 `Weak<Inner>`，不再强持有 KimSdk。`stop_session` bump epoch 后调 `end_session`；worker 在 host 返回后复核取消与 epoch，迟到回复不落库 |

## 尚未落地

R10 的 app、web、media 物理换目录尚未做。R11 的服务端 Cargo 隔离和 Linux 镜像构建已由 #178 验证并部署；镜像发布配置使用生成上下文。Bot 升级的 Royal 先于 Chat 顺序与历史修复边界见 `docs/deploy.md`。
