# Agent 生产力：工作区、能力、Skill、广场

| Field | Value |
|---|---|
| Author | KIM Agent Working Group |
| Date | 2026-09-13 |
| Status | Draft |
| Audience | 实现桌面 Goose 生产力层的资深工程师（`sdk/mobile` + `kim-agent-host`） |
| 仓库 | `/Users/zhangfan/develop/github.com/im` |
| 扩展（不替换） | `docs/impl/goose-personalized-agents.md`、`docs/impl/agent-provider-persona.md`（P-KD）、`docs/impl/multi-agent-vendor-catalog.md`（C-KD）、`docs/impl/goose-bot-first-class.md`（bot-KD） |
| Goose pin | `goose-agent` / `goose-provider-types` / `goose-providers` **0.1.0-alpha.9** |
| 编号 | 本文决策写作 **S-KD *n***。不重开 P-KD / C-KD / bot-KD。 |

---

## Overview

桌面 Goose 已经能按 `AgentProfile` 跑机器、把 bot 注册成好友、1:1 消息上云。**生产力还没拆开。** 今天所有人设共用一个 `KimPaths.agentWorkspace`；`fs` / `bash` 是人设上的布尔开关，没有「这个 Agent 的仓库地址」；`ensureAgentDirs` 已经建了 `.agents/skills` 和 `AGENTS.md`，host **从不读**；P-KD 1 明确本轮不做 Skills。

本设计把生产力拆成四层，禁止再焊死：

| 层 | 一句话 | 权威存在哪 |
|---|---|---|
| **Capability** | 能调用哪些工具（读/写/shell/IM/MCP） | 已有 `ToolSet` + `PermissionConfig` + `extensions` |
| **Workspace** | 工具的 cwd / 沙箱根；非 coding 的长期记忆也落这里 | **新** `WorkspaceSpec`，每人设一份 |
| **Skill** | 程序性知识。先分 **生态 portable** 与 **App / 内置** | portable：`~/.agents` + 项目 `.agents`；app：KIM 库 / 解析器 / 可上云 |
| **Plugin / 广场** | 浏览生态目录 + 管理 KIM 内置 | 生态只扫不搬；KIM 架分配与更新 |

Skill **不是**新的 ToolProvider，也不是人设模板（P-KD 1 仍成立）。**不要**把依赖 `send_message` / 沙箱 `MEMORY.md` 的正文写进 `~/.agents`——Claude / Cursor / Goose 会加载并幻觉这些工具。可移植 Skill 留在生态目录；真正和本 harness 深绑的走动态注入（catalog → `activate_skill` → 按版本取全文），以便更新、复用、以后上云共享。

---

## Background & Motivation

### 已落地、本文不重做

| 能力 | 现状 | 证据 |
|---|---|---|
| `ToolSet` | `fs` / `fs_write` / `bash` / IM 工具 / `subagent` | `profile.rs`；Dart `AgentToolSet` |
| 进程内工具 | `FsToolProvider` / `BashToolProvider` 以 `project_root` 为根，防 `..` 逃逸 | `ops/fs.rs`、`ops/bash.rs` |
| `session_open` | 已收 `projectRoot`，写入 `ResolvedProfile.project_root` | `session.rs:231-272` |
| ChatAgent | **写死** `projectRoot: paths.agentWorkspace.path` | `chat_agent.dart:304` |
| 默认目录 | `support/agent/workspace` + `.agents/skills` + `AGENTS.md` 模板 | `paths.dart:74-93` |
| MCP | 人设 `extensions`；详情 Advanced 文本框 | `agent_settings_page.dart` |
| 权限卡 | `PermissionOp` + `action_required` 气泡 | 已有 |
| 人设 IA | Provider ≠ Agent；创建短表单；工具在 Advanced | P-KD 1–3 |
| 云身份 | IM 上云，Goose 仅桌面；后台不存 key / prompt / 工具 | bot-KD 12 |

### 痛点（已核对代码）

1. **所有 Agent 抢同一工作区。** 译者、助手、coder 的 `read_file` 都打进 `support/agent/workspace`。长期记忆、草稿、仓库文件会混在一起。
2. **coding 不能指定仓库。** `project_root` 槽位在，产品没有 folder picker，也没有 per-profile 持久化路径。
3. **`fs_write` 没有 UI。** Advanced 只有「只读 fs」和 bash。`copyWith(fs: _fs, bash: _bash)` 从不置 `fsWrite`。
4. **Skill 目录是空壳。** 路径在，无扫描、无注入、无分配。
5. **`AGENTS.md` 只写不读。** 模型看不见工作区约定。
6. **Release macOS 开了 App Sandbox**，没有 `user-selected` 文件权利。即使用户能选文件夹，Release 也进不去用户仓库。

### 主流 harness 对齐（只抄对象，不抄运行时）

| 项目 | Skill | Plugin / 广场 | 工作区 |
|---|---|---|---|
| **Agent Skills 规范** | 目录 + `SKILL.md` YAML；渐进披露（name/desc → body → references） | — | — |
| **Claude Code** | `~/.claude/skills`、`.claude/skills`；`activate` / 读文件 | Plugin = skills + hooks + MCP | 当前 repo |
| **Cursor** | 原生 `SKILL.md` | `.cursor-plugin/plugin.json` + marketplace | 当前 repo |
| **Goose** | 读 `.agents/skills` / `.claude/skills` | Open Plugins：`plugin.json` + `skills/` + hooks | Recipe 可绑 extensions |
| **Codex** | `SKILL.md` | `.codex-plugin/plugin.json` + `marketplace.json` | 当前 repo |

结论：Skill 格式跟 **Agent Skills**（`SKILL.md`）；Plugin 是**包装/安装**；广场是 **marketplace.json 目录**。Goose Recipe（整份人设 YAML）**不要**——KIM 已有 `AgentProfile`。unpublished Goose Skill/Doctor 管道 **不要 git 依赖**（与个性化 Agent 设计一致）。

---

## Goals & Non-Goals

### Goals

- 每个 Agent 一份 `WorkspaceSpec`：沙箱或用户仓库；`fs`/`bash` 的根 = 该路径。
- 非 coding Agent 默认沙箱：`MEMORY.md` + `notes/` + 可选 `AGENTS.md`。
- Coding Agent 能在详情里选本地仓库当 cwd（桌面 Release 必须能跨过 App Sandbox）。
- **适配生态 Skill：** 读本机 `~/.agents/skills` 与项目 `<cwd>/.agents/skills`（标准 `SKILL.md`）。其它 harness 也能读同一份文件；KIM 不把它们拷进 Application Support。
- **App / 内置 Skill：** 依赖 KIM 能力面（IM 工具、确认卡、沙箱记忆等）。只活在 app 库 / 版本缓存，**禁止**写入 `~/.agents`。
- 内置 Skill **只动态注入**：开场 catalog（id / name / description / version）；模型 `activate_skill` 时再按解析器取**当前版本全文**。人设只存引用，不存正文——才能更新、多人设复用、上云共享同一 `id@version`。
- 广场分两架：生态（只浏览/发现）与 KIM（分配/更新）。
- 详情 Advanced 露出 **读 / 写 / 终端**、工作区、已分配的 App Skill。
- Agent 执行中复用已有「正在输入」行：本机立刻亮，已注册 1:1 经 `chat.bot.typing` 同步到手机 / 其它桌面。
- 用户仓库路径 / bookmark / `MEMORY.md` / key **不进** `chat.bot.create`。一等 Skill **包**可以像厂商目录一样由 KIM 托管（不是用户秘密）。
- rust-strict；永不 log 仓库绝对路径里的用户目录细节到 info 以上（debug 可打 profile_id + kind）。

### Non-Goals

- 不恢复产品模板 / 向导（P-KD 1）。能力预设只是 Advanced 的快捷勾选，不创建人设。
- 不合并两条 FFI；不服务端代跑 Goose；不后台存 Skill 正文。
- 不抄 unpublished Goose Recipe / Doctor / 全管道。
- v1 **不做** hooks（`PostToolUse` 等）。MCP 仍走现有 `extensions`。
- v1 **不拉任意第三方远程广场**（与 C-KD 2 同一纪律）。内置 Skill 的云更新是 **KIM 一等 catalog**，另开 PR，不跟用户 CDN 混。
- 不做 OS seatbelt / Docker（早期沙箱仍是 path prefix + 权限卡）。
- 不做群 @、手机 Goose runtime。
- 不把本仓库开发用 `.agents/skills`（rust-skills 等）打进 app，也不把它们标成 KIM 内置。
- 不在 app 沙箱里再造一份假的 `.agents` 冒充生态目录。

---

## Key Decisions

1. **S-KD 1 — 四层对象，禁止再焊死。** Capability / Workspace / Skill / Plugin。人设 JSON 只存 `workspace` + `skills[]`（id 引用），不内嵌 Skill 正文、不把仓库文件拷进人设。
2. **S-KD 2 — 磁盘上的 portable Skill 格式 = Agent Skills 开源规范。** `SKILL.md` + `name` / `description`。KIM 扩展只放 `metadata.kim`。App / 内置 **作者格式仍是 SKILL.md**，但落点不是 `~/.agents`。
3. **S-KD 3 — 两类 Skill，两套发现。** **portable**：本机 `~/.agents/skills` + 项目 `<cwd>/.agents/skills`，其它 harness 也能读。**仅当** `workspace.kind=repo` 或 `tools.fs` 时自动进 catalog（coding / 能读盘的人设）；详情 `portable_denylist` 可关掉某一 id。沙箱 IM-only **不**扫 `~/.agents`，避免译者 catalog 里出现 `git-commit`。**app / 内置**：显式分配，不进生态目录。禁止「把 App Skill 安装进 `~/.agents`」。
4. **S-KD 4 — Workspace 两种：`sandbox` | `repo`。** 缺字段 = `sandbox`。新 id 默认沙箱。`repo` 仅桌面；path 必须是用户用系统选文件夹选出来的绝对目录。
5. **S-KD 5 — `project_root` 的权威改成 `WorkspaceSpec.resolved_path`。** `ChatAgent` 禁止再写死 `KimPaths.agentWorkspace`。旧共享目录保留只读迁移，不再当默认根。
6. **S-KD 6 — 统一入口 `activate_skill`；portable 另保留生态读法。** 开场只挂 catalog。`activate_skill` 对 portable = 读那份 `SKILL.md`；对 app / 内置 = `SkillResolver` 取**当前版本全文**再注入。项目内 portable 若已开 `fs`，模型也可以 `read_file .agents/skills/...`（和其它框架一样）。App Skill **禁止**只靠 `read_file`（文件不在 cwd）。
7. **S-KD 7 — Skill 可声明 `requires_tools`；禁止静默改 `ToolSet`。** 分配 App Skill 时若缺能力：toast + 可选「打开这些开关」，用户点了才写。bash 仍默认 ask_before，永不 AlwaysAllow。portable 的 `allowed-tools` 只作提示，不映射成 KIM ToolSet。
8. **S-KD 8 — 长期记忆 = 沙箱文件 + **App Skill**。** `MEMORY.md` + `notes/`。内置 `kim-memory` 教模型何时读/写。**不要**把这份 Skill 写进 `~/.agents`。不自动把 `MEMORY.md` 全文塞进 system prompt。
9. **S-KD 9 — 广场两架。** 「生态」= 扫描 `~/.agents` 与当前仓库 `.agents`，只发现、不拷贝。「KIM」= 内置 / 已分配 / 更新。导入 portable：写入 **真实** `~/.agents/skills`（或让用户自己放），不是 `support/agent/skills`。
10. **S-KD 10 — Plugin 是生态包装，不是 App Skill 的运行时。** 用户的 Open Plugin / Codex marketplace 落在 `.agents` 生态里；KIM 只扫 `skills/`。App / 内置不走 `plugin.json` 安装到家目录。
11. **S-KD 11 — 用户秘密仍本机；一等 Skill 包可以上云。** 遵守 bot-KD 12：key / prompt / workspace / `MEMORY.md` 不进 `chat.bot.create`。revises「Skill 正文永不进后台」：**KIM 托管的一等 Skill 包**（id、version、sha256、正文）可以像 `vendors.json` 一样分发、更新、共享。用户自写 portable 仍只在磁盘。
12. **S-KD 12 — 不把 Goose Recipe 当人设。** Recipe = 整份 agent YAML，与 `AgentProfile` 重叠。要分发「人设+工具+Skill」以后另做用户模板，且不绑 vendor（P-KD 1）。
13. **S-KD 13 — `AGENTS.md` 进 assemble，Skill body 不进。** 工作区根上存在则截断注入（上限 4 KiB）。任何 class 的 Skill 开场都只有 catalog 行。
14. **S-KD 14 — Release macOS 的 `repo` 必须走用户自选目录权利。** 加 `com.apple.security.files.user-selected.read-write`；把 security-scoped bookmark 和 path 一起持久化；`session_open` 前 `startAccessing`，`close` 时停。Debug 沙箱关着，测 bookmark 仍要走同一 API，避免 Release 才爆。
15. **S-KD 15 — `activate_skill` 是 host 进程内工具，不是 Dart IM 工具。** 不 yield。权限默认 always_allow（读的是已发现的 portable 或已分配的 app Skill）。
16. **S-KD 16 — 能力预设不是模板。** Advanced 可一键「知识（沙箱读写）」/「编码（读写+终端）」——只改当前表单的 `ToolSet` + 若编码则提示选仓库。不改创建短表单，不插入人设行。
17. **S-KD 17 — v1 不做 hooks。** 生命周期脚本另开切片。
18. **S-KD 18 — portable 只扫生态路径。** 本机：`~/.agents/skills`（真实用户家目录，见 S-KD 23）。项目：`<cwd>/.agents/skills`。v1 不扫 `.claude/skills`。**禁止**在 `KimPaths.agentWorkspace` / 沙箱里再 seed 一份给别的框架看的 `.agents`。
19. **S-KD 19 — `SkillClass` = `portable` | `app`。** `app` = 离开 KIM 能力面就没有正确行为。内置 Skill 是 KIM 署名的 `app` Skill，不是第三 class。
20. **S-KD 20 — 内置 Skill 永不落地 `~/.agents`。** 更新走解析器换版本，不改人设 JSON、不改用户家目录。多人设引用同一 `id`。
21. **S-KD 21 — `SkillResolver` 是内置的唯一取文面。** `bundled` → 本地 cache（`sha256`）→ （以后）云 catalog。`activate_skill` 取的是 pin 或 latest，不是 session_open 时拷进 conversation 的一份死副本。
22. **S-KD 22 — 上云共享 = 同一引用模型。** 人设存 `{ id, class: app, origin: cloud, version? }`。空 version = latest。云对象是**包**不是会话。v1 先落地 resolver + bundled + cache；拉网另 PR。
23. **S-KD 23 — Release 必须看见与 Claude 相同的 `~/.agents`。** macOS App Sandbox 的容器 `$HOME` 不是生态目录。用真实用户 home（bookmark `~/.agents`，或 home-relative 只读例外）。容器内自建 `.agents` **不算**适配生态。
24. **S-KD 24 — 不要把 `git-commit` / `code-review` 标成 KIM 内置。** 那是 portable；用户仓库或 `~/.agents` 里已有则发现即可。v1 内置只留 KIM 深绑：`kim-im`、`kim-memory`。
25. **S-KD 25 — Agent 忙碌复用 `typingProvider` + `KimTypingRow`，不新做一套指示器。** ChatPage 对 `isAgentDest` / `b_*` 也看 `peerTypingProvider(dest)`。桌面在 `prompt` 开始本地 `applyPush(typer: dest)`。
26. **S-KD 26 — 已注册 1:1 不能走 `chat.typing`。** typer 被写成 `session.account`（owner），电机会以为是主人在打字。Bot 无 JWT。新命令 `chat.bot.typing`：与 `chat.bot.reply` 同一套 owner 校验，`TypingPush.typer=bot`。v1 body 仍是 `active` bool，不加 hint 字段。
27. **S-KD 27 — 生命周期跟 host 相位，不跟 composer。** `Running` → typing on + 8s 心跳；`Yielded`（确认卡）→ off（轮到用户）；`Idle` / failed / abort / `assistant_finished` → off。进站 talk 已有 `clearDest(sender)`，回复到达也会清。
28. **S-KD 28 — 工具细节仍走本机 `agentCard`，不上 typing 协议。** 手机只看到 bars；桌面可以 bars + 卡片。禁止为进度新开 `chat.bot.progress` 或把 tool 名当消息刷屏。
29. **S-KD 29 — 创建仍是短表单，详情改成总览 + 子页。** 遵守 P-KD 2：`/agent/new` 不加工作区 / 能力 / Skill / MCP。**禁止**继续往现有 `ExpansionTile(Advanced)` 堆开关。人设页只留身份 + 模型 + prompt；能力、技能、权限各走子路由。
30. **S-KD 30 — 三个子页按用户任务切，不按数据结构切。** `/workspace` = cwd + 读/写/终端（「它在哪干活」）。`/skills` = 分配 `kim-*` + portable 发现/屏蔽。「权限与扩展」= 确认策略 + MCP（少用，放最后）。广场是**全局** `/agent/plaza`，不是第四个人设 Tab。
31. **S-KD 31 — 每页只存自己那一层，总览不脏子页。** 子页保存立刻 `saveProfile` 对应字段。创建成功 → **进入总览**（不是只 toast 回列表）；总览主按钮「去聊天」；磁贴上的缺省文案当引导，不弹强制向导。
32. **S-KD 32 — 「知识 / 编码」只是工作区页上的预设，不是创建向导。** 不恢复 `agent_wizard_sheet`。不绑 vendor。编码预设会要求选仓库（S-KD 4 / 14）。
33. **S-KD 33 — Buzz 可借，不可照搬。** 对照本地旁路仓库 `../buzz`（Block Buzz）。**借：**（a）提示词分层 — host 固定 `[Base]`（平台/工作区/Skill catalog）vs 人设 `systemPrompt`，禁止把平台说明写进人设；（b）创建短表单 = 名 + 人格提示词（+ 头像/模型），Advanced 折叠；（c）沙箱 `AGENTS.md` 用版本号 + `BEGIN/END MANAGED` 标记刷新，用户自写区不覆盖；（d）pack 级 Skill 作用域规则 — 写进 ≥1 人设 `skills:` 的只给那些人，未引用的给全员（映射到我们的分配 / denylist）；（e）远期广场单元对齐 OPS / Persona Pack（`plugin.json` + `skills/` + 多 persona），不另造包装格式。**不借：**（a）默认全体 Agent 共用一个 nest cwd（Buzz `~/.buzz`）——我们坚持 per-agent sandbox（S-KD 4/5）；（b）把 App 深绑 Skill（如 `buzz-cli`）写进 `.agents/skills` 再 symlink 给其它 harness——我们 `kim-*` 只动态注入（S-KD 3/20）；（c）Desktop 创建/编辑里没有 Skill 子页也能「靠 cwd 自动发现」糊弄过去——我们要显式 `/skills`（S-KD 30）；（d）Persona Pack 运行时整包部署（Buzz 自身也标 planned）——v1 不做。

---

## Buzz 对照（`../buzz`）

Buzz 与 KIM 同属「IM + 本机 Agent harness」。成熟点主要在 **Desktop 人设库 + nest 工作区 + Persona Pack 规范**，不是在「每 Agent 独立沙箱 + App Skill 动态注入」。

| 主题 | Buzz 怎么做 | KIM 现状 / 本文 |
|---|---|---|
| 身份模型 | **Definition（persona）** 与 **Instance（带 key 的进程）** 分离；实例可链回 definition 拿 prompt/model | `AgentProfile` ≈ definition；`b_*` 注册 ≈ instance。编辑时保持「人设改 → 下次 session 生效」即可 |
| 创建 UI | `AgentDefinitionDialog`：头像 + 名 + **Instructions（systemPrompt）** + harness/model；Advanced：respond_to / parallelism / env / Run on | 对齐 S-KD 29：创建短；详情拆子页，不堆 Advanced |
| 人格提示词 | 人设字段 `systemPrompt`；spawn 时 `effective_config` 解析 Definition vs InstanceLegacy | 已有；补 **host Base 层**（S-KD 13 catalog + 工作区约定），别塞进人设正文 |
| 默认 cwd | 全体默认 **`~/.buzz` nest**（`GUIDES/` `RESEARCH/` `PLANS/` `WORK_LOGS/` `OUTBOX/` `REPOS/`）；`REPOS` 可 symlink 到用户目录 | **反例**。我们每人设 sandbox 或自选 repo |
| 平台 Skill | nest 写入 `.agents/skills/buzz-cli/SKILL.md`，symlink `.goose` / `.claude` / `.codex`；版本文件刷新 | **反例**。`kim-im` / `kim-memory` 不进 `~/.agents` |
| portable Skill | 靠 harness 扫 cwd 的 `.agents/skills`；Pack 规范：deploy 时 copy + 人设 `skills:` 作用域（**copy 仍 planned**） | 扫真实 `~/.agents` + 项目；`activate_skill`；分配 UI |
| 打包 / 广场 | Persona Pack = OPS 超集：`agents/*.persona.md` + `skills/` + `instructions.md`；社区 **persona catalog**（分享人设，不是 Skill 市场） | 广场两架（S-KD 9）；远期 pack 可当 Plugin 单元（S-KD 33e） |
| 聊天里建 Agent | `buzz agents draft-create` → 加密草稿到主人 Desktop，**人工确认**才落库 | 可作 `kim-im` 后续能力；v1 不做 |

证据路径（Buzz）：`desktop/src-tauri/src/managed_agents/{nest,types,personas,discovery}.rs`；`desktop/src/features/agents/ui/AgentDefinitionDialog.tsx`；`crates/buzz-persona/PERSONA_PACK_SPEC.md`；`examples/meadow-core/`。

---

## 与既有决策的关系

| 决策 | 本文 |
|---|---|
| P-KD 1 不发模板、不做 Skills | **本切片开始做 Skills**；仍不发译者/编码模板 |
| P-KD 2 创建不含工具 | **遵守并收紧**：工作区 / Skill / 读写不进创建，也不进总览长表单，只进子页 |
| C-KD 2 不拉 CDN 目录 | **遵守**用户侧任意 URL；**一等 Skill catalog** 另算（S-KD 22） |
| bot-KD 12 后台不存 key | **遵守** key / 人设 / 工作区。**revises**「任何 Skill 都不进后台」：仅 KIM 一等包可托管 |
| 个性化 Agent「不引入 Recipe/Skill」 | **revises 那一句**：Skill 由 KIM 自实现，仍不 git 依赖 `block/goose` |
| 两条 FFI 隔离 | **遵守**。`activate_skill` 在 host 内读盘 |

---

## Proposed Design

### 目标分层

```text
生态 portable（其它 harness 也能读）
  ~/.agents/skills/<id>/SKILL.md          ★ 本机
  <cwd>/.agents/skills/<id>/SKILL.md      ★ 项目
        │ 扫描 catalog，不拷进 app
        ▼
KIM App / 内置（只对本 harness 有意义）
  bundled | cache | 以后 cloud catalog
        │ 详情显式分配
        ▼
AgentProfile.skills[]  ──► session_open ──► SkillRegistry
AgentProfile.workspace ──► resolved cwd ──► Fs/Bash root
```

```text
activate_skill(id)

portable  ──► 读 ~/.agents 或 <cwd>/.agents 的 SKILL.md
app/内置  ──► SkillResolver(id, pin)
                 ├─ bundled
                 ├─ local cache (sha256)
                 └─ ★ 以后 cloud latest
              ──► hidden 全文注入（不是 kickoff）
```

```text
session_open 之后的机器（相对今日）

SystemPromptOp     ← persona + AGENTS.md(≤4KiB) + 两类 catalog ★
Steer / MaxTurns / Compaction / Permission     (现有)
DeferredKim        (现有 IM 工具)
ToolOperation
  ├─ Fs/Bash       root = workspace.path   (现有, 根不再写死)
  ├─ SkillOp       activate_skill ★
  ├─ MCP / Subagent (现有)
  └─ UnknownTool
InferenceRunner
```

### 工作区布局

```text
~/.agents/skills/<id>/              (现有生态；Claude/Cursor/Goose/Codex 共用)
<repo>/.agents/skills/<id>/         (项目生态；随 git)

Application Support/agent/
  sessions/                         (现有)
  workspace/                        ✂ 不再当默认根
  workspaces/<profile_id>/          ★ sandbox（不是生态 .agents）
    AGENTS.md
    MEMORY.md
    notes/
  app-skills/cache/<id>/<version>/  ★ 内置解析缓存（用户不可当 portable）
```

沙箱种子（`ensureSandbox(profile_id)`，已存在则不覆盖）：

```text
# AGENTS.md
This is a private workspace for this KIM agent.
Long-term notes live in MEMORY.md and notes/.
```

```text
# MEMORY.md
(empty — the long-term-memory skill says when to append)
```

`repo` 不往用户仓库写种子文件。仓库里已有的 `AGENTS.md` / `.agents/skills` 按 portable 发现。沙箱 **不** 创建 `.agents/skills`（S-KD 18 / 24）。

### 解析 cwd

```text
resolve_workspace(profile, kim_paths):
  spec = profile.workspace ?? { kind: sandbox, path: "" }
  if spec.kind == sandbox:
    dir = kim_paths.agentWorkspaces / profile.id
    create seed if missing
    return dir
  if spec.kind == repo:
    if !desktop: reject
    bookmark.startAccessing(spec.bookmark_ref)   ★
    if !exists(spec.path) || !is_dir: HostError::Workspace
    return spec.path
```

`ChatAgent._ensureSession` 把 **解析后的绝对路径** 传给现有 `projectRoot`。host 仍只看见一条根，fs/bash 合同不变。

### Skill 两类与扫描

```text
class=portable                         class=app（含内置）
  origin=user    ~/.agents/skills        origin=bundled   二进制
  origin=project <cwd>/.agents/skills    origin=cache     support/agent/app-skills
                                         origin=cloud     ★ 同一 id@version
```

同名冲突（例如项目和 `~/.agents` 都有 `git-commit`）：**project > user**。App id 必须走 `kim-` 前缀（`kim-im`、`kim-memory`），禁止和 portable 抢名。

人设只存 **app** 引用。portable 靠发现，用可选屏蔽表关掉噪声：

```json
"skills": [
  { "id": "kim-im", "class": "app", "origin": "bundled", "enabled": true },
  { "id": "kim-memory", "class": "app", "origin": "cloud", "version": "", "enabled": true }
],
"portable_denylist": ["noisy-skill"]
```

`version: ""` = latest（下次 `activate_skill` 可换文）。钉死则写 `"1.2.0"`。

扫描失败（缺 `SKILL.md`、家目录不可达）→ 该项不进 catalog + `tracing::warn`，不挡 `session_open`。

### 渐进披露

```text
ChatAgent          AgentHost              SkillOp           磁盘
  │ prompt              │                    │                │
  │ ──►                 │ assemble           │                │
  │                     │ ──► catalog ★      │ 读 frontmatter │
  │                     │ system += 目录     │                │
  │                     │ Inference          │                │
  │                     │ ToolReq activate   │                │
  │                     │ ──►                │ 读 SKILL.md    │
  │                     │                    │ ──►            │
  │                     │ ◄── body           │                │
  │                     │ Append hidden msg  │                │
  │                     │ + ToolResponse     │                │
  │                     │ Inference (sees    │                │
  │                     │  instructions)     │                │
```

`activate_skill` inputSchema：

```json
{
  "name": "activate_skill",
  "description": "Load an assigned skill's instructions (or a file under that skill). Use when the task matches a listed skill.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "id": { "type": "string" },
      "path": {
        "type": "string",
        "description": "Relative to the skill directory. Default SKILL.md"
      }
    },
    "required": ["id"],
    "additionalProperties": false
  }
}
```

规则：

- `id` 必须在本 session catalog 里（portable 已发现且未 denylist，或 app 已分配且 enabled）。否则错误 JSON。
- portable：`path` 落在该 Skill 目录内（canonicalize + prefix）。默认 `SKILL.md`。
- app / 内置：默认取解析器的主文档；`path` 只允许包内 `references/` 相对路径，同样防逃逸。
- 默认文件上限 64 KiB；超出截断并注明 truncated。
- 注入：`Message::user().with_text(body).with_visibility(false, true)`，**不是** kickoff。正文带一行 `skill: id@version`（info，可打 log），便于更新后对比。
- 同一 `(id, version, path)` 本轮已激活 → `{"ok":true,"already":true}`。**换 version 再激活**应追加新正文（更新场景）。
- 不设 `goose.external_dispatch`。

System prompt 追加块（两类分行，避免模型把 `kim-im` 当成「也可以给 Cursor 用」）：

```text
Project/user skills (portable; also visible to other agents on this machine):
- git-commit: Create Conventional Commits ...

KIM app skills (only work in this messenger; call activate_skill):
- kim-im: Send IM to friends with confirmation ...
- kim-memory: Persist facts to this agent's MEMORY.md ...
```

只放 `name` + `description`（截断 256 字）。App 行必须写清「only in KIM」。

### 内置 App Skill（v1 两份）

| id | 给谁 | `requires_tools` | 为何不能进 `~/.agents` |
|---|---|---|---|
| `kim-im` | IM 助手 | `send_message` 等 | 教确认卡 / 拒群 dest / Outbox |
| `kim-memory` | 沙箱知识 | `fs`, `fs_write` | 记的是 app 沙箱 `MEMORY.md` |

**不要** bundled `git-commit` / `code-review`。正文自写、短；不 vendoring 本仓库 rust-skills。

新 id 默认工具仍是 `kCreateDefaultTools`。**不**自动分配 `kim-memory`。分配 App Skill 缺工具时走 S-KD 7。

### 能力与工作区怎么配合

```text
                    sandbox cwd              repo cwd
IM-only (默认新)    不用 fs；无记忆文件         无意义，不要选 repo
知识 / 记忆         fs+write；MEMORY.md        少见
coding              可，但通常不够             fs+write+bash；用户选仓库
```

读写开关只出现在 **工作区子页**（S-KD 30），不进总览、不进创建。

快捷（仅工作区页顶部，可选点）：

- 「知识」→ sandbox + fs + fs_write，建议去技能页加 `kim-memory`
- 「编码」→ 提示选 repo；fs + fs_write + bash；portable 随仓库自动发现；不自动 AlwaysAllow

### 页面与流程（revises「堆进 Advanced」）

今日详情已是厨房水槽边缘：主字段 + 一个 `ExpansionTile` 塞 fs / bash / MCP / 权限。再加工作区、写、Skill、denylist 会不可用。

```text
/agent                     列表（+ 广场、钥匙）
/agent/new                 短表单（P-KD 2，零新增字段）
/agent/:id                 总览：人设 + 三块入口
/agent/:id/workspace       工作区 + 读/写/终端
/agent/:id/skills          该人设的 App Skill / portable
/agent/:id/tools           权限 + MCP
/agent/plaza               生态架 + KIM 架（全局）
/agent/accounts…           Provider（现有，不动）
```

```text
列表 ──► 创建（名称/Provider/模型/推理/prompt）
         │ 保存
         ▼
       总览 ──► 去聊天（通讯录 1:1）
         │
         ├─► 工作区与能力     cwd + fs/write/bash
         ├─► 技能             分配 / 屏蔽 / 去广场
         └─► 权限与扩展       少用
```

总览主区 = 今天创建/编辑的主字段（名称、别名、Provider、模型、推理、prompt）。主区下面三块 `ListTile`，subtitle 是状态不是控件：

| 入口 | subtitle 例子 |
|---|---|
| 工作区与能力 | `应用内沙箱 · 未开读写` / `…/im · 读·写·终端` |
| 技能 | `未分配` / `kim-im · 生态 3` |
| 权限与扩展 | `默认 · 无 MCP` |

创建成功 **push 总览并去掉 new**（`go('/agent/$id')`），不要只 pop 回列表。总览不弹 modal 向导。用户可以马上聊天；磁贴缺省文案负责「还可以再配」。

广场从列表 header 进；技能页有「从广场添加」→ `/agent/plaza?assignTo=<id>`，KIM 架点选后写回该人设 `skills[]`。

**禁止：** 总览上再挂 ExpansionTile；创建多步向导；把 Provider URL/key 拉回人设页；一个 TabController 把四页状态焊在同一个 `State` 里（子页各自存盘）。

### 广场 IA

`/agent/plaza` 两段，不要合成一个「已安装库」。

| 段 | 内容 |
|---|---|
| 生态 | 列出真实 `~/.agents/skills` 与（若当前编辑的人设是 repo）`<cwd>/.agents/skills`。只读发现。导入 = 拷到 `~/.agents/skills` |
| KIM | 内置 `kim-*`；已分配到哪些人设；「更新」走 resolver（v1 刷新 bundled/cache） |

v1 不做评分、付费、第三方 URL 安装。MCP 仍只在人设 Advanced 手填。

生态侧的 `plugin.json`：KIM **只扫其中的 `skills/`**，不执行 hooks、不安 MCP。

### macOS 沙箱（repo 的门槛）

今日：

- Debug：`app-sandbox=false`（`DebugProfile.entitlements`）
- Release：`app-sandbox=true`，仅 `network.client` + keychain

`repo` 在 Release 要同时具备：

1. Entitlement `com.apple.security.files.user-selected.read-write`
2. 系统 `NSOpenPanel` / Flutter 等价物选出目录
3. 把 bookmark data 存 keychain 或 support 文件（**不要**只存 path 字符串当权威）
4. 每次 `session_open` `startAccessingSecurityScopedResource`；`close` / 进程退出 stop

没有 bookmark 的旧 path：打开详情时视为失效，强制重选。沙箱 kind 不需要 bookmark。

Windows / Linux：无 bookmark；存 path，open 时 `exists` 即可。

### Agent 执行态接「正在输入」

已有线：

```text
真人 composer
  onComposerTyping ──► chat.typing {dest=peer, active}
  ══► TypingPush {typer=me} ──► typingProvider ──► KimTypingRow

ChatPage 今日：isAgentDest → 不看 peerTyping（本地助手永远没有行）
chat.typing 的 typer 永远是 session.account
Bot 无 JWT / 无 Location（bot-KD）
进站 talk 已 clearDest(sender)
```

目标：用户交出任务之后、Agent 回复落地之前，同一条 `KimTypingRow` 亮着。

```text
ChatAgent.prompt / 已注册 echo
  │ ① typingProvider.applyPush(typer=threadDest)     ★ 本机立刻亮
  │ ② dest 已注册? ──► chat.bot.typing {active}      ★ 手机 / 其它桌面
  │      8s 心跳保持
  │
  ├─ Running（推理 / 本机工具）     保持 on
  ├─ Yielded + 确认卡              off（等用户）
  ├─ complete_tool / 继续          再 on
  └─ Idle / failed / abort / 回复  off
       （bot.reply 的 talk Push 也会 clearDest）
```

**为什么必须新命令：** owner 调现有 `sendTyping(dest=自己)` 非法；`sendTyping(dest=bot)` 会让对端看到「主人在打字」。`do_bot_typing` 与 `do_bot_reply` 同构：`lookup_bot` + `owner_account == session`，fanout `TypingPush.typer=bot`、`dest=owner`，viewers 是进了 `dest=bot` 房间的 owner 设备。

v1 不扩展 `TypingReq` 字段。工具名 /「正在读文件」只留在桌面 `agentCard`。手机只看到 bars——和真人输入同一套视觉，足够表达「对面在忙」。

接收端建议加 **20s 无刷新 TTL**（人类误停也会卡死，不是 Agent 专属）。心跳 8s 小于 TTL。

---

## API / Interface Changes

无 proto、无 gateway。

### Rust

```rust
// profile.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    #[default]
    Sandbox,
    Repo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceSpec {
    #[serde(default)]
    pub kind: WorkspaceKind,
    /// sandbox: 空。repo: 绝对路径（展示/校验）；bookmark 只在 Dart。
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillClass {
    Portable,
    App,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillRef {
    pub id: String,
    pub class: SkillClass,
    /// bundled | cache | cloud — 仅 class=app。portable 不进此数组。
    #[serde(default)]
    pub origin: String,
    /// 空 = latest
    #[serde(default)]
    pub version: String,
    #[serde(default = "enabled_true")]
    pub enabled: bool,
}

impl AgentProfile {
    // 已有 tools / extensions；新增：
    // workspace: WorkspaceSpec
    // skills: Vec<SkillRef>
}
```

`session_open` **不**新增大字段。cwd 继续走现有 `projectRoot`。`profile_json` 带上 `workspace` + `skills`（host 用 skills 建 registry；cwd 仍以 FFI 传入的已解析路径为准，避免 host 猜 bookmark）。

新模块 `crates/kim-agent-host/src/skills.rs`：

- `scan_portable(user_agents, project_agents)` / `parse_skill_md`
- `SkillResolver`：`resolve(id, version) -> SkillPackage`（bundled → cache → 以后 cloud）
- `SkillRegistry`：portable 发现 ∪ app refs
- `SkillOp`：`activate_skill` → 读盘或 resolver → hidden append

hidden 消息：不要在 `ToolProvider::call` 里偷偷 mutate conversation。两种合法做法（选 **a**）：

- **a.** `SkillOp` 作为 `Operation`（不是 ToolProvider）：`run` 看见未回答的 `activate_skill` → 读盘 → `ConversationEffect::AppendMessage`（hidden user）+ `ToolResponse`，`applied`。
- b. ToolProvider 只返回 body，另写 HostEffect。多一层。

`MachineFactory`：catalog 非空才装 `SkillOp`（包括「只有自动发现的 portable」）。

FFI：

```rust
pub fn skill_portable_list(user_root: String, project_root: String) -> Result<String, String>;
pub fn skill_app_catalog() -> Result<String, String>; // 内置摘要 + cache version
```

**选定：Rust 解析 `SKILL.md` 与 resolver，Dart 不写第二份 YAML。**

### Dart

```dart
class WorkspaceSpec {
  final String kind; // sandbox | repo
  final String path;
  final String bookmarkRef; // keychain key；不进 toHostJson
}

class SkillRef {
  final String id;
  final String className; // portable 不持久化在人设里
  final String origin; // bundled | cache | cloud
  final String version;
  final bool enabled;
}

class AgentProfile {
  // + workspace, skills
}
```

`toJson`：写 `workspace` / `skills`。`toHostJson`：**不写** `bookmarkRef`。`ChatAgent` 解析 cwd 后走现有 `projectRoot`。

`KimPaths`：

```dart
Directory get agentWorkspaces => Directory('${support.path}/agent/workspaces');
Directory get appSkillCache => Directory('${support.path}/agent/app-skills/cache');
Directory sandboxFor(String profileId) =>
    Directory('${agentWorkspaces.path}/$profileId');
// 真实用户 ~/.agents/skills — 不是容器 HOME，见 S-KD 23
Future<Directory?> realUserAgentsSkills();
```

`agentWorkspace`（单数）留下，测试与旧注释改成「legacy shared root, unused」。

新页：`sdk/mobile/lib/screens/agent/agent_plaza_page.dart`。详情 Advanced 加工作区块 + Skill chips。

`file_picker`（或 macos `NSOpenPanel` 通道）仅桌面。测试用注入 `WorkspacePicker`。

---

## Data Model Changes

| Key / 路径 | Store | 内容 |
|---|---|---|
| `agent.profiles[].workspace` | prefs | `{kind, path}`。sandbox 的 path 可空 |
| `agent.profiles[].skills` | prefs | **仅 app** `SkillRef[]` |
| `agent.profiles[].portable_denylist` | prefs | 关掉的 portable id |
| `agent.workspace_bookmark.<profile_id>` | keychain | repo bookmark |
| `agent.agents_dir_bookmark` | keychain | ★ 真实 `~/.agents`（Release macOS） |
| `~/.agents/skills/` | 用户磁盘 | portable，与其它 harness 共用 |
| `<repo>/.agents/skills/` | 仓库 | portable |
| `support/agent/workspaces/<id>/` | 磁盘 | 沙箱；**无** `.agents/skills` |
| `support/agent/app-skills/cache/` | 磁盘 | 内置解析缓存 |
| 二进制 `app-skills/` | host crate | `kim-im` / `kim-memory` |

无 SQL、无 proto。旧客户端忽略未知字段 → 仍共用旧 `agentWorkspace`（ChatAgent 未改时）。**cwd 行为变化必须与 ChatAgent 同 PR**，避免半套。

迁移：

```text
profile.workspace 缺失:
  当 sandbox，不搬旧共享目录文件
  不改 tools
```

---

## Alternatives Considered

### 1. 所有已安装 Skill 自动注入 vs 显式分配

App / 内置：显式分配。portable 项目树：自动 catalog（生态预期）；`~/.agents`：自动 catalog，denylist 去噪。**拒绝**把 App Skill 自动灌进每个人设。

### 2. 模型自己 `read_file` Skill vs `activate_skill`

portable 在 cwd 内：两种都行。App / 内置：**必须** resolver + `activate_skill`。**拒绝**把内置正文写进仓库让 `read_file`。

### 3. 记忆用 SQLite vs 文件

库要迁移、要新 UI。文件 + Skill 与主流 harness 一致，用户能打开看。**选定文件。**

### 4. Recipe YAML 当「可分发 Agent」

与 `AgentProfile` + ProviderAccount 重叠，且绑模型。**拒绝 v1。**

### 5. 远程广场 vs 本机 vs 一等云包

任意第三方 CDN：**v1 拒绝**。KIM 署名 `id@version` 包：resolver 预留，拉网另 PR（S-KD 22）。

### 7. App Skill 写进 `~/.agents` 以便「统一扫描」

其它框架会加载并幻觉 `send_message`。**拒绝。**

### 6. 共享一个 workspace + 子目录 per agent

仍要改路径，却保留「默认根可逃到兄弟目录」的心智负担。**拒绝。** 每人设独立根。

---

## Security & Privacy Considerations

| 威胁 | 严重度 | 缓解 |
|---|---|---|
| repo cwd 读出整盘 | **高** | 用户自选；prefix jail；Release 无 bookmark 打不开；拒绝 `/`、拒绝选到 `$HOME` 本身（toast 可强选，默认挡） |
| bash 在用户仓库执行任意 argv | **高** | 默认关；ask_before；无 shell 拼接；广场不自动开 bash |
| 恶意 `SKILL.md` 诱导外发 / 读剪贴板 | **高** | App Skill 只加载已分配；写工具仍走确认卡；portable 进 denylist；导入 `~/.agents` 预览前 2 KiB |
| App Skill 泄漏进 `~/.agents` | **高** | 安装路径断言；CI 禁止 `kim-*` 出现在用户 skills 导出 |
| 容器 HOME 冒充 `~/.agents` | **高** | S-KD 23；单测路径 ≠ container home |
| Skill `path` 逃出技能目录 | **高** | 与 fs 同一 canonicalize + prefix |
| 静默打开 fs/bash | **高** | S-KD 7 |
| MEMORY.md 进云 / 进 botCreate | **高** | 文件只在 support。上云的是 **Skill 包**，不是记忆文件 |
| bookmark / 路径进日志 | 中 | info 只打 `kind` + `profile_id` |
| 广场暗装 MCP 进程 | **高** | v1 不从广场写 `extensions` |
| App Sandbox 形同虚设（Debug 关、Release 忘加 entitlement） | **高** | repo 路径单测 + 文档；无 entitlement 的 Release 选文件夹应失败并 toast |

Auth：Agent 仍不持 JWT。IM 工具合同不变。

---

## Observability

- `tracing`：`workspace.kind`、`skills.n`、`activate_skill id ok=`（无 path 正文、无绝对 cwd）
- Dart toast：仓库失效请重选；分配 Skill 缺工具；导入无 `SKILL.md`
- 回归：沙箱 per-id 隔离；repo 逃逸；`activate_skill` hidden 不是 kickoff；未分配 id 失败；旧 profile 无字段 → sandbox

---

## Rollout Plan

桌面-only Goose。无新 pref 总闸；`agentHostSupported` 仍是总闸。

顺序：**沙箱 cwd → repo bookmark → portable 扫描 + `activate_skill` + 内置 resolver → 详情分配 → 广场两架 →（另 PR）云 catalog。**

回滚：忽略 `workspace`/`skills`；ChatAgent 若回退会再次共用旧根——cwd PR 与 store 字段必须同列车。

---

## PR Plan

每条可单独审。不改 `services/**`、不改 `kim-client` 热路径、不改 proto。

### PR1 — per-agent sandbox cwd

- **Title:** `agent: per-profile sandbox workspace as project_root`
- **Depends:** PR-U1（工作区子页已在）
- **Desc:** 旧 `agent/workspace` 不再传入。不搬文件。`fs_write` UI 同 PR 补上。

**文件级清单**

| 文件 | 改动 |
|---|---|
| `sdk/mobile/lib/core/paths.dart` | `agentWorkspaces` / `sandboxFor` / `ensureSandbox`；`ensureAgentDirs` **不再**建共享 `.agents/skills` |
| `sdk/mobile/lib/agent/workspace.dart` | **新** `resolveAgentProjectRoot` |
| `sdk/mobile/lib/state/agent_profiles.dart` | `WorkspaceSpec`；人设默认 sandbox；`toHostJson` 去 `bookmark_ref` |
| `sdk/mobile/lib/state/chat_agent.dart` | `projectRoot` = 解析后的沙箱/repo；`_machineChanged` 含 workspace |
| `…/agent_workspace_page.dart` | 展示 cwd；fs / **fs_write** / bash；知识/编码预设 |
| `…/agent_overview_status.dart` | subtitle 含写 |
| `test/agent_paths_test.dart` | 两 profile 隔离；无假 `.agents`；repo 回退 |

**验收：** 两人设 `read_file` 互不可见；总览 subtitle 反映写；共享 `agent/workspace` 不再作 `projectRoot`。

### PR2 — repo cwd + macOS user-selected bookmark

- **Title:** `agent: repo workspace picker and security-scoped bookmarks`
- **Depends:** PR1
- **Desc:** 无 bookmark 的 Release 不得假装 repo 可用。

**文件级清单**

| 文件 | 改动 |
|---|---|
| `sdk/mobile/macos/Runner/{Release,DebugProfile}.entitlements` | `com.apple.security.files.user-selected.read-write` |
| `…/WorkspaceBookmarkPlugin.swift` + `MainFlutterWindow` + `pbxproj` | MethodChannel `kim.workspace`：选目录 / bookmark start·stop / `realHomeAgentsSkills` |
| `sdk/mobile/lib/agent/workspace_access.dart` | **新**；macOS 走插件，其它平台 `file_picker`；bookmark 进 secure storage |
| `sdk/mobile/lib/agent/workspace.dart` | repo：`startAccessing`；失效 → `invalidRepo` + 回退沙箱 |
| `…/agent_workspace_page.dart` | 「使用本地仓库」开关；选仓 / 失效文案；Release 无 bookmark 拒存 |
| `…/chat_agent.dart` | session close 停 bookmark；`user_agents_skills` 进 `toHostJson` |
| `l10n` | repo 相关文案；总览 `kind · caps` |

**验收：** Release 选仓后可 `read_file`；删 bookmark / 失效后强制重选；Windows/Linux 只存 path。

### PR3 — portable 扫描 + 内置 resolver + `activate_skill`

- **Title:** `host: portable .agents scan and kim-* SkillResolver`
- **Depends:** PR1（cwd 已稳）。项目扫描依赖 PR2 的真实 repo cwd，可先用测试目录。
- **Desc:** 尚无广场页。Release `~/.agents` bookmark 可本 PR 或跟 PR2 绑（S-KD 23）。

**文件级清单**

| 文件 | 改动 |
|---|---|
| `crates/kim-agent-host/src/skills.rs` | `scan_portable` / `SkillResolver` / `build_registry` / `catalog_prompt_block` / `activate`；`skill_portable_list_json` / `skill_app_catalog_json` |
| `…/app-skills/kim-im\|kim-memory/SKILL.md` | 内置 app skill |
| `…/ops/skill.rs` | `SkillOp`：`activate_skill` → hidden user + ToolResponse |
| `…/machine.rs` | `fs\|\|fs_write\|\|repo` 才扫 portable；catalog → system prompt；非空装 `SkillOp` |
| `…/profile.rs` | `workspace` / `skills` / `portable_denylist` / `user_agents_skills` |
| `sdk/mobile/rust_agent/.../session.rs` | FFI `skill_portable_list` / `skill_app_catalog`（Dart 侧待 FRB 再生成后接广场） |
| `sdk/mobile/.../agent_profiles.dart` | `SkillRef`；`toHostJson` 带 skills / denylist / user shelf |

**验收：** project 压过 user；`kim-*` 不读 `~/.agents`；activate 后 hidden 非 kickoff；换 version 再注入；path 逃逸 / 未在 catalog 失败。

### PR4 — 详情：分配 App Skill + portable denylist

- **Title:** `mobile: assign kim skills; denylist portable`
- **Depends:** PR3
- **Desc:** 创建短表单仍无 Skill。落点是 `/skills` 子页（不是 Advanced）。

**文件级清单**

| 文件 | 改动 |
|---|---|
| `…/agent_skills_page.dart` | KIM chips（assign）+ portable 屏蔽开关；S-KD 7 对话框 |
| `…/skills_catalog.dart` | **新**；catalog JSON / requires_tools / import 辅助 |
| `l10n` | 技能 / 缺工具 / 屏蔽文案 |

**验收：** 分配 `kim-memory` 缺写 → toast；点「打开这些开关」才写 ToolSet；portable mute 进 denylist。

### PR5 — 广场两架

- **Title:** `mobile: plaza ecosystem shelf and KIM shelf`
- **Depends:** PR4
- **Desc:** 禁止导入进 Application Support。

**文件级清单**

| 文件 | 改动 |
|---|---|
| `…/agent_plaza_page.dart` | 生态架扫真实 `~/.agents`；KIM 架分配/版本；导入写 `~/.agents/skills` |
| `app_router.dart` | 读 `?assignTo=` |
| `l10n` | 两架 / 导入 / 分配文案 |

**验收：** 从技能页带 `assignTo` 进广场可分配；导入含 `SKILL.md` 的目录；`kim-*` 拒导入。

### PR6 — 文档回写

- **Title:** `docs: agent productivity shape`
- **Depends:** PR5
- **Files:** `docs/agent-goose.md`；`glossary.md`（portable / App Skill / SkillResolver / bot.typing）；P-KD 1 加指针
- **Desc:** 不改后台协议字段。

### PR-U1 — 详情拆成总览 + 子路由（先于往 Advanced 堆功能）

- **Title:** `mobile: split agent editor into overview and child pages`
- **Depends:** 无
- **Desc:** 行为不变，只改 IA。后续工作区 / Skill / `fs_write` **只写子页**。

**文件级清单**

| 文件 | 改动 |
|---|---|
| `sdk/mobile/lib/router/app_router.dart` | `plaza` 在 `:id` 前；`:id` 下挂 `workspace` / `skills` / `tools` |
| `sdk/mobile/lib/screens/agent/agent_settings_page.dart` | 删 `ExpansionTile`；编辑态加人设下三入口 +「去聊天」；创建 `save` → `go('/agent/$id')`；编辑 save **只**写身份/模型/prompt（工具字段改子页） |
| `…/agent_workspace_page.dart` | **新**；迁 fs / bash 开关；本页 `saveProfile` |
| `…/agent_skills_page.dart` | **新**；空态 +「去广场」（PR4 再填） |
| `…/agent_tools_page.dart` | **新**；迁 MCP + 权限下拉；本页存盘 |
| `…/agent_plaza_page.dart` | **新**；空态（PR5 再填） |
| `…/agent_list_page.dart` | header 加广场按钮 |
| `l10n/app_{zh,en}.arb` + gen | 入口标题 / subtitle / 空态文案 |
| `test/agent/agent_editor_page_test.dart` | 创建无 fs/bash/「高级」；编辑有三入口、无 ExpansionTile「高级」 |

**验收：** 创建页树无读写开关；总览三磁贴可进子页；子页改 bash 后回总览 subtitle 更新；Advanced tile 不存在。

### PR-T1 — 本机 Agent 接 typing 行（无后台）

- **Title:** `mobile: show typing row while Goose is running`
- **Depends:** 无（不改 proto）
- **Files:** `chat_page.dart`（agent / `b_*` 也读 `peerTypingProvider`）；`chat_agent.dart`（prompt / Yielded / finished）；`typing.dart` 20s TTL
- **Desc:** 未注册只本机可见。

### PR-T2 — `chat.bot.typing`（已注册 1:1 同步到手机）

- **Title:** `chat: owner-sent bot typing, same auth as bot.reply`
- **Depends:** PR-T1；bot.reply 已在
- **Files:** `CMD_BOT_TYPING`；`do_bot_typing`；kim-client `bot_typing`；FFI `botTyping`；ChatAgent 8s 心跳；Push 仍 `CMD_TYPING` + `typer=bot`
- **Desc:** 不改 `do_typing` 热路径。非 owner → 112。

### PR7 — 一等云 catalog（可后置）

- **Title:** `agent: cloud skill packages for kim-*`
- **Files:** resolver 拉 KIM catalog；校验 sha256 / 签名；cache；空 version = latest
- **Depends:** PR3
- **Desc:** 不上传用户 `MEMORY.md`。不开放任意 URL。

**刻意不做：** hooks；第三方远程广场；Goose Recipe；把 rust-skills / git-commit 标成内置；seatbelt；沙箱里假 `.agents`。

---

## Open Questions

None 作为实现分叉——产品形状按 S-KD。若要改下面三条，另开修订，不要在 PR 里临时改：

1. 新 Agent 是否默认分配 `kim-memory`：**否**（S-KD 8 / 默认工具不变）。
2. 广场是否安装 MCP：**v1 否**。
3. `$HOME` 当 repo：**默认拒绝，可在确认后允许**（S-KD 安全表）。
4. `~/.agents` 是否自动进每个 Agent 的 catalog：**仅 repo / 开了 fs 的人设**（S-KD 3）。
5. 云 catalog：**PR7**，v1 只把 resolver 接口留好。

---

## References

- Agent Skills spec: https://github.com/agentskills/agentskills
- Claude Code skills / plugins；Cursor plugins；Goose Open Plugins；Codex `marketplace.json`
- Buzz（旁路对照）：`../buzz` — `PERSONA_PACK_SPEC.md`、nest、`AgentDefinitionDialog`（见 S-KD 33）
- `docs/impl/goose-personalized-agents.md` — 机器、工具、yield
- `docs/impl/agent-provider-persona.md` — P-KD 1 不做模板
- 代码：`profile.rs`、`machine.rs`、`ops/fs.rs`、`ops/bash.rs`、`chat_agent.dart`、`paths.dart`、`agent_settings_page.dart`、`macos/Runner/*entitlements`
