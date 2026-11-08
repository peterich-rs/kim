# KIM monorepo 依赖分析与改造设计

保留一个 monorepo、一个跨手机与桌面的 Flutter 应用，以及现有 Rust 通信和消息生命周期核心。改造先解除错误依赖、明确资源所有权，再调整目录和构建入口。

这是调研与待实施设计，不是当前架构合同。实施时按切片进入对应分支的 `docs/impl/`；完成后把长期约定写回现有专题文档。本文留在 `research/`，不把尚未实现的设计写成现有行为。

## 1. 分析范围与基线

- 分析日期：2026-11-07。
- 当前分支：`refactor/ffi-ownership`；HEAD：`1f433bc391d74b5205f316345ffee3a12d771e35`。
- 核对了根 workspace 的 26 个成员的 manifest，以及服务端、客户端、Agent、Flutter、Web、Media、Docker、CI、OTA 的相关入口、接口和关键实现。
- 逐文件依赖清单见 [dependency-audit.json](dependency-audit.json)。它记录普通、build、dev 和 target 条件依赖，是静态清单，不是 Cargo 解析后的依赖图。工具行为的隔离实验见 [tool-behavior-checks.json](tool-behavior-checks.json)。
- 现存源码优先。CodeGraph 用于定位和读取源码，实际 import 与 manifest 用于确认依赖；其跨语言同名符号推导的调用计数不作为证据。
- 当前有 18 个已跟踪的零字节文件，包括 `kim-sdk/Cargo.toml`、`kim-sdk/src/secrets.rs`、`kim-desktop-runtime/src/lib.rs`、若干 FFI 入口、主 CI 和文档。新增的 FFI 分文件中也存在仅有空行的文件。
- 已运行 `cargo metadata --no-deps --format-version 1`，返回 101：`kim-sdk/Cargo.toml` 缺少 `[package]` 或 `[workspace]`。因此本次没有可验证的完整构建、目标平台 feature 图、二进制尺寸或运行时结果。
- 对空的 SDK manifest、桌面 runtime 和旧 FFI 入口，读取了 HEAD 版本作为历史参照，未恢复或覆盖文件。下文明确标注 HEAD 的结论不代表当前入口仍可执行。

当前工作区处于迁移中。先前回答中“配置已按平台区分”的判断成立于 Flutter hook 与可见 manifest；它不能证明手机端已排除所有 Host 依赖，也不能证明当前工程可构建。

## 2. 已确认的依赖和问题

### 2.1 当前编译依赖

箭头表示普通源码或 manifest 依赖，条件写在括号内；SDK manifest 的空缺由现存源码补充确认。

```text
royal ──► chat ──► container / router / session / protocol / tcp
tgateway ──► gateway ──► container / session / protocol / ws

Flutter ──► client-ffi ──► sdk ──► client ──► core / protocol / ws
                            │
                            └─► agent-codec ──► agent-host ──► Goose / MCP

client-ffi ──► desktop-runtime ──► sdk / agent-codec / agent-host(codex)
               (桌面条件)
Flutter ──► agent-ffi ──► agent-host(codex) / agent-codec
               (hook 仅在桌面构建)
```

`kim-sdk → kim-agent-codec` 有现存方法调用证据：`upsert_agent_profile` 与 `list_agent_profiles`。不能因 SDK manifest 为空而把这条关系从分析中删除。

### 2.2 问题登记

| 编号 | 代码依据 | 影响与判定 | 处理 |
|---|---|---|---|
| C01 | `services/royal/src/lib.rs:21` 引入 chat 的目录、ID、消息存储、用户、社交、AgentSpec 和 nonce 类型 | Royal 的 HTTP 模块依赖实时 Chat 包的实现布局；需要数据能力时带入 Chat 的编译依赖 | 提取服务端数据模块，移除 `royal → chat` 普通依赖 |
| C02 | `services/tgateway/src/main.rs:4` 调用 `gateway::load_config/run_gateway`；`gateway/src/run.rs` 已接收 `Server` | 复用本身合理，但公共网关运行框架寄存在 WS 服务包，TGateway manifest 间接引入 WS 适配 | 提取网关运行库，保留既有 Server 接口 |
| C03 | `kim-sdk/src/lib.rs:632/689` 调 codec；`kim-agent-codec/src/lib.rs:7` 依赖 GooseMode 和 Host 数据类型；Host manifest 默认包含 Goose、rmcp、Tokio process | 读写跨端配置需要完整执行 Host。关闭 codex feature 只能排除 Codex，不能排除 Goose/MCP 基础依赖 | 提取无执行依赖的 Agent 数据模型；codec 改为只依赖该模型与 protocol |
| C04 | `kim-sdk/src/agent/mod.rs:74` 的 MobileAgent 持有 KimSdk；`lib.rs:112/611` 的 SDK 强持有 AgentPort | 形成可构造的 `SDK → Agent → SDK` 强引用环。HEAD runtime 的 install 正是该装配方式；当前 runtime 入口为空，不能断言当前线上必然泄漏 | 桌面调度移出 SDK；回调持有弱引用；关闭流程显式管理 task |
| C05 | SDK `stop_session:373` 清理连接/Outbox，但 AgentPort 只有 enqueue/catch_up；desktop `drive.rs:126/143` 每次创建独立 cancel token | SDK 会话结束与 Host 工具执行、权限等待缺少统一退出合同。epoch 检查不能代替取消已经开始的外部操作 | 会话 lease、取消传播、任务回收、旧 epoch 写入拒绝 |
| C06 | `app/lib/main.dart:90` 启动第二套 Agent FFI；`kim-agent-ffi/api/catalog.rs:15` 独立 SUPPORT_ROOT；client FFI 桌面依赖 runtime | 两个动态库都引用 Host，目录查询与执行分处两套初始化。不能假定两个动态库中的 Rust static 共享 | 合并为一个应用 FFI 库；Agent 目录/技能查询成为该库中的接口 |
| C07 | 扫描 app/lib 并排除生成目录 ffi：25 个文件、43 条 import/export；features 14、bridge 6、core 2、design 1、models 1、main 1 | 原统计 16 仅覆盖 features/design/models 子集。core/errors、core/failures 与 main 的引用也要迁移；bridge 的 adapter 引用与 ports 泄漏须分别判断 | 边界门扫描整个手写 lib，以路径白名单批准 adapter；main/core/设计/UI 禁止直接引用生成代码，ports 不通过 export 绕过 |
| C08 | `sdk/web/app/lib/chat.ts:10/11`、`ChatProvider.tsx:28` 直接读 `../../src`；Vite alias 指向 SDK 源文件 | npm 包的 exports 没有成为真正接口；SDK 和 React 产品依赖混在同一包 | 拆应用与 SDK，应用只依赖包导出的入口 |
| C09 | `sdk/media/src/index.ts:41` 调 Royal `/api/v1/auth/me`，并写入 R2；有独立 wrangler 配置 | 是远程部署服务，不是客户端 SDK；其鉴权可用性依赖 Royal | 迁到 services/media，保留运行协议与独立发布 |
| C10 | 根 Dockerfile 复制全部 crates/services/examples，未复制 third_party；Host/helper 的 manifest 指向 Codex path deps | manifest 解析闭包、实际编译单元、Docker 上下文/最终镜像是不同集合。选 server 包仍可能解析未选包的 path manifest，不意味着要复制完整桌面源码 | 优先验证 manifest 解析骨架或 server 子集 workspace；以真实 locked 构建证明，不预设复制整个 Codex |
| C11 | 根 sqlx workspace dependency 同时启用 postgres/sqlite/macros/migrate；根 patch、SQLite pin 为 Codex/客户端共同服务 | 编译包选择与 feature、锁解析、原生链接约束混在一起；不同目录并不提供隔离 | 成员声明后端 feature；一个锁保持原生 SQLite 兼容；按产物执行检查 |
| C12 | Web/Media workflow 按当前目录触发；Web 没包含共享 proto 路径；主 CI 当前为空 | 协议变化可能不触发 Web 检查；目录迁移还会影响缓存、工作目录和发布路径 | 检查按依赖闭包触发，发布入口另设 |
| C13 | OTA workflow 在 `.github/workflows` 与 `app/ota/logic-ota.workflow.yml` 各一份，hook/FRB/IDE/文档多处硬编码路径 | 目录迁移会影响生成、缓存、签名打包和调试；两份 workflow 易漂移 | 确定唯一执行定义，配套脚本集中维护，逐个入口迁移 |
| C14 | 根 goose submodule 未初始化；Host manifest 使用 registry 中的 goose-* crate，没有本地 goose path | 根 goose 不是当前 Host 编译来源 | 先确认它是参考源码还是待用 fork；有用途才迁到 third_party，不把移动当升级 |
| C15 | protocol 导出 strict runtime、Redis/Consul 配置检查、JWT 签发和密码私钥能力；core 为 ChannelId 依赖 protocol | 通信抽象与业务、安全实现存在编译关联，但不是当前首要运行问题 | 先隔离服务端部署策略；是否抽 IDs 库由依赖/构建收益决定 |
| C16 | chat/users.rs:191 的 UserDirectory 含 create/password_hash/set_password/token_epoch 等；royal.rs 的 HTTP 实现对部分方法返回 royal-only | 认证存储并不全在 Royal 文件中，整份提取会把私有账号写能力共享出去 | R05a 先拆 Royal 私有 AccountAuthStore 与公开 UserDirectory，保持原子改密/epoch SQL 和 Memory 一致性 |
| C17 | chat/bot.rs:74/133 与 royal/bot.rs:57/85 都经 UserDirectory 创建/删除 bot；royal.rs:1100 的 count_bots 返回 royal-only | bot 身份生命周期、配额和公开目录查询混在一个接口；HTTP 模式已委托 Royal，本地模式仍可直接写 | R05c 独立收口 BotControl：身份写与配额归 Royal，消息清理按实际 store 路由；不保留 count_bots 占位 |
| C18 | Chat 与 Royal 的删除各自先 delete_bot 再 purge_peer_dm；Memory/PG delete_bot 与 Chat lookup 都会对缺失行返回 NotFound | 身份已删但清理失败无法自然恢复；Royal 也无法清理 Chat 进程内 Memory。HTTP 请求成功不等于跨 store 删除完成 | R05c1–c3：身份删除回执、实际 store/scope 路由、删除协调器及失败恢复；不能删除 Chat 必要的本地 purge |

### 2.3 应保留的设计

1. `Conn`/`Server`、TCP/WS 实现与容器的组合已有真实复用，TGateway 的泛型运行入口就是证据。
2. `MessageStore`、`GroupDirectory`、`UserDirectory`、`SocialDirectory` 已有真实 adapter，应复用其 IM 合同；其中 UserDirectory 混入认证能力，按 R05a 定向分离，不照搬整个接口，也不另造所有领域接口。
3. SDK 已把消息写入、Outbox、未读与 persist-before-ACK 收在 Rust 内部。`store/mod.rs` 的单写队列应继续隐藏事务与发布顺序。
4. Flutter `KimBridge`、能力 ports、FakeKim 和 UI 订阅器可继续使用。`AgentHostController` 当前只转发权限/状态事件，不应把 Agent turn 编排搬回 Dart。
5. `RoyalPool` 已有发现、熔断、重试、deadline；提取 HTTP adapter 时整体迁移这些语义。

## 3. 目标目录和依赖规则

目录只是表达职责；实施前几个阶段可保持 crates 平铺，避免逻辑提取与全仓路径迁移混在同一次变更。

```text
im/
├─ apps/                                  ★ 产品入口
│  ├─ kim/                                ★ 当前 app，手机/桌面共用
│  │  ├─ lib/{bootstrap,bridge,core,design,features,models,router,l10n}
│  │  ├─ lib/ffi/                           生成文件，应用内部使用
│  │  ├─ android/ ios/ macos/                现有平台工程
│  │  ├─ hook/ plugins/ packages/
│  │  └─ ota/
│  └─ web/                                ★ React 应用、边缘入口、assets 配置
├─ services/
│  ├─ gateway/ tgateway/ chat/ royal/ router/
│  └─ media/                              ★ 当前 sdk/media
├─ crates/
│  ├─ shared/                             ★ 明确的跨端依赖集合
│  │  ├─ kim-core/ kim-tcp/ kim-ws/ kim-protocol/ kim-log/
│  │  └─ kim-agent-model/                  ★ 纯 Agent 配置模型与静态目录
│  ├─ server/                             ★ 服务端复用
│  │  ├─ kim-container/ kim-naming/ kim-router/ kim-session/ kim-metrics/
│  │  ├─ kim-server-data/                  ★ IM ports、数据类型、Memory/PG/Redis adapter
│  │  ├─ kim-server-schema/                ★ 唯一迁移序列，不含运行时认证存储
│  │  ├─ kim-royal-client/                 ★ Royal HTTP adapter 与连接池
│  │  └─ kim-gateway-runtime/              ★ 公共网关运行与鉴权/路由
│  ├─ client/                             ★ 跨手机/桌面的消息核心
│  │  └─ kim-client/ kim-sdk/ kim-client-ffi/
│  └─ agent/                              ★ 运行适配
│     └─ kim-agent-codec/ kim-agent-host/ kim-desktop-runtime/ kim-codex-helper/
├─ sdk/web/                                纯 TypeScript SDK
├─ deploy/                                 环境、编排、运维脚本
├─ tools/                                  构建/生成/边界检查/压测
│  └─ kim-server-migrate/                  ★ 唯一链接 server-schema 的独立 runner
├─ examples/                               最小使用示例
├─ third_party/codex/                       独立上游源码与版本
├─ docs/                                   当前行为合同
└─ research/                               研究与待决设计
```

`kim-agent-model` 是纯数据与无 I/O 校验库；`kim-agent-codec` 是跨格式兼容适配层，必须无执行引擎依赖，但其 JSON 别名、protobuf 编号、版本及未知字段兼容合同仍要单独验收。无执行依赖不等于没有兼容性复杂度。`kim-agent-ffi` 在兼容过渡完成后删除；不建立第三个 FFI 库。

Windows/Linux 当前只是 Host 支持条件与构建意图，目录树不把尚不存在的平台工程视为已交付；先验收当前实际支持的平台。

完整迁移映射：

| 当前成员或目录 | 目标归属 | 处理性质 |
|---|---|---|
| kim-core、kim-tcp、kim-ws、kim-protocol、kim-log | crates/shared 下同名目录 | 路径迁移；通信设计保持 |
| kim-container、kim-naming、kim-router、kim-session、kim-metrics | crates/server 下同名目录 | 路径迁移；SessionStorage 暂保留在 router，避免顺手改变大量合同 |
| kim-client、kim-sdk、kim-client-ffi | crates/client 下同名目录 | 路径迁移；SDK 调度提取、FFI 合库在之前完成 |
| kim-agent-codec、kim-agent-host、kim-desktop-runtime、kim-codex-helper | crates/agent 下同名目录 | codec 降低依赖，Host/model 分开，helper 保持独立可执行文件 |
| kim-agent-ffi | 主 FFI 的 agent/catalog/skills 接口 | 调用方切换后删除，不保留第二份生成源码 |
| services 的 gateway、tgateway、chat、royal、router | 原位置 | 抽出可复用实现，进程名与运行配置保持 |
| examples/pkt-client、examples/kim-client-demo | 原位置 | 最小使用示例；更新路径 |
| examples/kimbench | tools/kimbench | 压测可执行工具；仍作为根 Cargo workspace 成员 |
| app（包含原 sdk/mobile 迁移内容） | apps/kim | 完成当前迁移后只保留一个有效应用目录 |
| sdk/web 的 src、协议脚本、SDK 测试 | sdk/web | 保留 SDK，按测试职责拆分混入的应用测试 |
| sdk/web/app、worker、Vite app 配置、wrangler 配置 | apps/web | 同一个 Web 产品发布单元 |
| sdk/web/demo | examples/web-client | 保留最小 SDK 演示；归根 npm workspace |
| sdk/media | services/media | 独立部署服务 |
| third_party/codex、根 goose | Codex 保持；Goose 用途确认后再决定 | 不随结构迁移变更上游 revision 或依赖来源 |
| deploy、docs、research、books | 原角色 | books 是参考资料，不进入产品依赖、镜像或代码生成链路 |

规则：

- shared 禁止依赖 client、server、Host、FFI；对应用环境变量、进程启动无隐式副作用。
- client 核心禁止依赖 server、agent-host、desktop-runtime、Codex、Goose 执行包。
- agent-model/codec 禁止依赖 agent-host、provider 执行实现、MCP、子进程。
- Host 不依赖 KimSdk 和 IM 存储；IM 集成只在 desktop-runtime。
- desktop-runtime 可以依赖 SDK 的公共合同；SDK 不依赖 desktop-runtime。SDK 回调通过 AgentPort 接入。
- client-ffi 作为应用装配入口，可在桌面目标条件下依赖 runtime；把这个例外显式列入检查器。
- services 的普通/build 依赖禁止指向另一个 services 包；E2E harness 的 dev 依赖允许，并单独登记。
- server 分组内部也按角色限制依赖：kim-royal-client 不依赖 naming/Consul、kim-metrics 或任一进程包；发现与指标 adapter 由 Chat 装配并注入。物理同组不代表可以任意相互依赖。
- kim-server-data 本身也不能普通依赖 naming/kim-metrics，否则 HTTP adapter 会经它重新引入这些依赖。通用 observer 合同留在库，进程侧 newtype 实现指标桥接。
- Royal 的 device/JWT/revoke/password/epoch 写能力属于 Royal 私有实现；共享 IM 数据库不导出这些写能力。schema 归属与运行时写入所有者分开登记。
- Royal 也持有 bot 生命周期和配额规则。共享 UserDirectory 保留 lookup/profiles/upsert 等公开目录能力，不包含 create_bot/update_bot/delete_bot/count_bots；Chat 通过独立 BotControl port 调 Royal。
- 只有独立迁移 runner 可以依赖 kim-server-schema。Chat、kim-server-data、kim-royal-client 与 Royal 在线进程均不得在 normal/build/dev 依赖闭包中链接它；本地/E2E 由外部命令运行同一个 migrator。
- Web 应用只通过 `@kim/web-sdk` 导出读取 SDK；不得跨路径读取 `sdk/web/src`。
- 应用插件与 lints 先留在 apps/kim 内；出现第二个真实消费者后再提升为根 packages。

## 4. 服务端代码设计

### 4.1 先把数据能力从 Chat 提出来

```text
现状：royal ──► chat::{store,directory,users,social,idgen,agent_spec}

目标：royal ──► ★ kim-server-data ◄── chat
       chat ──► ★ kim-royal-client ──► kim-server-data 的 ports/类型
```

`kim-server-data` 第一版内部组织：

```text
src/
├─ lib.rs                 稳定导出，按现有领域模块组织
├─ message/               MessageStore、消息/投递/inbox 类型及公共算法
├─ directory/             group/user/social 的合同与实现
├─ agent_spec/            AgentSpecStore、ProviderAccount 记录及实现
├─ idgen/                 Sequence/Snowflake
├─ nonce/                 重放 nonce 存储；签名算法仍由 protocol 提供
├─ memory/                本地/E2E adapter
└─ postgres/ redis/       feature 控制的 IM 数据 adapter
```

R05 先定写入归属、拆开混合的 UserDirectory 合同，再迁移对应类型、ports 与实现。MessageStore 保持现有形状；Memory/PG 作为内部 adapter，不把每张表拆成 crate。

| 数据/能力 | 唯一运行时归属 | 共享接口与限制 |
|---|---|---|
| 消息、投递、inbox、群、社交、公开 profile、AgentSpec、IdGen | kim-server-data；Royal 或 Chat 的既有模式装配 | 保留现有 IM ports 和 Memory/HTTP/PG 替换能力 |
| device_credentials 的 enroll/get/revoke，DeviceHot | services/royal::device | 只迁路径时保持当前写入实现一份；Chat 不获得 DeviceDirectory/凭据写接口 |
| JWT 签发决策、JTI revoke、账号 token epoch 的权威业务 | services/royal 的 auth/revoke | Gateway 可以通过现有校验 port 读 Redis 热状态或调 Royal；不是另一份凭据写实现 |
| password_hash、账号 create、改密与 epoch 原子更新 | Royal 私有 AccountAuthStore（现有 users.rs 中的认证代码迁入） | shared UserDirectory 删除这些方法；HTTP adapter 不再实现“royal-only”失败方法 |
| bot 创建/配置/限额/身份删除与删除回执 | Royal 私有 BotService/BotStore | BotControl 不拥有 Chat 的消息存储；count_bots 私有，身份删除/恢复与完成回执是显式合同 |
| bot 删除后的 DM 清理 | 持有实际 MessageStore 的 Chat 删除协调器；HTTP adapter 交给 Royal 的消息接口执行 | 按实际 adapter 选择执行位置；不要求 Royal 与 Chat 的 MessageStore 是同一个对象/数据库 |
| HMAC nonce 重放记录 | server-data::nonce 的通用存储 port | 不承载用户认证/设备生命周期，不因有 Redis 就合并所有认证存储 |
| PostgreSQL 0001–0016 的 schema 定义 | 新 kim-server-schema；唯一消费者是独立 kim-server-migrate runner | 在线进程与 IM adapter 不链接它，不通过启动 helper/dev 依赖间接带入；DDL 唯一来源与 DML owner 分开 |

当前 `users.rs:191` 的 UserDirectory 不只是公开资料：它包含 create/password_hash/set_password/token_epoch/set_password_and_bump_epoch。仅把文件移到共享 crate 仍会把认证存储暴露给 Chat。这部分必须在 R05a 显式分离：

- 共享 UserDirectory 只留公开资料与 IM 目录操作，包括实时链路使用的 lookup/profiles/upsert；注册、口令和 token epoch 改为 Royal 私有 AccountAuthStore。bot 生命周期也移出该 trait，按下面 R05c 处理；保留改密加 epoch 更新的单条 SQL 原子性。
- PG 两个接口仍访问既有 users 表；认证 SQL 只有 Royal adapter 一份。Memory 测试由 Royal 私有组合 adapter 共享同一个账号状态，不能复制两份账号 map 造成注册后 profile 不可见。
- Chat 的 Memory/public PG adapter 不包含认证 secret 字段或 password/epoch 写方法；移除 CachedUserDirectory 与 HttpUserDirectory 中为认证接口提供的转发/占位错误。
- device.rs/revoke.rs/JwtConfig/RoyalState 的认证字段明确留在 Royal，不在“原样搬迁”中进入 server-data。
- 当前 store/mod.rs:52 直接为 KimMetrics 实现 AckObserver，也不能整段搬走：库只保留 AckObserver，Royal 装配定义自己的 ProcessAckObserver(Arc<KimMetrics>) 并实现该 trait。跨 crate 后不能在进程里直接给两个外部类型补 impl；newtype 同时避免 orphan rule 与指标编译依赖回流。

`services/chat` 保留：长连接指令 handler、鉴权后的消息编排、实时推送、room/presence/typing、admin、容器装配。

`services/royal` 保留：HTTP 路由、用户认证/设备与吊销业务、bot 生命周期/配额、对外错误映射、maintenance 装配；它的进程入口负责选择共享数据 adapter。

Chat 的消息/公开目录保留“经 Royal HTTP”与“直接数据库/Memory”两种装配。bot 控制路径单独调整为 Royal 权威，属于 R05c 的明确行为变化，不能套用“仅搬代码、不改变 fallback”的验收。

**BotControl 的读写归属与本地模式策略（R05c）**

- 当前并非 Chat 没有配额检查：Memory create_bot 调 count_bots，PG create_bot 在事务内查询计数；它们都使用既有上限。问题是公共 trait 让查询、产品规则和可写 adapter 混为一体。
- BotControl 的 create/update/config 复用已有 Royal HTTP 路由；删除改为下面的 identity-delete/resume/complete 合同，不能把当前 DELETE /api/v1/bot 当成“只删身份”。count_bots/配额与 owner 校验仍是 Royal 私有规则。新增内部合同需要先发布 Royal 支持，再切换 Chat，不能假设旧服务识别新阶段。
- Chat 的 create/update 做协议适配；delete 交给 BotDeletionCoordinator，后者持有 BotControl 和**与当前消息 handler 相同的 MessageStore 资源**。lookup/profiles/upsert 保留给实时路径；bot 的可写身份存储实现不进入 kim-server-data。
- 公开 upsert 只承担既有普通账号目录建档，不能设置 kind=bot、owner_account、client_profile_id 或 bot 配置；通用 profile 更新也不能成为绕过 BotControl 的 bot 配置写口。公开查询仍可返回 bot 的身份/owner 投影，读取这些字段不等于拥有生命周期写能力。
- 明确选择**不保留生产或本地 Chat 的独立 bot 身份 writer**。即使消息走直连 DB，也配置独立 BotControl endpoint（可以复用原 Royal URL，但不能由消息 backend 选择隐式决定）。没有 endpoint 的独立 Chat 仍能做已有普通 IM，bot 创建/配置/删除明确返回 ServiceUnavailable；不返回假成功、不回退本地身份写。完整本地 bot 演示启动 Royal；清理仍按下表绑定实际消息 store。
- 已有 Chat 独立 bot 测试改为注入记录请求的 BotControl fake；Royal 自己测试 Memory/PG writer、配额、幂等 client_profile_id、owner 校验和删除。fake 不另实现配额/持久化规则；本地 bot fixture 由 Royal 测试 setup 创建，不在 Chat 暗藏第二实现。
- 撤回“Royal 统一清理、Chat 一律不再 purge”的设计，也撤回“同一 URL/配置即证明同一 store”的前提。身份权威与消息存储可以分离；实际 purge 必须落到接收/查询这些消息的那个 MessageStore。
- 保持既有限额与协议状态编号；删除的补偿、授权和重试需要实质行为修复，不能标为纯接口搬迁。为保证创建与删除的生命周期排序，Royal writer 采用 owner 级锁；它也顺带串行配额检查，不再保留这条已被该锁覆盖的并发竞态。
- R05c1–c3 先在当前 Chat/Royal 文件布局中完成删除回执、存储路由与接口替换，不等待 R06 抽出 client crate。其后 R05b 才迁移剩余共享实现，避免把 bot 身份 writer 顺带移进共享库。

**Bot 删除的存储路由与补偿合同（R05c）**

现状：Chat 先 lookup/owner 校验，再 users.delete_bot，随后 store.purge_peer_dm；Royal 的 DELETE /api/v1/bot 同样先删身份、再 purge。HTTP 模式因此有两次 purge；任一次清理失败时身份可能已删除。Memory/PG 的 delete_bot 对已不存在的 bot 返回 NotFound，当前 Chat 的前置 lookup 也会阻止再次清理。这是待修复现状，不是目标幂等语义。

| Chat 实际消息 backend | 删除协调器调用的清理实现 | 支持范围 |
|---|---|---|
| HTTP Royal store | 该 MessageStore 的 HTTP adapter 调 Royal purge-dm 的作用域校验版本，由 Royal 的 st.store 执行 | 单 Royal Memory 实例或共享同一 PG 数据集的 Royal pool；独立 Memory 实例不能当可互换 HA 节点 |
| Chat 直连 PG | 当前 Chat MessageStore 已绑定的 PgPool 执行 purge | Royal 身份库可以是另一份 DB；不比较两端 URL，也不要求 Royal 能访问 Chat 数据库 |
| Chat 进程内 Memory | 当前消息 handler 与协调器共享的同一个 Arc/store instance 执行 purge | 本地/测试单 Chat 进程；重试必须命中同一实例，不承诺跨进程/重启恢复，不伪造远端清理能力 |
| 未知 backend、清理目标不匹配/不可达 | 不删除身份，不开始新的操作；已进入 PendingCleanup 的操作保持 pending | 返回明确失败，不能换一个空 store 当作原 store 已清理 |

启用远端 bot 控制时，bot 身份/owner 查询也使用 Royal 的公开目录投影，不在 Chat Memory 中维护另一份可写 bot 身份；普通消息直连/Memory 并不意味着目录也必须是该进程的 Memory。没有 Royal endpoint 时维持 bot 控制不可用的明确策略。

为重试绑定清理目标，MessageStore 暴露只读 StoreScope：PG 数据集使用持久化 UUID（独立克隆的数据集必须换 ID），Memory 使用该对象/进程生命周期的 instance UUID，HTTP adapter 取得并校验远端实际消息 store 的 scope。协调器从已装配的实际 store 取得 scope，不能由客户端提供 URL 或仅凭配置字符串构造。

scope 的证明方是持有消息的 store，不是 Royal。直连时 Royal 看不到 Chat 的库，步骤①只把 Chat 报来的 scope 原样写入回执，不能据此证明 UUID 属于那份库，也不能代替步骤②的核对。purge 在同一资源上拒绝 scope 不符的调用；HTTP 的作用域校验在 Royal 消息端完成，避免 pool 切到另一份 Memory。Royal 证明的是 actor 与 original_owner、以及 deletion_id；store 证明消息在这份库里。

Royal 的 BotDeletionRecord 保存 deletion_id、app、bot_account、original_owner、cleanup_scope 和 PendingCleanup/Done 状态。PG 回执必须与身份删除在同一事务提交；Memory 回执与身份由同一私有状态所有者原子更新，仅承诺当前生命周期。回执先于或与身份删除生效，禁止先删行、再另写一条可能丢失的授权记录。bot account 不复用；如未来允许复用，必须增加 generation 并重新设计旧请求识别，不能沿用旧回执。

```text
Chat BotDeletionCoordinator（持有实际 MessageStore）
  │ ▼ ① Royal delete_identity_or_resume(app, actor, bot, store_scope)
  │       └─ 每次都用当前 actor 对 original_owner；通过才删身份或返回既有 PendingCleanup
  │ ▼ ② actual_store.purge_peer_dm(expected_scope, app, actor, bot)
  │       └─ store 核对 scope；不符则拒绝。失败返回错误、保留回执
  │ ▼ ③ Royal complete_delete(deletion_id, actor, store_scope)
  └─ 只有 purge 成功且回执 Done 后，向客户端返回 Success
```

- resume 的输入必须带当前 actor。Royal 把它与回执的 original_owner 比较：不匹配返回 NotBotOwner(112)，不 purge，不换绑回执。匹配才进入步骤②。取消的只是 lookup_bot 里「行已经不存在 → UserNotFound(108)」这一支；身份还在、当前账号不是 owner 时仍是 112，与今天 bot.rs:126 的比较相同。不能把 lookup 失败的整类结果当成可以跳过的早退。
- 108 只用于「没有身份，且没有这份 actor 的匹配回执」。知道 bot_account 但不是 original_owner 的账号，不能借 pending 回执清理双方 DM。第一次删除有 owner 检查，重试也有。
- cleanup_scope 一经开始不可换绑。步骤②由该 store 用自己的 scope 拒绝另一份库；Royal 不证明 scope 真伪。Memory 重试到另一进程或新建实例返回失败并保留 pending，不能因新 store 为空自动报 Done。生产需要重启恢复时使用 PG 或其他可恢复消息 backend；单进程 Memory 的生命周期限制写进本地运行说明。
- 完成回执失败、响应丢失、进程在②后退出，都允许重复调用同一目标的幂等 purge，再提交 Done。Done 回执重复请求返回 Success；不能因为 live bot 行没了返回 NotFound。Pending 回执不得 GC，Done 至少保留能校验 original_owner/目标与幂等键的紧凑记录；不能先删授权痕迹再把缺失当成功。
- 现有 Royal DELETE /api/v1/bot 在兼容期继续“身份删除加其自身消息清理”，但内部也走回执语义，修复本来已有的清理失败后 NotFound。新 Chat 只调用身份阶段接口加其实际 MessageStore 的清理接口，不能先调旧完整删除、再本地 purge。Royal 的独立 purge-dm 接口保留，复用其清理实现而不是把它误当身份删除。
- 新内部 identity/resume/complete 与 store-scope/scoped-purge 合同单独声明，HMAC、owner、scope 校验和版本支持一起验收。旧 endpoint 不认识新字段不能被当作已支持；版本不满足返回不可用，不自动回退到有双清理问题的老流程。
- 发布顺序先支持新回执/清理接口，再切 Chat。切换前排空或处理旧流程的未完成删除；历史上已经删身份但没有回执的记录不能凭空重建 owner 授权，必须列为单独修复项。新流程的 Done/Pending 按作用域解释，不能把旧 Royal 自身 store 的 Done 误当成另一份 Chat Memory/PG 已清理。
- PG 回执和 StoreScope 元数据需要新增后续迁移，归唯一 schema 序列；不改 0001–0016。跨 Royal 身份库与 Chat 消息库的步骤①–③不是一个事务，不在数据库事务里等待 HTTP；待清理阶段通过回执恢复。该迁移与内部接口作为 R05c 子切片，不能暗藏在 R05b 的路径搬迁里。

失败映射与重试验收必须逐项固定：

| 状态/故障注入点 | 客户端既有状态 | 重试要求 |
|---|---|---|
| 身份存在但 actor 非 owner；或回执 owner 与当前 actor 不匹配 | NotBotOwner(112) | 不删身份、不 purge、不换绑回执；重试同样比较 actor |
| 身份不存在，且没有这份 actor 的匹配回执 | UserNotFound(108) | 不授权清理任意会话；不得用别人的 pending 回执代替授权 |
| 身份删除成功，purge Backend/超时/HTTP 5xx | SystemException(99)，不是 Success | 身份保持已删，PendingCleanup 保留；同一 actor 再次请求只跳过「行已不存在 → 108」，owner 仍核对，通过后继续 purge |
| purge 返回其他既有 StoreError | 保留 store_status：Invalid/HTTP400→InvalidPacketBody(101)，HTTP403→NotBotOwner(112)，NotFound/HTTP404→UserNotFound(108) | 这是清理阶段错误；不删除 pending，也不把它解释成“身份尚在”或禁止再次恢复 |
| purge 已成功，complete_delete 失败或结果未知 | SystemException(99) | 重复 purge 安全，再写 Done；若查询已 Done，返回 Success |
| StoreScope 不匹配，或原 Memory 实例已不在 | SystemException(99)，有可诊断的阶段错误 | 不清另一份 store、不完成回执；不能自动变更目标 |
| bot 控制能力/服务版本未配置或不支持 | ServiceUnavailable(3) | 开始删除前失败；不回退本地身份 writer |
| purge 成功且回执 Done（包括重复请求） | Success(0) | 不要求重新找到已删除 bot |

本矩阵处理权威消息/索引/inbox/读状态的清理，不等于已推送到客户端的展示缓存会自动消失；客户端投影失效仍沿既有删除响应/刷新机制验收。删除期间的新 bot 写与已接受事务的排序也要在该 store 的行为测试中覆盖，不能只测空库重复 purge。

**Schema 的编译依赖、查询 SQL 与运行执行分别约束（R05a）**

0001–0016 原样迁入 kim-server-schema/migrations，只有该库使用 sqlx::migrate! 嵌入。版本号、SQL 内容与 checksum 不变，不按消息/认证重编号拆历史序列。kim-server-migrate 独立可执行文件链接此库，生产 migration job、本地 tools/dev 和 E2E setup 都以外部进程执行它；Chat/Royal 只接收已经初始化的 pool，不调用链接进进程的迁移 helper。

| 层次 | 允许 | 禁止/验收 |
|---|---|---|
| schema/migrator | schema 编译完整历史 DDL，runner 链接 schema 并执行 | 其他 crate 的 normal/build/dev 闭包不得含 schema；runner 不把认证 adapter 带给 Chat |
| Royal 私有认证 adapter | 编译 password_hash/token_epoch/device_credentials 的查询与写入 | 唯一 DML owner；保持认证事务语义，不能复制给 IM adapter |
| Chat、kim-server-data、kim-royal-client 的 IM 查询 | 显式选择公开列；公开 upsert 可使用表的既有默认值 | 查询 SQL、QueryBuilder 片段、include SQL、生成查询元数据不得引用认证列/表；users 上的 SELECT * / RETURNING * 也禁止 |
| 在线进程 | 使用已迁移 schema、连接数据库 | 不嵌入迁移目录、不依赖 migrator/schema，包括 dev/build helper 的传递依赖 |
| 本地/E2E/CI setup | 外部 migrator 创建完整测试 schema | 不以测试方便为由给 Chat 添加 schema dev-dependency |

已核对的当前 PG 源码使用 sqlx::query/query_as/query_scalar **函数**；未发现对应的 query!/query_as!/query_scalar! 宏。当前唯一迁移嵌入位于 services/chat/src/store/postgres.rs:69，这个旧引用必须删除，不能新建 schema 后仍保留。

未来采用 query! 时，它在编译时依赖可检查的数据库或对应 .sqlx 离线元数据，并不要求把迁移目录嵌入消费者。schema job 解决运行时初始化，prepare/query! 解决编译时检查，migrate! 解决 DDL 嵌入，三者不可互相替代。[SQLx query!](https://docs.rs/sqlx/0.9.0/sqlx/macro.query.html)、[SQLx migrate!](https://docs.rs/sqlx/0.9.0/sqlx/macro.migrate.html)

prepare 阶段用外部 migrator 初始化临时数据库，按查询所属包生成/校验元数据；IM 查询只带公开列，认证元数据不共享给 IM 包。边界门同时检查 Cargo 可达依赖与实际 SQL（包含 SQL 文件/动态拼接/QueryBuilder），不是对 token_epoch 这个词做全仓禁用：协议字段和 Gateway 的 Redis epoch 校验不是 IM 认证 SQL。静态归属门不等同于数据库权限隔离。

本次默认保留已有 query 函数，不顺手改成 query!。未来若启用宏，应为每个构建产物准备仅含该产物查询的离线元数据输入；若固定版本工具只能使用混合根 .sqlx 缓存，就记录该构建输入例外及拆分方案，不把它写成认证 SQL 已从 IM 输入剥离。

如某个现有工具链确实无法在不嵌入认证迁移的情况下构建 IM query!，必须登记具体包、宏位置、原因与退出切片，作为尚未完成的例外；不能把 schema 已隔离写为通过。当前没有查询宏导致的必要例外；旧 Chat migrate! 和混合 users.rs 是待移除的已知违规，不是永久豁免。

可通过短暂 re-export 维持现有 import，要求切换所有生产消费者后在后续切片删除；不长期保留两套实现。

### 4.2 提取 Royal HTTP adapter

`kim-royal-client` 接收当前 royal.rs 与 pool 的 HTTP adapter、HMAC、endpoint 选择、熔断、重试和 deadline。依赖 kim-server-data 的 IM 合同、protocol、reqwest/Tokio 等通用库；禁止依赖 kim-naming/Consul、kim-metrics、chat/royal 服务包。

现有 RoyalPool 同时持有 Naming 与 KimMetrics，并自行 spawn 发现刷新；不能整文件搬进 HTTP adapter。最小 seam 采用“调用方推送 endpoint 快照”，不新增可变更范围过大的 resolver：

```rust
// 设计草图：类型由 HTTP adapter 自己定义，不暴露 DefaultRegistration/KimMetrics。
pub struct Endpoint { pub id: String, pub base_url: Url }
pub trait RpcObserver: Send + Sync {
    fn completed(&self, sample: RpcSample);
}
impl RoyalPool {
    pub fn replace_endpoints(&self, endpoints: Vec<Endpoint>) -> Result<(), ClientError>;
}
```

Chat 装配层拥有 Consul/Naming 查询循环，把 registration 转成 Endpoint 后更新 pool，并拥有刷新 task 的取消/回收。pool 只接收快照；静态配置使用同一入口。RpcObserver 在 Chat 装配层适配 KimMetrics；测试注入记录器或 Noop。指标标签不带 URL/token，高基数控制仍由进程侧实施。

- 公共入口仍返回 MessageStore/Directory/AgentSpecStore 的实现，调用者不需要知道 HTTP 编解码。
- 保留 deadline、幂等语义、重试状态集合、熔断与现有发现刷新周期/空列表 fallback；这些行为通过组合验收，不要求同处一个 crate。`with_rpc_deadline` 的 task-local 作用域不能被另一次 spawn 隔断。
- endpoint 稳定 ID/URL 不变时复用当前 client 和熔断状态；更新原子发布，删除与恢复的行为覆盖测试。发现失败、空快照、静态 bootstrap 各自保持既有策略。
- 同一失败只在入口映射一次：adapter 保留结构化错误，Chat/HTTP handler 映射已有协议状态，保留现有 404、503、超时区别。
- 不给每个 HTTP endpoint 再套一个只转发参数的 service 类。

### 4.3 提取通用网关运行库

```text
gateway  ──► kim-ws
    └──────► ★ kim-gateway-runtime ◄────── tgateway
                                            └─► kim-tcp / TlsFrontend
```

箭头表示编译依赖。两个进程都依赖运行库；只有 WS 入口依赖 kim-ws，TCP/TLS 入口继续留在 tgateway。

迁移 `GatewayHandler`、route selector、slots、公共配置、revoke adapter、hooks 和 run。保留 `run_gateway(config, server: impl Server + ...)` 的现有复用形状。

| 现有代码/符号 | 目标 | frontend 约束 |
|---|---|---|
| lib.rs:317 GatewayHandler，ChannelMeta，heartbeat、accept/receive/disconnect | runtime::handler | 只使用 Conn/Server/Frame/OpCode，不出现 WsServer/TcpServer 具体类型 |
| lib.rs:136 RevokeCheck/AllowAllRevoke、156 RevokeStore、229 HttpRevoke | runtime::revoke | 保留 Redis/HTTP 两种既有 adapter；不得移动 Royal 的写入实现 |
| lib.rs:648 MetricsHook、909 KickHook | runtime::hooks | KickHook 持有 dyn Server；“Attach WsServer”注释改为通用 server，实际类型已经通用 |
| selector.rs、slots.rs | runtime 的同名模块 | 属于后端路由/容器装配，不得以 frontend 协议分叉复制 |
| run.rs 的配置加载、容器/发现/metrics/revoke 装配和 shutdown | runtime::run/config | 函数接收 S: Server + Send + Sync + 'static，不创建 WS listener |
| gateway/main.rs:4/18 的 WsServer::bind | services/gateway | kim-ws 正常依赖只属于此入口 |
| tgateway/main.rs 的 TcpServer 与 tls.rs 的 TlsFrontend/证书装配 | services/tgateway | kim-tcp/rustls 正常依赖属于此入口；runtime 的内部容器 TCP 依赖单独保留 |
| 各 frontend 的真实 WS/TCP/TLS E2E | 进程级 integration tests | runtime 单测只用 ScriptedConn/RecordingServer；不能以 dev 依赖掩盖普通依赖漏入 kim-ws |

验收同时要求 `tgateway → kim-gateway-runtime` 与 `gateway → kim-gateway-runtime` 存在，`tgateway → services/gateway` 不存在；runtime 普通/build 的可达依赖无 kim-ws。不是要求 TGateway 不依赖任何网关库。

- gateway 包只绑定 WS frontend 并调用运行库。
- tgateway 包只绑定 TCP/TLS frontend 并调用运行库。
- 运行库不依赖 kim-ws；内部容器依赖 kim-tcp 仍合理，不能把服务间 TCP 与客户端 frontend 混为一谈。
- 保留握手 fail-closed、心跳 revoke 有界宽限、JTI/epoch/device 检查、会话 TTL、摘发现后 drain、背压和 channel lane 语义。
- package 名和产物名 `gateway/tgateway/chat/royal/router` 第一阶段不变；部署地址、协议与进程数不变。

## 5. Agent 配置、执行与生命周期设计

### 5.1 数据模型与执行适配分开

```text
★ agent-model ◄── agent-codec ◄── SDK
       ▲              ▲
       │              │
   agent-host ◄── desktop-runtime ──► SDK 公共接口
       │
       └─► Goose / Codex / MCP / tools
```

此图所有箭头表示编译依赖；agent-model 不依赖 Host。codec 是跨格式兼容 adapter，SDK 仍可用它提供手机端配置同步，不赋予手机执行能力。模型的无 I/O 验收与 codec 的跨平台/兼容验收是两项。

从 Host 的 profile/capability/catalog/skills 中提取纯数据定义，而非整文件搬移。`profile.rs` 当前同时引用 GooseMode、ProviderConfig、HostError、catalog、skills，直接移动会复制错误依赖。

`kim-agent-model` 持有：

- AgentProfile 的持久化形状、ProviderSpec、ModelSpec、CapabilityRef、SkillRef、Workspace/Sandbox/Permission 的配置值。
- KIM 自己的 RuntimeKind、ApprovalMode、ReasoningChoice 等闭集类型。
- 不触碰文件系统的校验、序列化默认值、静态 provider/catalog 数据。

Host 持有：配置转 Goose/Codex 参数、provider 构建、模型网络查询、技能扫描/文件读取、MCP 子进程、工具执行、sandbox、运行时状态。

GooseMode/ThinkingEffort 的引擎类型转换放在 Host adapter 内；磁盘 JSON 的历史值、别名与 protobuf 对应由 codec 兼容层负责。两处职责不能因目标 model 有 KIM enum 就混成一层。profile.rs 中 mode 就是 GooseMode，不能整文件移动。JSON 字段名、默认 SmartApprove、缺省 runtime 的 Goose 语义、protobuf 编号、schema_version、extra pocket 和未知字段策略以 fixtures 固定，不能因为内部 enum 重命名改变磁盘与同步协议。

codec 保持当前 blob/JSON 兼容接口，改成依赖 agent-model；删除 goose-provider-types 与 Host 依赖。手机目标的 normal/build 可达图分别检查 model 和 codec，不允许 kim-agent-host、goose-*、rmcp、Codex 或 helper。再单独执行目标编译和旧数据 fixture，证明“无执行依赖”和“兼容可用”；不能用一个“纯库”标签代替。SDK 的 profile、provider account、device overlay、云同步仍保留。

### 5.2 调度由桌面 runtime 持有

把 SDK 中 MobileAgent 的队列、LRU、执行重试、busy 心跳和 runtime adapter 移到 desktop-runtime，命名为 `AgentScheduler`。SDK 保留消息处理后触发 Agent 的接口与 Noop 实现。手机不会创建 Scheduler，但可以同步 profile 数据。

以下是接口设计草图，类型名表示目标合同，不是可直接编译的补丁：

```rust
// kim-sdk 的公共合同；不暴露 Store/Inner/Host。
pub struct SessionScope {
    pub account: AccountId,
    pub epoch: SessionEpoch,
    pub cancel: CancellationToken,
}

pub struct AgentTurn {
    pub conversation: ConversationKey,
    pub in_reply_to: MessageId,
    pub text: String,
    pub scope: SessionScope,
}

#[async_trait]
pub trait AgentPort: Send + Sync {
    async fn enqueue_turn(&self, turn: AgentTurn) -> Result<(), SdkError>;
    async fn catch_up(&self, scope: SessionScope, dests: &[ConversationKey])
        -> Result<(), SdkError>;
    async fn end_session(&self, epoch: SessionEpoch) -> Result<(), SdkError>;
}

// desktop-runtime：持有队列、取消令牌、task handles；只通过 SDK 公共命令回写。
pub struct DesktopRuntime { /* AgentScheduler, Host sessions, permissions, task set */ }

impl DesktopRuntime {
    pub async fn shutdown(&self) -> Result<(), RuntimeError>;
}
```

AccountId/MessageId 可沿用已有类型或在确有混用风险的接口引入，不要求为所有 String/u64 建 wrapper。Noop、Scripted 与真实 Host 已是实际 adapter，适合保留可替换接口；不要为每个内部函数新增 trait。

上述 AgentPort 是“SDK 调度执行”的方向，不覆盖“执行器访问 SDK 数据”的反方向。反方向定义不暴露 Store 的 AgentDataAccess，位于 SDK，内部只持有 Weak<Inner>：

```rust
// 设计草图；操作内部升级弱引用、检查 scope、执行一次有界操作。
impl AgentDataAccess {
    pub async fn prepare_turn(&self, scope: &SessionScope, dest: &ConversationKey)
        -> Result<PreparedAgentData, SdkError>;
    pub async fn pending_turns(&self, scope: &SessionScope, dest: &ConversationKey)
        -> Result<Vec<PendingTurn>, SdkError>;
    pub async fn publish_busy(&self, run: &RunScope, active: bool) -> Result<(), SdkError>;
    pub async fn publish_turn(&self, run: &RunScope, status: TurnStatus) -> Result<(), SdkError>;
    pub async fn enqueue_reply(&self, run: &RunScope, reply: AgentReply)
        -> Result<CommandReceipt, SdkError>;
}
```

| 当前 worker 调用 | 目标操作与条件 |
|---|---|
| sdk.store + profile_id_for_dest，session_snapshot | prepare_turn 校验 account/epoch，读取 profile/provider/overlay 与会话事实；返回拥有型数据，不返回 Store/连接/令牌 |
| protocol.bot_pending | pending_turns，绑定旧 scope 的协议会话，取消后返回 SessionEnded/StaleEpoch |
| set_bot_busy | publish_busy，发送前验证旧 scope；不能重新读取“当前协议”而借用新账号连接 |
| emit_session_wait(AgentTurn) | publish_turn，事件带 account/epoch/run_id；订阅器过滤 scope |
| enqueue_bot_reply | enqueue_reply，入写队列和 writer 开始事务时校验 scope，保留幂等 clientId |
| execute_im_tool 的 SDK 查询/发消息 | 使用同一套 scoped query/command facade；不绑定未限定账号的 KimSdk clone |

弱句柄升级失败时返回 SdkClosed，丢弃该 run 的后续业务写入；不是另找一个 Store。task/waiter 的本地清理由 RuntimeOwner 自有状态完成，runtime 结束日志不需要升级 SDK。每次操作内部可以短暂强持有 SDK，等待队列、Host 生成和权限等待期间不强持有 SDK。

所有权与 Clone 规定：

| 对象 | 可否 Clone | 拥有内容与关闭责任 |
|---|---|---|
| AppSessionOwner / RuntimeOwner | 否 | 唯一承担 SDK/runtime 关闭责任，持有 task set 和 shutdown handles；SDK 数据句柄可以有副本；显式 close/shutdown，有界 join；Drop 发取消但不承诺异步清理已完成 |
| KimSdk | 保留 Clone | 只延长 SDK 数据对象，不强持有 RuntimeOwner/Host/Scheduler；所有副本共享关闭状态，不能重新启用已关闭资源 |
| AgentPort 的 client handle | 可以 | Sender + 已关闭状态，不能是 Arc<RuntimeOwner>；所有副本在 end_session/shutdown 后拒绝入队 |
| AgentDataAccess | 可以 | 只复制 Weak<Inner>，不公开 upgrade() 返回 KimSdk/Store |
| Scheduler worker/heartbeat/Host 回调 | 不持有 owner 强副本 | 使用 run cancel、弱 data access 和本地状态；worker 不保留自身队列的强 Sender 形成重试自环 |
| FFI 派生能力视图 | 可按 FRB 句柄要求复制 | 引用关闭状态和能力 client，不各建 RuntimeOwner，close 后所有视图失效 |

删除进程级 INSTALLED 的业务入口。Tokio/日志等进程初始化不保存账号 runtime；审计所有 spawn、Arc clone、watch sender、retry sender 和 static，不能只修改 MobileAgent.sdk 一个字段。bootstrap/layout 注入实例，同参数幂等，不同参数报冲突。

关闭与切换账号保持现有 stop_session 的“先 bump epoch，再停 Outbox/连接”方向：

```text
会话结束/切换
  │ ▼ ① 关闭旧入队许可；bump epoch，取消旧 session/run scope
  │ ▼ ② 立即发起旧连接、Outbox 与读同步的停止；不等 Agent 完成
  │ ▼ ③ 同时取消 Host/子进程/权限等待；end_session 等待 task 退出
  │ ▼ ④ 等待旧 Store 写入屏障及 task；超时则 abort、回收并报错
  │ ▼ ⑤ 清理旧账号投影；成功后允许建立新 scope
  └─ 完整退出时再关闭 store 级 publisher/writer/pool
```

不持有锁等待 end_session。①–② 不开启新账号且禁止新的旧 scope 外发；已经发送到网络的帧不能撤回。③–④ 为有界异步关闭，失败不能静默开启新账号。prepare/query 操作在 await 前后检查 scope；副作用使用固定 scope 的协议 handle 或 writer gate，不能用最后一次 current_session 查询替代。断线重连不执行账号结束流程，也不关闭 store/query publisher。

旧 epoch 已废、Store 尚在时的验收时序：

| 动作发生点 | 明确结果 |
|---|---|
| 失效后新提交的旧 reply/profile 写/业务状态事件 | 返回 StaleEpoch/SessionEnded，不能写新账号或对外发布 |
| 已入队但 writer 尚未开始的旧账号写 | writer 复核 epoch，拒绝并完成返回通道，不能悬挂等待者 |
| 失效前越过 writer 接受点、正在执行的事务 | 可完成旧账号事务，保持原子性；commit effect 带旧 scope，新会话不可见；新 scope 激活前等待写入屏障 |
| busy=true/false 或 Host 完成晚到 | 禁止取新账号 protocol 发送；旧网络已停直接结束，busy=false 不是阻塞关连接的必要条件 |
| run/task/waiter 清理、已接受写的 reply 通道 | 必须完成；只处理 runtime 私有状态/已有事务，不产生新账号业务写 |

事务接受点与失效顺序由单写队列和会话 gate 明确排序；不能做一次 current_epoch 比较就宣称解决 TOCTOU。R03 用屏障控制测试，让旧事务暂停在接受前/接受后，分别验证以上结果。若要禁止已接受事务在 bump 后完成，须另设提交屏障协议，不能在测试里假设已经存在。

权限与执行合同：

- permission key 包含 epoch、run/session ID、call ID，避免仅用 call ID 跨运行串扰。
- timeout、取消、拒绝和响应都移除 waiter；无界 listener 改为有界订阅或快照加事件，慢 UI 不能无限积压。
- 取消令牌传入 `prompt/complete_tool`，使用 session 子令牌，不在每次调用中创建与会话无关的新令牌。
- 保留一次重试、队列上限、重复入队防护、每会话串行规则；新增全局并发上限与空闲队列回收的具体数值以现有测试与产品需求确定，不擅自改变吞吐策略。
- AgentHost 的会话复用明确归 Scheduler/Host session map 所有，不能只靠每次 turn 相同的 session_id 假定状态自动跨新建 Host 保留。

### 5.3 合并应用 FFI 库

目标保留 `kim_client_ffi` 产物名，避免目录调整同时改变 Android OTA 与 Native Assets 的装载名称。Rust package/crate 与动态库命名的规范化另行考虑。

迁移顺序：把 catalog/skill 接口迁入主 FFI，Flutter 切换调用，删除第二套 FRB 生成器配置和 `AgentRustLib.init`，最后删除 kim-agent-ffi workspace 成员与旧生成文件。

主 FFI 的 KimHost（当前 Dart adapter 期待的名字）拥有应用会话资源，提供 session/inbox/directory/settings/agent 等能力视图。能力视图共享同一资源所有者，不各自建立 SDK、数据库或 Host。

- Agent 数据编辑/同步在手机可用与否由产品功能决定；执行、扫描用户目录等桌面能力由 RuntimeCapabilities 明确返回。
- unsupported 能力返回明确的错误或 capability；不能用空列表/空成功伪装一次执行成功。
- DTO、错误映射、StreamSink 转发留在 FFI；配置校验、消息规则和持久化仍在对应 Rust 深模块。
- watch 生命周期与 handle/订阅取消绑定；UI 退订后 Rust pump 必须退出。关键状态用可重建快照，事件通道不承担唯一状态源。
- SecretStore 的当前实现为空，必须先恢复到能验证的设计。目标保持密钥通过一次表单提交进入 Rust/平台凭据库，后续用 key_ref；权限批准不承担明文密钥传递。
- 合库前后都做 native load 与 FRB 生成验收；不能仅凭 Rust 单元测试断言动态库和 static 状态正确。

## 6. Flutter 应用内部设计

继续一个 Flutter app。手机与桌面采用自适应 UI 与能力差异；只有出现独立产品导航、维护团队或发布节奏，才评估拆入口 package。

- `bootstrap/`：boot、平台事实、依赖注入、应用级关闭。main 只进入 bootstrap；不向 feature 分散初始化顺序。
- `bridge/`：唯一接触生成 FFI 的 adapter 层；对外公开 auth/session/inbox/directory/settings/agent/media 的 ports、DTO 和错误。
- `features/`：providers、UI 与必要的展示映射。通过对应能力 port 调用，不获取 KimHost 原始句柄。
- `design/`：接收显示数据与交互回调，不 import FFI 或业务 provider。
- `core/`：平台能力、环境、展示工具；避免逐渐收进消息状态机与 Agent 编排。

R09 的扫描范围是整个手写 app/lib，不限于 features。当前 25 文件的完整名单与 43 条语句位置写入 dependency-audit.json；main 1、core 2、bridge 6 也在验收清单内。目标白名单如下：

| 路径角色 | 直接引用生成 FFI | R09 处理 |
|---|---|---|
| lib/ffi/** | 允许生成器内部引用 | 标为生成代码，不统计成手写 adapter |
| lib/bridge/src/ffi_adapter/** | 允许 | 收纳当前真正的 FFI 调用/映射；按路径批准，不能把全部 bridge 一律判违规 |
| bridge 的 ports/contracts/public exports | 不允许直接 import 或转 export 生成类型 | 当前 kim_ports/conversation_port 的泄漏归入 R09，公开语义用稳定类型表示 |
| core/errors.dart、core/failures.dart | 不允许 | FFI error 类型转换迁至 adapter，core 只接收稳定应用错误 |
| main.dart、bootstrap/** | 不允许 | 移除 agent_catalog 直接调用；通过 bridge factory 启动和设置平台事实 |
| features/**、design/**、models/**、router/** 及其余手写 lib | 不允许 | 统一依赖公开合同；design/models 各 1 个现有引用也不能漏 |

边界门同时识别 package、相对路径 import/export 与公开转导出；tests 中的 bridge adapter 测试单独批准，其余测试经公共合同。现有 bridge 6 个引用文件不是 6 个永久豁免：其中 adapter 引用合法，ports 泄漏要消除。目录从 app 改为 apps/kim 后白名单由同一应用根定位规则重定位。

先统一 import 出口，再对确有稳定语义需求的 DTO 映射。不要机械复制每一个生成 struct；没有差异的不可变数据可以集中导出，但这仍属于“生成类型暴露”的过渡状态，必须明确哪些接口承诺隔离生成器。

集中导出只作为带文件/到期切片的过渡例外；R09 完成时公共 ports 不再转导出生成 FFI。未被公共合同使用的生成类型不需要额外镜像一份。

把 chats 对 agent providers 的引用改为在组合处注入 `LocalThreadSummary`/等价展示投影：`ThreadsRepository.project(remote, localThreads)` 不负责读取 AgentProfile/provider。这样 chats 的展示不需要知道 Host profile 的存储形状。

客户端 ID、会话 epoch、消息发送状态机、重试和未读去重继续在 SDK。Dart 仅持有展示状态与用户意图；不要用拆 package 的方式复制这些业务规则。

保留 `kim_media_picker` 原生插件、kim_lints 与 Native Assets hook 的现有职责。新增 bridge 边界检查应按 AST import/export 或足够明确的规则实现；页面行数门只作为维护提示，不作为架构正确性的证明。

## 7. Web、协议和 Media 设计

### 7.1 拆 Web 应用和 SDK

- apps/web 获得当前 app、worker.ts、Vite app 配置、SPA assets 与 wrangler 发布配置。
- sdk/web 保留协议、连接、心跳重连、状态、token 更新、ACK/离线、消息数据、media 客户端和纯 SDK 测试。
- examples/web-client 接收最小 demo；kimbench 作为压测工具迁到 tools/kimbench，pkt-client/kim-client-demo 保持示例身份。
- 应用通过 `@kim/web-sdk` 的 public exports 引用；需要的 WireProfile/Presence 等只挑实际消费者导出，或转为面向消费者的 DTO，不开放 `src/*` 万能出口。
- ChatSession 的浏览器 localStorage、用户提示、UI 状态映射留在 apps/web。ChatProvider 先迁移保行为，再按 session、inbox/timeline、UI action 切为应用内部模块；不把 React Context/Riverpod 式业务状态强行塞进 SDK。
- SDK 发布/打包结果不得依赖 React、路由、Tailwind、sonner、Worker 环境类型。用打包后 import 验证 public exports，防止只在 Vite source alias 下能工作。

采用根 npm workspaces，沿用 npm 并固定 CLI 版本。先在现有位置登记 sdk/web、sdk/media 并完成根 lock 整合，再在原 app 目录拆 SDK/应用包，最后独立迁到 apps/web 与 services/media。

npm 工作区内引用采用普通版本范围；根 workspaces 中已登记、name 与版本匹配的本地包自动链接。所有安装从根执行，CI 检查 lock 中 SDK 的 link 记录及 node_modules symlink 的真实目标；不能仅以 package.json 中出现 0.1.0 就推断本地链接，也不能在脱离根 workspace 的目录发布流程里假设仍然本地解析。

工具行为已用本机 Node v26.8.1/npm 11.19.0、临时空缓存、offline 安装验证：匹配的 0.1.0 链到本地包；workspace:* 与 workspace:^ 返回 EUNSUPPORTEDPROTOCOL。npm 官方文档的 workspace 互依示例也使用普通版本范围并自动链接。结果见 tool-behavior-checks.json。若以后更换 CLI/包管理器，重新验证，不能把 npm workspace 功能与 workspace: 依赖协议混为一谈。

初次整合 lock 单独提交，不挪应用目录、不升级依赖；拆包只允许增加本地包/link 与职责分配必需的元数据变化，物理迁移时 lock 只更新路径。第三方 Codex 自己的 pnpm workspace 不纳入根 npm workspace。

### 7.2 协议一个源头，跨语言生成可验证

先保留 proto 在 kim-protocol，避免无收益地增加顶层 contracts 目录。移动 crate 时更新 Rust build.rs 与 Web gen-proto 的相对定位。

- pkt.proto 与 agent.proto 各有明确消费者；当前 Web 脚本只生成 pkt，未实现的 Agent Web 能力不假装已有生成链路。
- Rust 继续 build.rs 生成，Web descriptor 继续受版本控制并可重复生成。FRB 是从 Rust API 生成，不把它混同为 protobuf 生成。
- 生成器、protobuf/FRB 版本与 fixture 固定；生成命令可从任意工作目录运行，重新生成后差异必须可解释。
- 对帧 magic、command/status 数字、protobuf 字段、密码信封、图片 extra、ACK ID 集合做跨语言 fixture 验证，不只各语言独立 round-trip。
- shared protocol 变化触发 SDK、server、Flutter bridge 和 Web 相关消费者检查。

### 7.3 Media 与边缘入口

Media 独立迁到 services/media，保留域名、R2 binding、对象 key、返回 JSON 和 Royal 鉴权协议。前端 Worker 留在 apps/web，因为它与 SPA 同一发布单元。

运行时依赖仍是客户端上传 → Media → Royal auth/me 与 R2。把 Royal 401、503、超时与错误响应作为契约测试；不能因移动目录就改成下放 JWT secret 或改变撤销语义。Web Worker 的 `/api` 与 WS 回源规则、Route/Custom Domain 配置也按原合同验收。

## 8. 构建、原生链接与发布

### 8.1 一个根 workspace，按产物选择包

维持一份 Cargo.lock，根 patch 与原生 SQLite 约束保持统一。default-members 只改善无参数命令，不是 manifest 解析、第三方源码或锁解析隔离。

- root sqlx 只声明共同的最小配置，postgres/sqlite/migrate/macros 由真正使用的成员声明；所选包的 feature 仍可能统一，必须通过目标图确认。
- 手机检查只选主 FFI 的手机目标，不使用 `--workspace --all-features` 代表手机产物。
- 桌面可以启用 Host 的 Codex feature，手机的可达普通依赖图应不存在 Goose 执行包、Codex、rmcp 与 helper。
- 依赖图排除不等于无需 checkout：一个 workspace 中的可选 path dep 和锁解析仍可能要求对应 manifest 存在。分开验收“不会编译执行引擎”和“无需第三方源码初始化”。
- 保留 SQLite/SQLx pin，直到桌面合库与 phone build 都验收。检查 `cargo tree -d`、links 冲突、Native Assets 与最终 dylib/SO 依赖，不能随意删除兼容约束。
- Rust 工具链、Codex revision、Flutter/FRB 版本保持当前基线，升级另开变更。

### 8.2 服务端 Docker 输入分三层验证

区分：① Cargo 解析 manifest/锁/feature 所需的闭包；② 选定 server target 的 normal/build 编译单元和资源闭包；③ Docker 构建上下文与最终 runtime image。解析到 Codex 的 manifest 不代表必须复制整个 Codex 源码，更不代表最终服务镜像需要它。

提供 tools/build/prepare-server-context，先在临时目录验证两种候选，均不改真实根 workspace：

1. **保留解析图的 manifest 骨架。** 包含根与 path/workspace 继承涉及的全部 manifest、必要的 target 声明和解析锚点；只为实际 server 编译闭包复制真实源码、迁移与生成资源。未选桌面包的 target 若依赖自动发现，需要保留或显式声明 target 形状。不能只复制若干 Cargo.toml 就宣称充分。
2. **生成 server 子集 workspace。** 裁减临时 members 与未引用路径声明，保留有效 patch/profile/lints/toolchain；试验原根 lock 是否可用 --locked。不能静默重写 lock 掩盖差异。

本机 Cargo 1.95 的最小 offline 实验已确认：未选 host 的 optional path 包只有自动发现式 manifest、没有源码 target 时，server check 仍报 no targets specified；显式 lib.path 后，不存在该 lib 源码也能只编译 server。实际 Codex 同时存在显式与自动发现的 target、独立 workspace 继承、传递 path 依赖，因此此实验不证明真实 manifest 骨架已经完整。

真实验证要求：

- 列出 manifest 解析闭包与 server normal/build 可达闭包；对当前 server 包、实际 target、features 分别记录，禁止用 --all-features 的全仓图代替。
- 准备结果不含 Codex 真实执行源码；需要 target 锚点时在临时骨架明确标识为不可编译占位，若误选必须失败，不能生成可被错误发布的假库。
- 在干净目录执行 metadata/locked server build，核对构建的 package/artifact 清单没有 Host/Codex/Goose/helper，并完成服务器 smoke；锁/feature/package 解析身份与基线一致才验收。
- 检查实际发送的 Docker context 清单及 runtime image 内容。只在 Dockerfile 不 COPY 源码、但 build context 仍上传全树，也不算隔离完成。
- 任一候选失败就记录缺少的 manifest/target/锁差异并继续修正；在方案验证前不更改发布流程，不以“先复制整个 third_party/codex”作为默认回退。

只有实际 locked 隔离目标无法实现时，才单独评估独立 server workspace/lock 的维护代价。根 workspace 是否保留与 Docker 输入是否精简是两个决策。

### 8.3 统一操作入口，保留不同发布单元

提供 `tools/dev` 或根薄任务入口，转发到现有 Cargo/npm/Flutter 命令：`check server|mobile|desktop|web|media`、`gen protocol|ffi`、`build server|android|macos|web`、`dev server|app|web`。一个检查包列表由脚本与 CI 共用，避免两边手写漂移。

- 服务器镜像、Web、Media、手机安装包、桌面安装包、Android OTA 仍分别发布，不使用一个全仓产品版本。
- `.github/workflows` 是唯一 CI 执行定义；app/ota 留打包/校验脚本与说明，删除或生成其 workflow 副本。
- APK/OTA 继续原 `libapp.so + libkim_client_ffi.so` 配对，保留 host_line、引擎/FRB 兼容、签名校验与回滚规则；这次结构迁移首发完整安装包，避免同时改变 OTA ABI 与文件布局。
- 改目录时同步 FRB rust_root、hook cratePath/root 检测、profile package 名、IDE/FVM、Xcode scripts、OTA 输入输出路径、Docker COPY、Dependabot、缓存目录和文档链接。
- 补回根 ignore 的合理规则，但不自动删除本机 node_modules、日志或 IDE 配置；清理 tracked/untracked 文件单独审核。

## 9. 验证设计

以下是实施验收要求，不代表本次已运行。优先迁移已有测试，只有新 seam 或已确认的生命周期风险增加行为测试；不为纯目录重命名新增镜像实现的测试。

| 检查单元 | 触发依赖 | 核心验收 |
|---|---|---|
| server 基础 | shared、server libs、services | fmt/clippy、端口层测试、Gateway WS/TCP/TLS、Chat/Royal E2E |
| server PG/Redis | server-data/session/认证存储/迁移 | 独立真实 PG/Redis，完整迁移与幂等/ACK/inbox 行为，不能以 Memory 代替事务验证 |
| bot 控制/删除 | BotControl/BotDeletionCoordinator/实际 MessageStore | 身份写在 Royal，清理按 HTTP/直连 PG/同实例 Memory 分别验证；失败保留回执、重试不依赖活跃 bot 行；无 endpoint 时不可用；共享目录无 count_bots/身份写口 |
| schema/SQL 归属 | schema/migrator、在线进程、SQL/查询元数据 | migrator 外部初始化；Chat/IM adapter 无 schema normal/build/dev 依赖与认证 SQL；query! 元数据和迁移嵌入分别检查 |
| mobile Rust | SDK/client/model/codec/protocol/FFI | 目标 cargo check/build、依赖图禁入 Host、Native Assets 装载、安装包检查 |
| desktop | runtime/Host/SDK/model/FFI/Codex pin | Scripted 行为测试、真实 native load、helper 路径与 sandbox smoke |
| Flutter | app、FFI public surface、SDK 语义 | analyze、现有 widget/state 测试、登录与发送/重连 integration |
| Web SDK | sdk/web、pkt.proto、根 npm lock | typecheck/单测/fixture、npm pack 后 Node/浏览器入口可用 |
| Web 应用 | apps/web、SDK public surface、边缘配置 | 类型检查/SPA build、登录/聊天/图片上传、HTTP/WS 回源 |
| Media | services/media、Royal auth 契约、根 lock | 现有 jwt/object/index 测试、Royal 错误映射、R2 写入契约 |
| codegen/边界 | proto/FRB 定义、manifest、规则 | 可重复生成、禁止反向依赖、禁止跨包 src/FFI import |
| release smoke | build/发布配置变更 | 干净 checkout 的 Docker/APK/macOS/Web/Media 构建；签名打包 dry-run |

必须保持的语义：

1. 服务端 persist-first；相同 clientId 的幂等结果与冲突语义不变。
2. 客户端只有落库成功后 ACK；重放不重复增加未读；Outbox 重启可恢复。
3. 旧 epoch 不向新账号写消息、权限结果、busy 状态或 provider 秘密。
4. Agent 取消后不再启动新工具；已开始的不可撤回外部操作如实报告结果，不把取消当作回滚。
5. 查询发布仍按 commit/sequence 排序；断线重连不销毁 store 级 publisher。
6. token/JTI/device 撤销、HMAC nonce 和 deadline 不因抽库失效。
7. 配置 blob/JSON/云同步、图片 metadata、帧与 protobuf 协议保持兼容。

关键新增行为测试：

- 关闭时即使保留 KimSdk/FFI capability clone，所有 scoped 操作返回 Closed/SessionEnded，worker/task/waiter 仍结束；丢弃全部数据副本后弱引用才必须不能升级。分别测试关闭与对象释放，不能混为一个条件。
- 账号 A 的 turn 运行中切到 B；旧回复、权限响应和重试不能进入 B；断网重连仍在同一有效 scope 下恢复。
- UI 退订/重订不会留下 pump，慢订阅者遵守有界策略；权限超时清理 waiter。
- 两次 turn 的 Host session 复用符合明确策略；分别验证 Goose 与 Codex adapter。
- 迁移前后的 codec fixtures 与两种 server backend adapter 得到相同公开结果。
- bot 删除：分别给 HTTP store、Chat 直连 PG、Chat 同实例 Memory 写入消息与会话状态；身份删除成功后注入 purge 失败，断言客户端 SystemException(99)、身份已删、回执 pending。恢复清理后由原 owner 重试，断言跳过的只是「行已不存在 → 108」，清理完成并返回 Success。另一账号持有同一 bot_account 重试，断言 NotBotOwner(112)、消息未清、回执 owner 不变。scope 不符的 purge 由该 store 拒绝，Royal 不把记下的 UUID 当作库的证明。
- bot 删除：验证无回执 NotFound、错误 owner、错误 StoreScope 不触发清理；PG 在身份删除后、purge 后/complete 前、Done 响应丢失处重启恢复；Memory 到另一实例不能假成功。HTTP 模式不得先调用旧完整 delete 再重复 purge，直连模式必须命中 Chat 的实际 store。

## 10. 分阶段实施与完成条件

每个切片单独提交、可独立回退。兼容导出只作过渡，不把下一阶段的运行前提藏进当前阶段。

| 切片 | 依赖 | 代码变更 | 完成条件/回退边界 |
|---|---|---|---|
| R00 结构基线 | 无 | 固定 HEAD 对照与工作区差异；仅修 manifest、模块导出/缺失声明使结构可解析，不实现新的 FFI/密钥/Host 行为 | metadata 成功；各空文件有 HEAD 对照与归属。若导出后签名/生成物不一致，列为 R04a 阻塞，不承诺此时全量可编译 |
| R01 依赖基线与边界门 | R00 | 分别记录 HEAD 对照/工作区解析图和 target/feature，建立分角色允许清单 | 新坏依赖失败；旧例外具名。构建时间/尺寸在实际可构建基线确定后补测，不把 metadata 当运行验证 |
| R04a 现有 FFI/密钥路径连通 | R01 | 以 HEAD 双库实现为对照并先验证基线，明确处理 KimHost、secrets、生成 API 与当前 Dart 调用不一致；不合库、不迁目录 | 已有认证/会话/密钥行为可测，FRB 与 Rust 声明一致，双库 native load。旧行为缺失不能从空文件推导为新设计 |
| R02 模型与 codec 解耦 | R01 | 提取纯数据 model；codec 保留 JSON/protobuf 兼容层，Host 做引擎转换；不实现 SecretStore/会话/FRB | 只构建 model/codec 及其 fixtures；解析手机目标图验证无 codec→host/goose-*/rmcp。完整 SDK/FFI/APK 构建归后续汇合验收，不阻塞此切片 |
| R03 桌面任务与关闭 | R02 | 提取 Scheduler；实现双向合同、Clone/弱访问规则、scope 取消与 writer 屏障 | 隔离逻辑按公共合同测试；真实 SDK 的旧 epoch/释放测试在 R04a 连通后汇合执行，不能未运行就标绿，也不能回流为 R02 前置 |
| R04b 单 FFI 装配 | R03/R04a | 汇合模型/生命周期与行为恢复两条线，完成真实 SDK 测试，再迁 catalog/skills、切 Flutter 调用、删第二生成器/动态库入口 | native load、旧 SO 名不变、OTA 两 SO 配对不变；R03 集成测试通过；与 R04a 分提交 |
| R05a 认证/资料/schema 归属 | R01 | 分离 AccountAuthStore/公开目录，device/revoke 唯一 owner；建立独立 schema/migrator 与禁止在线链接规则 | Chat/IM SQL 不含认证列/表，无 schema 普通/build/dev 可达依赖；唯一历史迁移来源，checksum 不变 |
| R05c1 回执与清理目标 | R05a | 新增删除回执/PG StoreScope 元数据迁移与 Memory 生命周期状态；Royal 身份事务同写回执，定义 resume/complete 与实际 scope 合同 | purge 失败后身份已删仍可验证原 owner 并恢复；不靠 URL 比较；历史无回执案例明确隔离；旧迁移 checksum 不变 |
| R05c2 实际 store 清理接口 | R05c1 | 给 HTTP/直连 PG/Memory adapter 提供作用域校验与幂等清理，Royal 先发布内部接口并修复旧完整 delete 的回执恢复 | 三种 backend 都清实际消息资源；原 Memory 不可达不能换空 store 成功；旧客户端接口兼容；不在事务里等待远程调用 |
| R05c3 BotControl/删除协调器收口 | R05c2 | Royal 独占身份 writer/配额；Chat 改用 identity/resume → actual store purge → complete；移除 count_bots 占位与 HTTP 的旧重复完整删除 | 失败矩阵和重启/重试验收；重试只跳过「行已不存在 → 108」，每次仍用当前 actor 核对 original_owner；直连 Chat 保留必要 purge，普通目录实时调用不变 |
| R05b IM 数据抽库 | R05a/R05c3 | 移 MessageStore/公开 Directory/ID/spec/nonce 与 IM adapter；不移动 Royal 认证/bot 身份状态或 schema 嵌入 | Royal 无 chat 普通依赖；IM adapter 无认证 SQL/迁移依赖/bot 身份 writer；保留消息 purge 与 StoreScope 合同 |
| R06 Royal adapter 抽库 | R05b | HTTP/pool 提取；Chat 外置 Naming 刷新、指标 adapter 与 task 关闭 | adapter 普通/build 图无 naming/metrics/services；endpoint 更新、404/503/deadline/熔断及指标组合测试 |
| R07 网关运行库 | R01 | 按 §4.3 符号表提取 handler/revoke/hooks/run，frontend 仍在各入口 | 两入口都依赖 kim-gateway-runtime；tgateway 不依赖 services/gateway；runtime 普通/build 图无 kim-ws；WS/TCP/TLS、撤销/drain 验收 |
| R08a 根 npm workspace/lock | R01 | 只在原位置登记现有 SDK/Media 包，整合根 lock；固定 npm 版本，不拆包、不迁目录 | 根安装、本地 link/lock 校验；依赖版本差异单独审查，禁止顺便升级 |
| R08b Web 接口拆包 | R08a | 在原 sdk/web/app 建应用包，分配配置/依赖/测试，改走 SDK exports；不挪应用源码到 apps | SDK packed import、应用 build/E2E；lock 仅允许本地 package/link 与必要职责变化，无第三方版本更新 |
| R09 Flutter 全 lib 收口 | R04a/R04b | bootstrap、稳定 ports/errors、25 文件逐项归类、chats 展示投影、订阅关闭 | 全手写 lib 扫描；仅指定 bridge adapter 可引用生成 FFI，main/core/ports/design/models/features 全覆盖 |
| R10a Rust 分组 | R04b/R05b/R06/R07 | shared、server、client/agent 每组一个独立路径提交；修组内外 path、生成资源和入口 | 每组分别 metadata/生成/对应构建通过，再进入下一组；无业务行为变化 |
| R10b app 迁位 | R09；若 R10a 已做则使用新 crate 路径 | app→apps/kim，原生工程随包迁位；同步 hook/FRB/FVM/IDE/OTA/CI | clean Native Assets、FRB 再生成、手机/桌面构建、OTA dry-run；独立回退此提交 |
| R10c Media 迁位 | R08a | sdk/media→services/media；同步 wrangler 路径、workflow、Dependabot、workspace/link | Media 单测/配置打包检查；根 lock 只变路径，无依赖重解析升级 |
| R10d Web 应用迁位 | R08b | 原应用包→apps/web；同步 Vite/Worker/assets/env/workspace/CI | 源码 import 不变，包 build/边缘路由测试；lock 只变本地路径，与 R08a 分离 |
| R10e 示例/工具归位 | 对应 R08b/R10a | demo 和 kimbench 各自独立路径提交；修任务入口和 workspace members | 各自构建/运行入口验收；不与应用或全部 Rust crate 同一提交 |
| R11a Docker 输入验证 | R01；每次 Rust 路径变更后重验 | 验证 manifest 骨架/子集上下文与根 lock 兼容，完成 server 构建及运行 smoke | 解析图和编译图分开记录，context 不含完整桌面源码；无验证前不切换发布，不预设完整 Codex 回退 |
| R11b 发布与检查闭包 | R10 各子切片/R11a | 核对共享依赖触发、产物分组、单 OTA 定义与文档 | 各切片自身已经同步路径；此处只做闭包复核与独立发布 dry-run，不补救前面漏改的断链 |
| R12 协议职责与清理 | R11b | 隔离服务端部署检查，按收益评估 IDs，删过渡导出/例外 | 长期约定归 docs，删除实施稿；不混入框架/引擎升级 |

客户端依赖分叉如下；这是任务前置关系，不要求启动并行 agent：

```text
R00 → R01
       ├─ R02（只做 model/codec）→ R03 ─┐
       └─ R04a（既有行为恢复）───────────┴─ R04b → R09 → R10b
```

R02 只依赖 R01，与 R04a 可独立推进。其验收只选 model/codec 包，检查手机解析依赖图；不能因 SDK 的 secrets/FFI 入口仍空而要求先完成登录、密钥或 KimHost。若现有测试 fixture 依赖 Host，则迁移该 fixture 的纯数据输入，不为通过 R02 测试实现执行器。R04a 阻塞 FFI/运行集成汇合，不阻塞模型提取；R03 真实 SDK 测试须在汇合时实际执行。服务端先 R05a，再 R05c1 → R05c2 → R05c3 → R05b → R06；网关 R07、Web R08a/R08b 保持独立线。

R00 的文件职责白名单：manifest、原有 mod/pub use 声明、基线记录。secrets 的平台执行实现、KimHost 行为、FRB API 改造、第二库删除与 OTA 装配归 R04；runtime 的新调度/取消归 R03。不能为让所有测试立即通过而在 R00 创造这些实现。HEAD 也只是对照，必须验证，不能把它未经验证写成已知全绿基线，更不能覆盖用户未提交修改。

每个 R10 子切片必须自己包含相关 CI/生成/配置路径修正，不延迟到 R11。路径迁移会产生 lock 的本地路径元数据差异；验收要求第三方解析版本不变，而非要求 lock 字节完全不变。

### 10.0 每类路径迁移的独立检查清单

| 切片 | 必须同提交修正的引用 | 最小验收 |
|---|---|---|
| R10a Rust shared | workspace members/dependencies、core/protocol path、Web gen-proto 相对定位、Rust build.rs、include 资源 | metadata、Rust/Web 协议生成及对应测试 |
| R10a Rust server | 数据库 macro 的编译包根、server-schema/migrations、各服务 path、Docker 上下文生成器 | schema checksum、PG 初始化、server check/build |
| R10a Rust client/agent | FFI rust_root、hook cratePath/根探测、Codex 相对 path、profile 包名引用、Native Assets 缓存 | 现有各 FRB 配置核对；若 R04b 完成只保留主配置；目标 native build |
| R10b app | hook 相对层级、有效 FRB 配置、生成 assetName、FVM/IDE、Xcode scripts、OTA workflow/脚本/公钥/输出路径 | fresh pub get、FRB 无漂移、APK/macOS native load、OTA 打包校验 |
| R10c Media | wrangler main/tsconfig schema/env/test 路径、CI working-directory/cache、Dependabot、根 workspace/lock | Media test/typecheck、Worker build dry-run |
| R10d Web | Vite root/envDir/alias/outDir、Worker main/assets、协议脚本所属 SDK 路径、tests、CI/cache、workspace/lock | SDK packed import、SPA build、HTTP/WS 回源契约 |

R08a 的 lock 整合、R08b 的包接口拆分、R10d 的物理迁移是三个不同提交组。不要把它们以“仅路径变化”的名义合并。

### 10.1 不随本次重构改变的外部合同

- IM 命令/状态/帧格式、HTTP 路由、域名、R2 key 和部署 service ID。
- PostgreSQL 表/迁移内容、SQLite 消息与配置格式、Agent schema version。
- 服务端 persist/ACK/收件箱与客户端发送状态语义。
- 平台凭据库的实际存储键与用户授权工作区路径。
- Codex/Goose/Flutter/FRB/Rust 版本；框架更换或数据库重设计不混入这些切片。

如果实现发现必须改变以上合同，独立写兼容/迁移切片，并给出旧客户端与新服务端的滚动升级验证。

R05c1–c3 是明确的行为修复与装配变化：bot 身份写入归 Royal，消息清理由实际 store owner 执行；没有 Royal 时不能创建/配置/删除 bot。它们新增删除回执/StoreScope 元数据和内部阶段接口，分别提交，不混入 R05b 的纯抽库。0001–0016 不改；新增迁移编号实施时按最新序列分配。对外 IM 命令号/状态编号不变，部分失败继续返回错误，重试可恢复。

### 10.2 全部完成的判定

1. fresh checkout 能按 README 分别检查 server、mobile、desktop、web、media。
2. 所有 runtime 的资源拥有者和退出顺序可由公共接口说明，生命周期行为测试通过。
3. Royal/TGateway 不依赖其他服务包；客户端数据路径不依赖执行 Host。
4. 一个 Flutter 应用、一个应用 FFI 库；生成文件只通过批准的 adapter/public types 出口使用。
5. 目录与构建规则一致，协议变更触发相关消费者；各发布单元独立可验收。
6. 公共行为与现有协议兼容；移除临时 re-export、重复 workflow 和迁移期例外。
7. BotControl 与公开目录分离，bot 身份写入经 Royal，消息清理由实际 MessageStore 执行；pending 删除重试只跳过「行已不存在 → 108」，每次仍用当前 actor 核对 original_owner，不匹配返回 112 且不清理；失败状态不变成成功；scope 由实际 MessageStore 核对，Royal 只原样记录。本地 Memory 的生命周期范围明确。
8. Chat/IM adapter 不链接 schema/migrator，不含认证 SQL；历史 DDL 仅由独立 migrator 链接的 schema 嵌入，任何未解除的编译输入例外都显式列出。

## 11. 关键源码索引

链接指向当前文件；HEAD 专属证据通过 `git show HEAD:<path>` 复核，不能用空的工作区文件代替历史内容。

| 主题 | 入口 |
|---|---|
| workspace、sqlx、patch、平台 FFI profile | [Cargo.toml](../../Cargo.toml) |
| Royal 使用 Chat 数据能力 | [royal/lib.rs](../../services/royal/src/lib.rs)、[royal/main.rs](../../services/royal/src/main.rs) |
| bot 的双入口与配额接口 | [Chat bot](../../services/chat/src/bot.rs)、[Royal bot](../../services/royal/src/bot.rs)、[users.rs](../../services/chat/src/users.rs)、[HTTP adapter](../../services/chat/src/royal.rs) |
| Chat 两种 backend 装配 | [chat/main.rs](../../services/chat/src/main.rs) |
| 数据合同与 PG migration | [store/mod.rs](../../services/chat/src/store/mod.rs)、[postgres.rs](../../services/chat/src/store/postgres.rs) |
| RPC deadline/重试/连接池 | [royal.rs](../../services/chat/src/royal.rs)、[royal_pool.rs](../../services/chat/src/royal_pool.rs) |
| 网关运行复用 | [gateway/run.rs](../../services/gateway/src/run.rs)、[tgateway/main.rs](../../services/tgateway/src/main.rs) |
| SDK 消息/配置/epoch/持有 Agent | [sdk/lib.rs](../../crates/kim-sdk/src/lib.rs)、[sdk/agent](../../crates/kim-sdk/src/agent/mod.rs) |
| SDK store 写队列 | [store/mod.rs](../../crates/kim-sdk/src/store/mod.rs) |
| 数据与 Host 的编译耦合 | [codec/lib.rs](../../crates/kim-agent-codec/src/lib.rs)、[host/profile.rs](../../crates/kim-agent-host/src/profile.rs)、[Host manifest](../../crates/kim-agent-host/Cargo.toml) |
| 桌面执行与权限 | [drive.rs](../../crates/kim-desktop-runtime/src/drive.rs)、[permissions.rs](../../crates/kim-desktop-runtime/src/permissions.rs) |
| 第二 FFI 独立目录状态 | [agent catalog](../../crates/kim-agent-ffi/src/api/catalog.rs) |
| Flutter 启动/生成器/原生构建 | [main.dart](../../app/lib/main.dart)、[hook](../../app/hook/build.dart)、[FRB](../../app/flutter_rust_bridge.yaml) |
| Flutter ports 与跨 feature 投影 | [kim_ports.dart](../../app/lib/bridge/kim_ports.dart)、[threads_repository.dart](../../app/lib/features/chats/data/threads_repository.dart) |
| Web 跨源码引用 | [chat.ts](../../sdk/web/app/lib/chat.ts)、[ChatProvider](../../sdk/web/app/state/ChatProvider.tsx) |
| 协议生成 | [Rust build.rs](../../crates/kim-protocol/build.rs)、[Web gen-proto](../../sdk/web/scripts/gen-proto.mjs) |
| Media 鉴权与写入 | [index.ts](../../sdk/media/src/index.ts) |
| 发布依赖 | [Dockerfile](../../Dockerfile)、[Web CI](../../.github/workflows/web.yml)、[Media CI](../../.github/workflows/media.yml)、[OTA CI](../../.github/workflows/logic-ota.yml) |

工具语义参考：[Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)、[Cargo resolver 与 feature 统一](https://doc.rust-lang.org/cargo/reference/resolver.html)、[npm workspaces](https://docs.npmjs.com/cli/v11/using-npm/workspaces/)。这些资料支持工具行为说明，仓库耦合判断来自上面的源码与配置。
