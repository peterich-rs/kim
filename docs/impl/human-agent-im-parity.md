# 人↔Agent 1:1 对齐人↔人 IM（消息 + 活动态）

| Field | Value |
|---|---|
| Author | KIM Agent Working Group |
| Date | 2026-09-16 |
| Status | Implementing |
| Audience | 投递 / kim-sdk / Flutter 壳 |
| 范围 | 人↔人回归核对 + 人↔Agent 与人↔人同一套多端实时体验。**不做** Agent↔Agent |
| 推翻 | `#126` 后「`bot_reply` 不写 timeline、靠 AgentTurn / Push 回显」；人↔Agent 不进房、不发 typing；agent 忙碌只活在本机 |
| 保留 | Bot 不上线、无 JWT、无 Location；Goose 只桌面；`kim_agent_ffi` 与 `kim_client_ffi` 不合并；不服务端代跑模型 |
| 父规格 | [goose-bot-first-class.md](./goose-bot-first-class.md)、[reliable-delivery.md](../reliable-delivery.md)、[presence-room-interest.md](../presence-room-interest.md)、[08-kim-sdk-ownership.md](./08-kim-sdk-ownership.md) |

编号：**HA-KD *n***。不重开 bot-KD。合入后形状写回 [reliable-delivery.md](../reliable-delivery.md)、[agent-goose.md](../agent-goose.md)、[presence-room-interest.md](../presence-room-interest.md)；本文件按 impl README 删除。

---

## Overview

人↔人已经是「账号 inbox + 多端 location」：正文落库、Push 尽力、发送端靠本地 outbox、正在输入走 `chat.typing` + 房间兴趣。人↔Agent 被接进同一套 IM，但 **代发连接被当成「已经有这条消息」**，活动态又被挡在 agent 线程外。结果是：Mac 看不见助手句、手机看不见自己在 Mac 发的触发句、输入中/忙碌上不了云。

本切片把人↔Agent 收成 **同一投递契约、同一活动态、同一套 UI 槽位**（气泡 = timeline，输入中/忙碌 = 列表 footer `KimTypingRow`）。Agent 执行细节（queued / running / tool / wait_permission）作为活动态的 `phase` 上云，不是第二套会话。

```
人↔人          人↔Agent
alice → bob     alice → b_bot
bob   → alice   b_bot → alice   （owner 桌面代发，sender 仍是 bot）

同一 insert / Push / 离线 / 活动 fanout
Goose 只决定谁跑模型，不决定哪台设备看见消息
```

---

## Background（代码事实，不是愿望）

### 人↔人（HEAD，mobile 壳之后）

| 环节 | 现状 | 风险 |
|---|---|---|
| 发 | outbox → `chat.user.talk` → `mark_sent` | 正常 |
| Push | `dispatch` **跳过本 `channel_id`**；`fanout.recipients` 含 sender+dest，其它端能 echo | 发送端不靠 Push |
| 离线 | 只拉 `direction=0` | **自己发出的话第二台设备重连后补不回**（人聊也有，只是不显眼） |
| 进房 | `ChatSessionNotifier._enterRoom` 仅 `isUserThread` | `_enterRoom` `catch (_) {}` 失败静默 |
| 输入中 | composer `onTypingChanged` → `sendTyping`；SDK `TypingUpdated` → `link.dart` → `typingProvider.applyPush(thread=typer)` → `KimTypingRow` | 协议还在；缺「打开会话出现等化条」的壳回归 |
| 动画 | `KimTypingBars` 自驱动 `AnimationController.repeat` | widget 测有；未挂在 live Push 上 |

`isUserThread` = `kind==user && !isAgentDest`。`isAgentDest` **只**认 `goose` / `agent:`。已注册 `b_…` 被当成普通人聊：会 `roomEnter`、会 `sendTyping`。

### 人↔Agent（HEAD）

| 环节 | 现状 |
|---|---|
| 用户句 | 走 outbox / `chat.user.talk`。Push 含 sender 账号，手机**在线**时应能收到；离线/receipt 只给 receiver=bot（无 loc）则第二台永远补不回 |
| 助手句 | `#114` 有 `_commitServerAssistant`。**`b661d2c` `#126` 删除**。kim-sdk 只 `bot_reply` + `AgentTurn{Done}`，不 `persist_talks` |
| Push skip | 代发 `chat.bot.reply` 的 Mac 连接被 skip → 本机无气泡 |
| `AgentTurn` | `link.dart` `default: break`，不画 footer、不写 timeline |
| 用户输入中 | `b_…` 会发 `chat.typing`，但 fanout 找 **bot 的 location 且进了 typer 的房** → bot 无 loc → **Mac/Web 收不到** |
| Agent 忙碌 | 协议 `chat.bot.typing`、FFI `botTyping` 仍在。**`#126` 后 kim-sdk / Dart features 都不再调用**。心跳死了 |
| 工具卡 / 权限 | 仍本机（bot-KD 12）。本切片 v1 **不上** 工具卡多端，只上活动态 |

---

## Goals & Non-Goals

### Goals

1. 人↔Agent 已成功消息：owner 所有设备最终有 **同一 `message_id`**（含自己发出的）。
2. 助手句在所有设备 `sender=bot`，不是 owner。
3. 用户在任一端输入中：其它端该 1:1 出现与人聊相同的 `KimTypingRow`。
4. Agent 从入队到 `bot_reply` 成功/失败：其它端同一 footer 显示忙碌（可带 phase 文案）；本机不依赖被 skip 的 Push。
5. 人↔人输入中 + 等化条动画在壳重构后有 **自动化回归**（测失败当缺陷修，不靠「应该还行」）。
6. 无双轨：禁止 AgentTurn 当正文、禁止 Dart `_appendLocal` 助手句、禁止人↔Agent 专用会话库。

### Non-Goals

- Agent↔Agent。
- Bot 登录 / Location。
- 工具确认卡、工具气泡跨设备（另切片）。
- 群 `@mention` Agent。
- 两桌面 runtime 租约。
- 改 ACK 模型编号；B0 pending receipt 运维翻转仍按原门闩，本切片只改 **谁进入 receipt / 离线集合**。

---

## Key Decisions

1. **投递按账号，不按「谁握着 TCP」。** Skip Push = 该 location **已经 persist 了这个 `message_id`**（本机 outbox/`mark_sent`/bot.reply 落盘）。禁止「这条连接发的包就 skip」作为 bot.reply 的唯一可见性来源。
2. **`chat.bot.reply` 成功必须立刻写入 kim-sdk timeline**（恢复 `_commitServerAssistant` 等价物：`sender=bot`，`dest=peer`，`message_id`/`send_time` 用 TalkResp）。与 `mark_sent` 同一所有权层，不在 Dart。
3. **离线/receipt 覆盖本账号线程的收+发。** 人聊第二台设备也要能看到自己发出的话。`online_targets` 对人↔Agent 至少包含 **owner 其它 jti**；不能只写 bot（无 loc）。
4. **活动态与正文分离，但同一 fanout 哲学。** 正文落库；输入中/忙碌 **不落库**、短 TTL。人↔人继续房间兴趣（隐私）。人↔Agent（双方 loc 都在 owner 上）fanout = **owner 全部 location，跳过已本地点亮的那一端**。
5. **复用 `CMD_TYPING` Push。** 用户输入：`chat.typing`，`typer=人`。Agent 忙碌：`chat.bot.typing`，`typer=bot`（已有，S-KD 26）。客户端仍一个 decoder、一个 `typingProvider`、一个 `KimTypingRow`。
6. **Agent 内部阶段用 TypingPush 扩展，不新开会话 command。** proto 增加可选 `phase`（0=composing 缺省，1=queued，2=running，3=tool，4=wait_permission）。旧客户端忽略未知字段，仍画 bars。v1 UI：bars + 可选一行文案；不把 phase 写成 timeline 气泡。
7. **本机先亮活动态。** 发 `typing`/`bot.typing` 的设备 `applyLocal(typer, dest, active, phase)`，不靠回显。与人聊「自己看不到自己的输入中」不同：Agent 忙碌必须在跑 Goose 的 Mac 上也有 footer（对标「对端正在输入」）。
8. **`b_…` 与真人 1:1 一样进房。** HEAD 已进房；保持。本地 `goose`/`agent:` 仍本地、不进房。
9. **人↔人 typing 回归是本切片门闩，不是顺手。** 壳重构后 `_enterRoom` 静默吞错、footer 只绑 `peerTypingProvider`：必须有 widget/集成测证明「对端 Push → 等化条在动」。
10. **`AgentTurn` 只留给桌面调试/权限，不进产品气泡。** Running/Done 驱动：本机 activity + `bot_typing`；Done 另走 persist 助手句。Dart 继续忽略 AgentTurn 正文。

---

## Design

### A. 消息（人↔Agent = 人↔人）

```
enqueue / bot_reply
  → 服务端 insert（content + 双方 index）
  → Resp(message_id, send_time)
  → 本机 persist（用户句 mark_sent；助手句 apply_talk sender=bot）
  → Push 给尚未持有该 id 的参与账号 location
重连
  → 未 ACK 的 message_id（收+发）→ persist-then-ack
```

**bot.reply 本机落盘（P0）** — `crates/kim-sdk/src/agent/mod.rs` 在 `bot_reply` Ok 后构造 `IncomingTalk { command: chat.user.talk, dest, sender: dest(bot), body, message_id, send_time }` 调 `persist_talks_for(..., UnreadPolicy::Keep)`（本机已在看，不涨未读）。失败：`AgentTurn::Error` + 可重试，**禁止**把正文只放在 event 里。

**多端用户句（P1）**

- Live：保持 KD 9（recipients 含 sender）。核对 `sender==me` 时 dest=`Header.dest`（已有）。加测试：alice 两 location，Mac talk→bot，phone persist 到 dest=bot。
- 离线：`offline_index` 对私聊同时返回该 account 的 `direction=0 和 1`（或 pending receipt 给 owner 其它 jti）。人↔人也受益。
- `do_user_talk` 的 `online_targets`：dest 为 bot 时改为 owner 的其它 location（与 `do_bot_reply` 对称），不要只 `fallback_targets(&[receiver])`。

**Skip 规则（P2，可与 P0 同 PR 若测试能锁）**  
`dispatch` 仍可跳过 **当前 channel**（避免重复），但发送端必须已经 persist。契约测试：skip 存在时发送端 store 已有该 `message_id`。

### B. 活动态（输入中 + Agent 执行）

**人↔人（不变语义，补回归）**

```
composer debounce → chat.typing { dest=peer, active }
server：好友 + 进了 dest=typer 的 peer 设备
peer：TypingUpdated → typingProvider[typer] → KimTypingRow
空闲 2.5s / 发送 / 离开 → active=false
客户端 20s TTL 仍保留
```

核对清单（必须写测）：

1. 打开真人 1:1 调用 `roomEnter`；失败要打日志，不能只 `catch`。
2. `onTypingChanged` 在 `userThread` 为真时接通（HEAD 已接）。
3. FFI/SDK 把 `chat.typing` Push 映成 `SessionUpdate::Typing`（HEAD `lib.rs` 已映）。
4. `applyPush` 用 **typer** 当 thread key，与 `peerTypingProvider(widget.id)` 一致（对端账号）。
5. `footer: KimTypingRow` 且 `KimTypingBars` 在 `active==true` 时 `AnimationController.repeat`（已有 widget 测，补「provider true → 找到 `Key('typing-row')`」）。

**人↔Agent 用户输入中**

- composer：`b_…` 已 `sendTyping`。保持。
- **改 server `do_typing`**：若 dest 的 `kind=bot` 且 `owner==typer`，fanout = owner 的 **其它** location（与 `do_bot_typing` 相同），**不**查 bot loc、不要求 bot 进房。
- 真人 dest 仍走房间兴趣（HA-KD 4）。
- 接收端：`typer=自己` 但来自其它设备 → thread key 用 **dest（bot）** 而不是 typer，否则 `peerTypingProvider(b_bot)` 对不上。  
  规则：`threadId = (typer == me) ? dest : typer`。人↔人 `typer != me`，行为不变。

**Agent 执行（上云）**

kim-sdk `MobileAgent` 状态机：

| 内部 | `bot_typing` | phase | 本机 footer |
|---|---|---|---|
| `enqueue_turn` 入队 | active=true | queued | 亮 |
| `run_turn` 开始 | heartbeat ~2s | running | 亮 |
| 工具进行中（runtime 能区分则） | heartbeat | tool | 亮 + 文案 |
| `WaitingPermission` | heartbeat | wait_permission | 亮 + 文案 |
| `bot_reply` 成功 / Error / 队列空 | active=false | — | 灭 |

Heartbeat 失败只 warn，不阻断模型。Mac 在发 RPC 前 `applyLocal(typer=bot, dest=bot, active)`，phone 靠 Push。

`do_bot_typing`：已向 owner 全 loc 发、skip 本 channel。保持。补：本机 applyLocal。

v1 **不**把 phase 写成历史消息。权限卡仍本机（Non-Goal）。

### C. UI 槽位

- 气泡：只 `watchThread` / timeline。
- Footer：只 `peerTypingProvider(dest)` → `KimTypingRow`（人在输入 = 对端头像；Agent 忙碌 = bot 头像）。可选 subtitle 读 phase。
- Inbox：busy 时可用同一 provider 在 tile 上显示「…」，本切片可选，不挡 P0/P1。

---

## Phased Implementation

每阶段可发布：编译、测试、无双写、无「只发不回」空窗。

### PR 1 — 人↔人 typing 回归 + 进房失败可见

- `_enterRoom` 失败 `tracing`/`WLogger` 等价日志，不再空 catch。
- 测试：`typing_receipts_test` 保留；新增 widget：`typingProvider.applyPush` → `ChatList` footer `Key('typing-row')` 且 bars 在 animating。
- FakeKim：`sendTyping` / 注入 Typing Push。
- 验收：真人 1:1 一端输入，另一端（或 Fake Push）出现等化条。**本 PR 不改协议。**

### PR 2 — 助手句本机落盘（修 Mac 无回包）

- kim-sdk `bot_reply` 成功 → `persist_talks`。
- 单测：`agent_port` 在 fake proto 返回 id 后 store 有 `sender=bot` 行。
- Dart 仍忽略 AgentTurn 正文。
- 验收：仅桌面、无第二设备，发一句有用户气泡 + 助手气泡。

### PR 3 — 活动态上云（用户输入中 + Agent 忙碌）

- `do_typing` bot-owner 分支；`threadId` 规则；kim-sdk 调 `bot_typing` + heartbeat + applyLocal。
- proto 可选 `phase`（可同 PR 或紧随；缺省=composing）。
- 验收：Phone 在 `b_…` 输入，Mac 该会话 footer；Mac 跑 Goose，Phone footer 亮，回复落地后灭。

### PR 4 — 多端正文副本（修手机缺触发句 + 人聊自己消息）

- 离线含 direction=1 或 owner 其它 jti 的 receipt。
- `do_user_talk` dest=bot 时 `online_targets` 含 owner 其它 loc。
- 测试：双 location，一端 talk，另一端仅靠 sync（无 live）也能拉到自己的句。
- 验收：Mac 发、Phone 先离线再上线，线程里用户句+助手句都在。

### PR 5 — skip 与文档

- 契约测：skip 本 channel 当且仅当本端已有 id。
- 回写 reliable-delivery / agent-goose / presence 文档；从 impl README 移入已合入；删本文件。

PR 2 与 PR 3 可并行（落盘 vs 活动）。PR 4 不要晚于活动态太久，否则「有正在输入、没有那句话」仍成立。

---

## 验收不变量

1. 人↔人：输入中仍出现 **动画中的** `KimTypingRow`；发消息后 bars 消失。
2. 人↔Agent：owner 每台设备，成功消息同一 `message_id`；助手句 `sender=bot`。
3. 用户在 Phone 输入、Mac 打开同一 `b_…`：Mac 有 footer；反之 Agent 忙碌 Phone 有 footer。
4. 重连不丢自己发出的 1:1（人与 Agent）。
5. 无 Goose 的 Phone 不跑模型，仍能看完整会话和忙碌态。
6. 禁止清库/重装当修复。

---

## 文件清单（预期）

| 路径 | PR | 动作 |
|---|---|---|
| `crates/kim-sdk/src/agent/mod.rs` | 2, 3 | persist 助手句；`bot_typing` 心跳 |
| `crates/kim-sdk/src/store/messages.rs` | 2 | 本机 Keep 未读 |
| `crates/kim-sdk/tests/agent_port.rs` | 2, 3 | 落盘 + typing 调用 |
| `services/chat/src/typing.rs` | 3 | dest=owner 的 bot 走 owner 其它 loc |
| `services/chat/src/talk.rs` | 4 | bot dest 的 `online_targets` |
| `services/chat/src/store/mod.rs` + postgres 离线 | 4 | 私聊 index 含 direction=1 |
| `services/chat/src/bot.rs` | 3 | 保持 bot.typing fanout；测 skip+本机 |
| `crates/kim-protocol/proto/pkt.proto` | 3 | TypingPush/Req 可选 phase |
| `sdk/mobile/lib/features/session/typing.dart` | 3 | `threadId = typer==me ? dest : typer`；phase |
| `sdk/mobile/lib/features/chats/chat_session.dart` | 1, 3 | 进房失败可见；`b_…` 保持 sendTyping |
| `sdk/mobile/lib/features/chats/chat_page.dart` | 1 | footer 回归测；可选 phase 文案 |
| `sdk/mobile/lib/features/session/link.dart` | — | Typing 分支保留；仍不把 AgentTurn 当正文 |
| `sdk/mobile/test/widgets/*typing*` | 1 | Push → 动画 footer |
| `services/chat/tests/e2e_bot.rs` | 3, 4 | typing owner 多 loc；sync 含自己的句 |
| `docs/reliable-delivery.md` `docs/agent-goose.md` `docs/presence-room-interest.md` | 5 | 形状 |

---

## 测试要点

- 人↔人 typing：enter 后 Push → footer 动画；leave 后不再收；20s TTL。
- 人↔人 自己消息：第二 loc 仅 sync 能看到 sent。
- bot.reply：发送端无 Push 仍有 timeline 行。
- 用户 typing dest=bot：第二 loc 收到 `typer=owner`，UI 键为 bot dest。
- bot.typing：phone 收到 `typer=bot`；Mac 本机亮；reply 后双方灭。
- 非 owner 对 bot typing / talk 仍 108（不泄露）。
- phase 缺省：旧客户端只画 bars。
