# 控制层：在线单聊与群聊（已落地）

对照小册第 19 章。登录 / 会话 / 互踢仍以 [link-layer-login.md](link-layer-login.md) 为准。本文只记 **现在代码里的** 在线 talk：`chat.user.talk`、`chat.group.talk`、最小 `chat.group.create`。

网关对 `chat.*` 仍然 `forward(service_name())`。`service_name("chat.user.talk")` 已是 `"chat"`。**不要**把 `if talk` 写进 `TcpServer` / `WsServer`。

---

## 进程

```text
pkt-client  --ws://127.0.0.1:8001-->  gateway (WsServer)
                                         │  TCP InnerHandshakeReq
                                         ▼
                                   chat (TcpServer :8002)
                                   login.* / chat.demo.echo / chat.user.talk
                                   / chat.group.talk / chat.group.create
                                   同一进程、同一 Router
```

必须先起 Chat，再起网关（StaticNaming 失败不重拨）。

---

## 指令

| command | Flag | 谁处理 |
|---|---|---|
| `chat.user.talk` | Request | Chat `do_user_talk` |
| `chat.user.talk` | Push | 网关按 `dest.channels` 推给接收方 |
| `chat.group.talk` | Request | Chat `do_group_talk` |
| `chat.group.talk` | Push | 同上，合包、跳过发送方 |
| `chat.group.create` | Request | Chat `do_group_create` |
| `chat.group.create` | Push | `GroupCreateNotify` 给在线成员 |
| `chat.group.join` / `quit` / `detail` / `members` | Request | 见 [group-royal.md](group-royal.md) |
| `chat.talk.ack` | Request | Chat `do_talk_ack`（见 [reliable-delivery.md](reliable-delivery.md)） |
| `chat.offline.index` | Request | Chat `do_offline_index` |
| `chat.offline.content` | Request | Chat `do_offline_content` |

Push **同 command**、`Flag=Push`。接收方 Header 没有发送者账号；`sender` 只在 `MessagePush` body 里。建群会发 `GroupCreateNotify`。

| Status | 值 | 何时 |
|---|---|---|
| Success | 0 | talk / create 走完（离线接收方也是 Success） |
| InvalidPacket | 1 | 不变 |
| CommandNotFound | 2 | 不变 |
| ServiceUnavailable | 3 | 不变 |
| SystemException | 99 | insert / members / 非 NotFound 的寻址 / dispatch 失败 |
| InvalidPacketBody | 101 | MessageReq / GroupCreateReq 解不开 |
| ContentBlocked | 106 | talk `ContentFilter` 拒绝（文本词表 / 图片 URL 等）。不 insert、不 Push。不在 SDK 重试区间 |
| NotGroupMember | 107 | 群聊发送方不在成员列表（含未知群）。不 insert、不 Push。不在 SDK 重试区间 |
| UserNotFound | 108 | `chat.user.talk` dest 不是用户表里的账号。不 insert、不 Push。不在 SDK 重试区间 |
| NoDestination | 300 | `Header.dest` 为空；不 decode、不 insert |
| SessionNotFound | 404 | 非 signin 且 cache miss（到不了 Handler） |

已落地 **0/1/2/3/99 不改号**。`NoDestination=300` 是新增。`106` / `107` 在 1xx，SDK 不会重试、也不会当 4xx 关连接。

---

## dest

| 指令 | `Header.dest` |
|---|---|
| `chat.user.talk` | 对方 **账号**（必须已注册或登录过；否则 `UserNotFound`） |
| `chat.group.talk` | **group id**（`GroupCreateResp.groupId`） |
| `chat.group.create` | 空（不寻址） |
| `chat.group.join` / `quit` / `detail` / `members` | **group id** |

本里程碑 group id 是雪花 i64 的 **base36** 字符串，这就是长连接 dest。第 22 章 Royal HTTP 的 Base32 是 REST 主键，**不要**把 dest 改成 Base32。

对自己发（dest = 本 session.account）：insert + Success，`dispatch` 跳过自身 channel，无 Push。

`MessageReq.clientId` 非空时按 `(app, sender, client_id)` 去重：命中则返回第一次的 `messageId` / `sendTime`，不再 insert、不再 Push。空 clientId 不去重。SDK 一次 `talkToUser` 生成 UUID，3xx 重试复用。

---

## messageId / sendTime

- `messageId`：Chat 侧雪花（`KIM_SNOWFLAKE_NODE` > `config.toml snowflake_node` > `1`）。测试里 `message_id > 10000`。
- `sendTime`：服务端 UnixNano。测试里 `send_time > 1000`。禁止读客户端字段。
- Resp 与 Push 用同一对值。

---

## 在线不是确定值

`get_location` 命中只表示会话表里还有这条 loc。网关可能已死、心跳未超时、`dispatch` 到达网关后 `push` 失败。因此 Handler **先 insert 再按 loc 决定是否 Push**。未 ACK 的消息可在重连后 Pull，见 [reliable-delivery.md](reliable-delivery.md)。

| 场景 | 本里程碑 |
|---|---|
| 在线且 dispatch 成功 | 对端至多一次 Push；接收方可 ACK |
| 在线但网关 push 失败 | 发送方 `SystemException`；可能部分成员已收到 |
| 离线 | 无 Push；发送方仍 Success + messageId。接收方重连后可 `chat.offline.index` |
| 发送方未收到 Resp | 客户端可重发 → 可能重复消息 |

ACK / 离线见 [reliable-delivery.md](reliable-delivery.md)。未知群：`members` 返回空列表，发送方不在其中 → `NotGroupMember`，不 insert。

Talk 在 insert 之前跑 `ContentFilter` 链（默认 ChatHandler 是 `NoopFilter`；进程用 `builtin_talk_filter`：文本拦截 + 图片拦截，词表来自 `config.toml` 的 `sensitive_words` / `blocked_image`）。只拦对应 `MessageReq.type`；语音 / 视频以后加新 impl。命中 → `ContentBlocked`。

---

## pkt-client 环境变量（命中即停）

默认路径 **不变**：JWT → ping → `chat.demo.echo` seq=2 → close。

| 优先级 | 条件 | 线性序列 |
|---|---|---|
| 1 | `KIM_HOLD=1` 且 `KIM_TALK_TO`、`KIM_GROUP_MEMBERS` 都空 | **不 ping**。读循环：Kickout（`login.signin` 且 `Flag=Push`）→ 校验 `channel_id` → close。`Flag=Push` 且 command 为 talk → 打 `message_id`/`sender`/`msg_type`/`body_len`（**不打 body**），继续。其它包忽略 |
| 2 | `KIM_GROUP_MEMBERS` 非空（忽略 `TALK_TO`、`PING_ONLY`） | ping → pong。seq=2 `chat.group.create`（`name=demo`，`owner=argv id`，members 逗号 split+trim）。seq=3 `chat.group.talk` dest=`group_id`，body=`KIM_TALK_BODY` 或 `hellogroup`。然后 HOLD 则同行 1 读循环，否则 close |
| 3 | `KIM_TALK_TO` 非空（忽略 `PING_ONLY`） | ping → pong。seq=2 `chat.user.talk` dest=`KIM_TALK_TO`，body=`KIM_TALK_BODY` 或 `hello world`。然后 HOLD 或 close |
| 4 | `KIM_PING_ONLY=1` | ping → pong → close |
| 5 | 默认 | ping → pong → echo seq=2 → close |

登录后若 `KIM_SYNC_OFFLINE=1`，先拉离线再走上表。HOLD 收到 talk Push 默认会 ACK，见 [reliable-delivery.md](reliable-delivery.md)。

---

## 本机怎么跑

必须先 Chat 再网关，见根目录 [README.md](../README.md)。

```bash
# 接收方（路径 1：不 ping，等 Push 或 Kickout）
KIM_HOLD=1 RUST_LOG=info cargo run -p pkt-client -- bob

# 发送方 1:1（路径 3）
KIM_TALK_TO=bob RUST_LOG=info cargo run -p pkt-client -- alice

# 建群并群聊（路径 2）
KIM_GROUP_MEMBERS=alice,bob,carol RUST_LOG=info cargo run -p pkt-client -- alice
```

e2e：`services/chat/tests/e2e_talk.rs`（登录回归仍是 `e2e_login.rs`）。

---

## 非目标（不要写进这一层）

聊天逻辑禁止进入 `kim-tcp` / `kim-ws` / `kim-core`。词表 / 图片 URL 拦截只通过 `ContentFilter` impl，不要写进 `WsServer`。ACK / 离线见 [reliable-delivery.md](reliable-delivery.md)。群 join 与 Royal 见 [group-royal.md](group-royal.md)。Web SDK 见 [web-sdk.md](web-sdk.md)。
