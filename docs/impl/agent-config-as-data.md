# Agent 配置即数据：protobuf 权威字节 + 存储抽象 + 多设备下发

| 字段 | 值 |
|---|---|
| 状态 | Draft（调研 + 方案；PR 1 schema/`agent.proto` 已开工） |
| 日期 | 2026-09-15 |
| 对照代码 | 行号以标识符为准 |
| 父规格 | [multi-agent-vendor-catalog.md](./multi-agent-vendor-catalog.md)（C-KD 三层拆分）、[agent-provider-persona.md](./agent-provider-persona.md)（P-KD 信息架构）、[agent-capability-blocks.md](./agent-capability-blocks.md)（B-KD 装配单元）、[agent-productivity.md](./agent-productivity.md)（S-KD 四层对象）、[goose-bot-first-class.md](./goose-bot-first-class.md)（bot-KD 云身份） |
| 范围 | `crates/kim-protocol/proto/agent.proto`（权威正文）、`crates/kim-agent-host`（装配）、`crates/kim-sdk`（profile 存储）、`sdk/mobile`（消费面）、`services/chat`（owner-only Spec 副本，Phase 3）。不动 gateway / royal 热路径、ACK 协议 |
| 编号 | 本文决策写作 **A-KD *n***。修订父决策时写 **revises C-KD / P-KD / B-KD / S-KD / bot-KD *n***。本修订新增 A-KD 9–13，revises A-KD 1 / 5 / 6 |

---

## Overview

**一句话**：Agent 启动时需要的那包东西——人设、模型、推理、能力、权限、工作区、Skill、密钥引用——今天已经是可序列化数据（`AgentProfile` JSON），但**存在三处、装配走两条轨、执行入口还带硬编码默认值**。本方案把它收敛成**单一权威数据结构（`AgentSpec` protobuf 字节）+ 存储抽象（`ProfileStore` trait）+ 一次装配入口（`AgentHost::from_spec`）**。本地 SQLite `body_blob` 与云端 BYTEA 是**同一串 bytes**；chat 不解析正文。换一台 macOS 登录同一账号即可拉到已配置 Agent；客户端升级加字段只发客户端，不部署 chat。

不新建领域模型。`AgentSpec` = 现有 `AgentProfile` 的正文 + `ProviderAccount` 引用。名字换成 Spec 是为了强调"它是数据，不是执行器配置"。JSON 降为一个 release 的读路径，不再是权威。

---

## 1. 现状盘点（对照代码，不是愿望）

### 1.1 配置的数据形态已经存在

| 层 | 事实 | 证据 |
|---|---|---|
| Rust 权威结构 | `AgentProfile`：id / display_name / aliases / provider / model / reasoning / system_prompt / mode / max_turns / capabilities / permission_rules / permissions / sandbox / extensions / enabled / steer / workspace / skills / portable_denylist / user_agents_skills。serde 全兼容（缺字段回默认），`Serialize + Deserialize + PartialEq` | `crates/kim-agent-host/src/profile.rs` `pub struct AgentProfile` |
| 本地持久化 | SQLite `agent_profiles(account, profile_id, nickname, server_account, body_json, key_ciphertext, updated_at)`；正文整包 `body_json`；**`key_ciphertext` 列预留、永远写 NULL** | `crates/kim-sdk/src/store/schema.rs:138`；`store/agent.rs` `upsert_profile` |
| SDK 读写面 | `KimSdk::{upsert,delete,list,import}_agent_profile`，行类型 `AgentProfileRow { profile_id, nickname, server_account, body_json }` | `crates/kim-sdk/src/lib.rs:461-486`；`agent/profiles.rs:5` |
| Dart 消费面 | `AgentProfileStore._reload`：SDK rows 非空即权威；prefs `agent.profiles` 只作一次性导入源，导入后 `prefs.remove` | `sdk/mobile/lib/features/agent/agent_profiles.dart` `_reload` / `_persist` |
| 服务端投影 | `chat.bot.create/update` 只存 owner-only `BotConfig { model, thinking_effort, context_tokens, visibility }`，**永不存 key / prompt / capabilities**（bot-KD 12） | `services/chat/src/users.rs:63`；`agent_profiles.dart` `_ensureBotIdentity` / `_syncBotConfig` |

### 1.2 存储分裂：同一份配置散在三处

| 数据 | 今天存在哪 | 应该在哪 |
|---|---|---|
| Profile 正文 | kim-sdk SQLite `body_json`（✅ 已对） | 不动 |
| ProviderAccount（vendor/baseUrl/keyRef/models） | **SharedPreferences** `agent.provider_accounts`（`provider_accounts.dart:4`） | SQLite 表 |
| API key | macOS Keychain `agent.api_key.acct.*` / `agent.api_key.goose`（✅ 密钥就该在 keychain，但**引用关系**没进 SQLite） | key_ref 列进表，值留 Keychain |
| 全局开关 | SharedPreferences `agent.multi_profile` / `agent.server_identity` / `agent.active_profile_id` | `device_settings`（SDK 已有 settings 表） |
| 旧 `AgentSettings` | SharedPreferences `agent.llm_backend` 等 7 键 + keychain 双写（`agent_settings.dart:8-16`） | 删除（迁移后） |

后果（已核对）：`AgentProfileStore._migrateAccounts` / `_migrateAccountModels` / `saveGoose` 三段迁移代码 ~150 行，全部在补"两处存储不一致"的账；每加一个 profile 字段要改 Dart `toJson/fromJson` + prefs 迁移 + SDK row 透传三层。

### 1.3 装配耦合：配置进执行器走两条轨

```text
轨 1（扁平字段）:  SessionOpenOpts { model, llm_backend, base_url, api_key,
                    enable_fs_tools, bash_enabled, thinking_effort, goose_mode, ... }
                    └─ profile_json 为空 → AgentProfile::from_legacy(LegacyOpenOpts)
                       (sdk/mobile/rust_agent/src/api/session.rs  resolved_from_opts)

轨 2（整包 JSON）:  SessionOpenOpts.profile_json → serde 反序列化 AgentProfile
                    + opts.api_key 平行传入 → ResolvedProfile { profile, api_key, project_root }
                    → AgentHost::from_resolved → provider + model + McpHub
```

两条轨在 `resolved_from_opts` 汇合时还互相污染：`fill_provider_from_legacy` 会用扁平字段回填 JSON 缺口。`SessionOpenOpts` 14 个字段里 11 个是轨 1 遗产。

### 1.4 执行入口没有走统一装配（现状缺陷，也是本方案的直接动机）

桌面 Agent 聊天链路：`MobileAgent.enqueue_turn` → FFI `AgentRunRequest{dest, profile_id}` → Dart `AgentRunLoop._promptGoose` → `goose.open`。但 `_promptGoose`（`sdk/mobile/lib/bridge/agent_bridge.dart:50`）传的是：

```dart
SessionOpenOpts(
  model: 'gpt-4o', llmBackend: 'openai', baseUrl: '', apiKey: '',
  profileId: req.profileId, profileJson: '', ... // ← 硬编码默认值
)
```

**没有** `readApiKey`（全仓库零调用者）、没有从 store 取 profile JSON。也就是说：配置数据躺在 SQLite 里，执行入口却在重新拼一份假配置。这正是"配置与执行没有一条解耦通道"的实证——修这个 wiring 的正确方式不是打补丁，而是建立 `resolve(spec_id) → AgentHost` 单一入口。

### 1.5 云端钩子已经在位

| 钩子 | 现状 |
|---|---|
| `agent_profiles.key_ciphertext BLOB` | 预留列，写 NULL。天然是"加密密钥随配置同步"的落点 |
| `server_account` + `bot_create/bot_update` | profile ↔ 云 bot 身份绑定与投影同步已通 |
| `chat.bot.pending` / `bot_reply` | Agent 离线/远程回复通道已通（owner 代发语义） |
| S-KD 11 | 已拍板：KIM 托管的一等 Skill 包（id/version/sha256/正文）可以像 `vendors.json` 一样云端分发 |
| C-KD VendorCatalog | `catalog/vendors.json` 内置 + 按钮拉模型——目录本身就是"数据分发"形态 |

结论：**缺的不是数据结构，是"存取抽象 + 单一装配入口 + 同步语义"三件事。**

---

## 2. 对标调研（外部项目怎么把 Agent 配置当数据）

| 项目 | 配置形态 | 存哪 | 执行怎么拿配置 | 抄什么 | 拒什么 |
|---|---|---|---|---|---|
| **OpenAI Assistants API** | Assistant 对象（id + instructions + tools + model），服务端资源 | 云，REST CRUD | `run(assistant_id, thread_id)`——执行只传引用 | **配置是服务端资源，执行传 id**；增量 patch | 全托管依赖（我们要本地优先） |
| **Claude Agent SDK** | settings JSON + `--agents` 子代理定义文件 | 本地文件 | 启动时读文件装配 | 文件即配置、可版本管理 | 无同步故事 |
| **Goose（上游）** | `config.yaml` + Recipe YAML | `~/.config/goose` | 启动读 yaml | serde 兼容字段缺省的思想（已抄） | Recipe 当人设（S-KD 12 已拒） |
| **cc-switch** | preset JSON 列表 | 本地文件 + keychain | 切换时写 CLI 配置 | preset 与 secret 分离 | 单一"当前 provider"模型（我们要多 Agent） |
| **Cherry Studio / LobeChat** | 助手 ≠ 账号，两层 JSON | 本地 IndexedDB / 云同步可选 | 打开会话时装配 | **账号层与助手层分离 + 云同步开关**（C-KD 已部分采纳） | 100+ provider 大列表 |
| **K8s Operator 模式** | Spec（期望态）/ Status（观测态）分离 | etcd | controller watch spec 变化 reconcile | **Spec/Status 分离**；`resourceVersion` 乐观并发 | 引入完整 controller 复杂度 |

**综合结论**：
1. 配置 = 服务端可寻址资源（Assistant API）或本地文件（Claude/Goose）都有成功先例；**分水岭不在结构，在"执行是否只传引用"**。
2. 所有健康生态都把 **secret 与 spec 分离**，secret 用引用（key_ref / keychain / vault）。
3. 多设备场景（LobeChat）证明"本地 SQLite 权威 + 可选云同步"可行，但必须先有**单表单写入口**。

---

## 3. 目标设计

### 3.1 数据结构：`AgentSpec`（A-KD 1，revises：序列化形态是 protobuf）

**A-KD 1 — `AgentSpec` 是 Agent 的唯一权威配置数据；它不含 secret，不含执行态。权威序列化是 `kim.agent.AgentSpec` protobuf 字节，不是 JSON。**

```text
AgentSpec                          # 序列化 = kim.agent.AgentSpec proto bytes
├── meta
│   ├── schema_version: uint32     # ★ 当前 1；过高 → 拒绝应用并提示升级
│   ├── id: string                 # 现 profile_id，如 "goose" / "p-1726..."
│   ├── display_name, aliases
│   ├── enabled: bool              # proto optional；unset → true
│   ├── placement: local | cloud   # ★ 新增：在哪执行（Phase 4 用，缺省 local）
│   └── updated_at: i64            # ★ 新增：同步用单调版本（LWW + tombstone 判据）
├── identity
│   ├── system_prompt
│   └── server_account             # 云 bot 身份（不变）
├── model_ref                      # ★ 从内嵌 provider/model 收窄为引用
│   ├── account_id: string         # → provider_accounts 表
│   ├── model: string
│   └── reasoning: ReasoningChoice # C-KD 形状不变；advanced 走 bytes 口袋
├── assembly                        # B-KD / S-KD 形状不变
│   ├── capabilities: [CapabilityRef]  # params 走 bytes 口袋
│   ├── permission_rules
│   ├── workspace.kind             # path 不在 Spec 里（A-KD 12）
│   ├── skills: [SkillRef], portable_denylist
│   └── steer, max_turns, mode
└── (不存在 api_key / key 值 / workspace.path / user_agents_skills)
```

要点：

- 运行时结构仍是 `AgentProfile`。`from_json` 只服务一个 release 的旧 `body_json` 导入，导入后 encode proto 回写 `body_blob`。
- `ProviderAccount` 升格为 SDK SQLite 表 `provider_accounts(...)`，与 `agent_profiles` 同库。Keychain 只存值，引用关系进库；账号行随 Spec 同步（A-KD 13）。
- Spec/Status 分离：`AgentTurnState`、会话转录、LRU 不在 Spec 里，也不随 Spec 同步。
- 本机路径进 `agent_device_overlay`，不同步（A-KD 12）。

### 3.2 存储抽象：`ProfileStore` trait（A-KD 2 / A-KD 3）

**A-KD 2 — 引入 `ProfileStore` trait，kim-sdk 提供本地 SQLite 实现；未来远端实现实现同一 trait。消费面（Dart / 宿主）只面向 trait。**

```rust
// crates/kim-sdk（示意，签名在实施 PR 定稿）
#[async_trait]
pub trait ProfileStore: Send + Sync {
    async fn list(&self) -> Result<Vec<AgentSpecRow>, SdkError>;
    async fn get(&self, id: &str) -> Result<Option<AgentSpecRow>, SdkError>;
    async fn upsert(&self, row: AgentSpecRow) -> Result<(), SdkError>;      // 带 updated_at 乐观检查
    async fn delete(&self, id: &str) -> Result<(), SdkError>;               // 写 tombstone 行
    async fn subscribe(&self) -> watch::Receiver<ProfilesSnapshot>;          // ★ 复用刚落地的 watch 模式
}
```

**A-KD 3 — 本地实现 = 现有 `agent_profiles` 表扩列；Dart 侧列表唯一来源改成 watch，删除 SharedPreferences 三键。**

刚合入的 `mobile-data-flow-convergence` 已经把"提交后查询发布 + watch 单一来源"的模具铺好（ChangeLog → QueryPublisher → `watch::Sender`）。Profile 存储直接复用同一套：`WriteOp::UpsertAgentProfile` 提交成功记 dirty，publisher 推 `ProfilesSnapshot`。Dart `AgentProfileStore` 从"pull + prefs 迁移 + 三段补账"退化为"watch + 命令回执"，与 contacts/chat 收敛完全同构。

### 3.3 装配解耦：单一入口 `resolve` → `AgentHost`（A-KD 4 / A-KD 5）

**A-KD 4 — `AgentHost::from_spec(spec: &AgentSpec, secrets: &dyn KeyVault, workspace: &dyn WorkspaceRoots)` 是唯一装配入口；`from_resolved` / `SessionOpenOpts` 扁平字段退役。**

```text
                    ┌─────────────┐   id    ┌──────────────┐
  UI / 消息驱动 ──► │ ProfileStore│ ──────► │  AgentSpec   │  (数据，可本地可远端)
                    └─────────────┘         └──────┬───────┘
                                                  │ resolve()
                    ┌─────────────┐               ▼
                    │  KeyVault   │ ─────► ResolvedAgent { provider, model,
                    │ (Keychain / │          capabilities, mcp, project_root }
                    │  云端 vault) │               │
                    └─────────────┘               ▼
                    ┌─────────────┐         AgentHost::from_spec
                    │ Workspace   │ ─────►  (进程内 Goose 机器，不变)
                    └─────────────┘
```

- `KeyVault` trait 先只有 Keychain 实现（Dart 侧保留 secure storage，FFI 传 key_ref + 值由宿主解析）；云端 vault（Phase 4）同 trait。
- **修复 1.4 的 wiring 缺陷**：`AgentRunLoop._promptGoose` 改为 `store.get(req.profileId)` → decode proto → `from_spec`。配置只从 store 来，`SessionOpenOpts` 收窄为 `{ spec_blob, session_id, resume_on_open }`（api_key 经 key_ref + vault，不再裸穿 FFI 扁平字段）。
- `reconfigure` 语义升级为：watch 到 Spec 变化 → diff → 仅重建受影响的段（provider/model 变则重建 host；capabilities 变则重建 tools；prompt/steer 热替换）。这也是上云后"远端改配置、本地执行器跟随"的机制。

**A-KD 5（revises）— JSON serde 兼容不是永久承诺：旧 `body_json` 只读一个 release，新权威是 proto field number。** `tools` / `extensions` 投影在 JSON 导入时消化，不进入 `agent.proto`。Dart `toJson` / `fromJson` 降为迁移/debug。

### 3.4 序列化分层（A-KD 9 / A-KD 10 / A-KD 11）

**A-KD 9 — 序列化分三层。** 后台用 protobuf，不等于把聊天正文那种开放 `string body` 套到 Agent 配置上，也不等于 Dart 引入 `package:protobuf`。

```text
线信封  AgentSpecRecord / SyncReq   pkt.proto     P3 才加命令
正文    kim.agent.AgentSpec         agent.proto   P1 落地；本地 blob = 云 BYTEA
运行时  AgentProfile + Dart FRB     不序列化      host 不依赖 kim-protocol
```

- 拒绝「本地继续 JSON、上云再转 pb」：转两次、漂移两次。
- 拒绝 RPC 旁路（JSON-RPC / REST）。P3 走 `chat.agent.spec.sync` / `upsert`，与其它 `chat.*` 同构。
- Dart 不引入 protobuf 包。编解码只在 Rust；FRB 暴露普通 struct。

**A-KD 10 — 开放 JSON 口袋保留为 `bytes`，不引入 `google.protobuf.Struct`。** `CapabilityRef.params`、`ModelSpec.extra_params`、`ReasoningChoice.advanced` 是厂商/block 专属。`CapabilityBlock::check` 已经吃 `serde_json::Value`。封闭字段全部 typed。

**A-KD 11 — Chat 不解析 AgentSpec。** 云端新表 `agent_specs(app, owner, profile_id, nickname, server_account, spec BYTEA, key_ciphertext, updated_at, deleted_at)`。唯一校验：非空、能 decode 出 `id` 与 `schema_version`。Spec 加字段只发客户端。

修订原 A-KD 6「复用 `bot_config` 投影表」：**取消。** `bot_config` 仍是社交投影（model / thinkingEffort / contextTokens / visibility），bot-KD 12 锁死四字段。`_syncBotConfig` 继续只推这四字段。Spec 同步失败不得阻塞 bot 身份。

**A-KD 12 — 本机绑定字段进 `DeviceOverlay`，不进同步 Spec。**

| 同步 | 不同步 |
|---|---|
| 人设、capabilities、permission_rules、SkillRef、steer、mode、placement、enabled | `workspace.path`、security-scoped bookmark |
| `model_ref`、`ProviderAccount`（无 key 值） | `user_agents_skills` 绝对路径 |
| `server_identity` / `multi_profile` | `active_profile_id`、Keychain 明文 |

Mac B 拉到 `workspace.kind = repo` 且 overlay 空：UI 提示重新授权，不静默用 Mac A 的 `/Users/alice/...`。

**A-KD 13 — ProviderAccount 与 AgentSpec 同一同步通道、同 LWW。** `model_ref.account_id` 悬空则 Mac B 打不开模型。`key_ref` 同步；key 值默认不上云，opt-in 后走 `key_ciphertext`（A-KD 8）。

Sync 路径 **按字节原样存放**（LWW 整包覆盖，A-KD 7）。不在 sync apply 时 decode-encode。本地编辑器保存时从当前 typed struct encode 一整份新 blob——旧客户端保存即整包覆盖，这是 LWW，不是字段级合并。prost 默认丢 unknown fields，因此禁止「为了保留未知字段而在旧端做读改写」；要保留新字段，就不要用旧端去保存那一行。

### 3.5 上云路径（A-KD 6 / A-KD 7 / A-KD 8）

**A-KD 6（revises）— 云同步 = `ProfileStore` 的远端实现，走 `ProtocolClient`；新建 owner-scoped `agent_specs` 表，不复用 `bot_config`，不新建服务进程。Postgres 只由 Royal 写；Chat 经 HMAC `/api/v1/agent/spec/*` 读写，与消息 / `bot_config` 同一扇门。**

阶段划分：

| 阶段 | 内容 | 动到哪 |
|---|---|---|
| **P0（已达成）** | 正文 `body_json` 本地 SQLite 权威；云身份投影 `BotConfig` 同步 | ✅ #118 / #122 / #124 / #125 |
| **P1 本地收敛** | `agent.proto` + `body_blob`；provider_accounts / overlay / 全局开关入 SQLite；Dart watch 单一来源；JSON 只读迁移 | `kim-protocol` + `kim-sdk` + Dart store，不动服务端 |
| **P2 装配解耦** | `from_spec` 唯一入口；`SessionOpenOpts` 收窄；修 `AgentRunLoop` wiring；`reconfigure` diff 重建 | `kim-agent-host` + `rust_agent` FFI |
| **P3 云同步** | Postgres `agent_specs` BYTEA + `chat.agent.spec.sync/upsert`（protobuf 信封，与本地 `body_blob` 同一串）；LWW + tombstone；密钥**默认不上云**，用户显式开启才走 `key_ciphertext`（端侧加密）。P3 **不做**向其它 session 的主动 Push，避免插 gateway 热路径 | Chat RPC 面 + Royal HMAC（`/api/v1/agent/spec/*`）。表由 Royal 写，Chat 生产不直连 `DATABASE_URL` |
| **P4 云执行** | `placement: cloud` 的 Spec 由服务端 runner（复用 `kim-agent-host`，同一 crate 同一 `from_spec`）执行，经既有 `bot_reply` 通道回消息；本地只做 UI | 新 `services/agentd`（或 chat 内 worker），`kim-agent-host` 保持可脱离 tokio-net 独立装配 |

**A-KD 7 — 同步语义：per-spec `updated_at` LWW + 软删除 tombstone 行（保留 30 天），冲突不做字段级合并。** 依据：Spec 是低频编辑、单 owner；LoweChat/Assistant API 均无字段级合并。首启多设备 adopt：远端为空 → 推本地全量；本地为空 → 拉远端全量。

**A-KD 8 — 密钥红线不因上云松动。** 服务端默认只见 `key_ciphertext`（用户开启同步且端侧加密后才存在）；`chat` 侧永不持有明文 key（bot-KD 12 延伸）；`key_ciphertext` 的 KDF/加密包格式在 P3 PR 里单独评审，本文不锁定。

**为什么这个形状"上云比较方便"**——三条解耦线各自独立演进：

```text
数据线:  AgentSpec proto bytes ── 本地 body_blob ──► chat.agent_specs BYTEA ──► 多设备
秘密线:  key_ref ── Keychain ──────────────────────► 云vault(可选,E2E)
执行线:  AgentHost::from_spec ── 本地进程 ─────────► 服务端runner(同一crate)
本机线:  DeviceOverlay (path / bookmark) ──────────► 不同步
                ▲ 装配只依赖 AgentSpec + KeyVault + Overlay
```

任何一条线换实现，另两条不动。这就是"配置即数据"的全部含义。

---

## 4. Phases 与 PR 切分

一份一切片；每 PR 独立可合、测试齐。

| PR | 内容 | 破坏面 | 测试门槛 |
|---|---|---|---|
| **PR 1** schema + proto | `agent.proto`；`agent_profiles` 加 `body_blob` / `placement`；新表 `provider_accounts` / `agent_device_overlay`；schema v5。**protobuf 在本 PR 落地成本地权威列，P3 只搬同一串 bytes** | 无（加列加表 + 新 proto 文件） | v4 DB 升 v5 列/表存在；`AgentSpec` encode/decode roundtrip |
| **PR 2** Dart 收敛 | `AgentProfileStore` 改 watch 单一来源；删 `_migrateAccounts` / prefs 三键；`AgentSettings` 只读迁移 | Dart 内部 | contacts/chat 测试模式复刻：`fake_kim` 补 profile watch |
| **PR 3** Spec 定形 | JSON→proto 回填；DTO `body_json` → `body_blob`；Dart 停写 `toJson` 权威；`from_json` 仅迁移 | FFI DTO | 旧 JSON fixture → proto → 再 decode 等价 |
| **PR 4** 装配解耦 | `from_spec` + `KeyVault` + Overlay；`SessionOpenOpts` 收窄；**修 `AgentRunLoop` 硬编码 wiring** | `rust_agent` FFI 签名（内部 app） | 桌面 agent 聊天：改模型 → reconfigure 生效 |
| **PR 5** reconfigure diff | watch Spec 变化 → 分级重建 | 无 | 单测：三类变更各只触发对应重建 |
| **PR 6**（P3，后台轨） | `chat.agent.spec.*` 信封进 `pkt.proto`；Royal `agent_specs` BYTEA + HMAC API；LWW + tombstone；可选 E2E key。旧客户端不发则服务端零行为 | 新 RPC，缺省兼容 | 双设备：A 写 prompt → B sync 看到 |
| **PR 7**（P4，独立排期） | `placement: cloud` + 服务端 runner 复用 `kim-agent-host`；`bot_reply` 回投 | 服务端 | scripted provider 跑通云回合 |

P1–P2（PR 1–5）是纯客户端轨，遵守 next-stage.md 分轨原则：不插后台 PR 队列。P3/P4 走后台轨评审。

---

## 5. Non-Goals

- 不重做 Capability / Skill / Workspace 模型（B-KD / S-KD 已定，Spec 只是容器）。
- 不做字段级同步合并、不做协同编辑（LWW 够用）。
- P3 之前不动 `services/chat` 任何表与 RPC（`agent.proto` 可以先合，命令仍未进 `pkt.proto`）。
- 不做多租户 Agent 市场分发（S-KD 11 的一等 Skill 包分发是另一条线）。
- 不把 Goose 会话转录纳入 Spec 或同步范围。
- 不给 Dart 加 `package:protobuf`。
- 不把 `workspace.path` 当权威同步字段。
- `kim-agent-host` 不依赖 `kim-protocol`。codec 放薄层（host feature / `sdk/mobile/rust` / `kim-agent-codec`），sdk 继续只存不透明 bytes。

## 6. 风险与开放问题

| 风险 | 缓解 |
|---|---|
| Dart `AgentProfile` 手写 `toJson/fromJson` 与 Rust serde 双份漂移（已发生过 `ReasoningChoice.toJson` 的 `value`/`budget` 双写 bug） | PR 3 起 Dart 不序列化；Rust encode proto；UI 走 FRB 镜像 |
| `updated_at` LWW 依赖时钟 | 单调来源用 SDK 本地 `now()`（写侧）+ 服务端二次盖戳（P3） |
| E2E 密钥同步丢失 passphrase = 配置永久不可用 | 云端正文永远可读（不含密钥）；密钥丢失仅降级为重新输入 key。默认不上云（A-KD 8） |
| 旧客户端保存会丢掉它不认识的新 field | 这是整包 LWW 的本意；sync apply 按字节存放。禁止旧端为「保留未知字段」做读改写 |
| `placement: cloud` 的权限/审批模型 | P4 前置；先只允许 `mode: chat` |
| FFI 收窄 `SessionOpenOpts` 撞上在途 PR | 通知在途分支；保留 legacy 构造器一个 release |

## 7. 验收口径（P1+P2 完成时）

1. 全仓库 grep：`agent.profiles` / `agent.provider_accounts` / `agent.multi_profile` / `agent.server_identity` 四个 prefs 键零引用。
2. `AgentRunLoop._promptGoose` 不再出现字面量 `'gpt-4o'` / `'openai'`；配置一律 `store.get(profile_id)`。
3. 改任一 profile 字段 → `watchAgentProfiles` 收到快照 → 若会话在线，`reconfigure` 只重建受影响段。
4. 新写路径只写 `body_blob`；`body_json` 仅迁移读。`agent.proto` roundtrip 绿。
5. `cargo test -p kim-protocol -p kim-sdk -p kim-agent-host`、`flutter test` 全绿；v4→v5 列/表存在。
