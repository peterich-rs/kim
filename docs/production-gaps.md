# 生产缺口与代码对照（2026-08-31）

对照当前仓库代码整理。不是已落地设计。文档和代码打架时以代码为准。

来源：

1. 产品/正确性演进报告（多设备、离线、Royal、群、推送）
2. 开源库与运行时硬化报告（tokio / redis-rs / sqlx / rustls / Channel 隔离）
3. 按热路径核对源码后的修订
4. 对本文方案的审查（跨 app 在线投递已用冻结 `app=kim` 关闭、控制面 Consul/Redis、自聊唯一约束、H1 顺序、停机顺序等）
5. 复核：ACK 不得用 Snowflake 当无洞游标、duplicate 从落库重建、TGateway 组合 trait、outbox 成员快照

行号以整理日工作区为准，之后提交可能漂移，以标识符为准。

阅读顺序：先看「结论」和「建议修复顺序」，再按 P0 → P1 → P2 翻条目，最后看「运行时与开源库」。已落地行为见 [reliable-delivery.md](reliable-delivery.md)、[control-layer-chat.md](control-layer-chat.md)、[group-royal.md](group-royal.md)、[gray.md](gray.md)、[perf.md](perf.md)。

---

## 结论

骨架分层是干净的：`Conn` 换传输不改业务；长连接、分帧、读写分离、JWT 登录/续期/吊销、互踢、单聊/群聊写扩散、ACK、离线 Pull、会话列表/历史、好友与黑名单、群 CRUD、R2 图、Consul + 灰度分区、Prometheus、Web/Flutter 客户端都在。

不能当生产 IM 用的原因不是「缺撤回」，也不是「没用 io_uring」，而是：

1. **读索引代码已换成 pending receipt，默认门闩仍关**（G-03 / G-04 / G-10）。compose `KIM_PENDING_RECEIPT=0` 时仍走 Redis 高水位。关上这三条要走完 [reliable-delivery.md](reliable-delivery.md) rollout，不是镜像合入。
2. **漏 Push 仍无可靠补偿**（G-03 / G-14）。发送方 Success 不保证对端收到；已落库不再回 99。
3. **内部控制面已关**：Chat `/internal/kick` 与 Royal 同 HMAC；nonce Redis NX EX 121；生产 Redis `requirepass` + `noeviction`；Consul 关明文 8500、HTTPS/mTLS 8501、ACL deny、每服务最小权限 token。demo/`change-me` JWT/HMAC 在 strict 下拒启动。
4. **租户已冻结为 `app=kim`**（[gray.md](gray.md)）。G-05 / G-06 已关。
5. **通信层骨架对**：读循环按 channel 串行 lane、下行 try_send、心跳 Redis 有界宽限已关（G-29 / G-30 / G-31）。TGateway 进程内 TLS、keepalive、连接上限已关（G-34）。停机顺序已关（G-07 / G-32）。reuseport / vectored write 仍延后。

两份外部报告的大方向都对。产品报告若干事实写错；库报告把硬化排在鉴权/读索引前面，且 Phase 0 的 duplicate 语义不能照抄。订正见下文两张表。

---

## 建议修复顺序

不改会丢数据或泄密的排在功能缺口前面。

| 序 | 项 | 条目 |
|---:|---|---|
| 1 | pending receipt rollout：`KIM_REQUIRE_JTI=1` + SCAN 空 jti=0 + Royal writer 先于 Chat reader | G-03, G-04, G-10 |
| 2 | device credential 客户端持久化 + `target_id` 迁 device_id；验证/找回/注销 | G-13, G-20 |
| 3 | Flutter 登录后 sync；Web `isRetryable` 对齐真实错误码 | G-14 |
| 4 | Royal RPC 延迟/错误、pending backlog gauge、告警规则 | G-15 |
| 5 | 群 inbox N+1；物化读开关默认开 | G-17 |
| 6 | 限流 governor、R2 签名 URL、撤回/已读、系统推送 | G-18 起 |

读循环隔离、下行 try_send、心跳 Redis 有界宽限、TGateway TLS / keepalive / 连接上限已关（G-29 / G-30 / G-31 / G-34）。Royal 发现 + 熔断 + 好友短缓存已关（G-16）。Redis/sqlx/Royal 热路径超时已关（G-33 剩余 tower-http / tokio Builder）。vectored write、reuseport、ChannelMap 分片、jemalloc、一致哈希、io_uring 再往后。撤回若做了却不改 R2 生命周期，只是客户端隐藏。

## 实施节奏

1. 本文件只做盘点与优先级，不写逐步补丁。
2. 高优先级按 [impl/](impl/README.md) **一份一切片** 写细化设计（文件、SQL、测试、明确不改什么）。
3. 按该切片执行；合入后从本文件删对应 G-xx，形状写回专题文档。租户已冻结为 `app=kim`（见 [gray.md](gray.md)）。漏 Push 补偿见 G-03 / G-14。

---

## 现状（代码里已有的）

| 能力 | 落点 | 备注 |
|---|---|---|
| TCP / WS、分帧、读写分离 | `kim-tcp` / `kim-ws` / `kim-core::Channel` | WS `read_frame` 非 cancel-safe |
| JWT 登录 / 续期 / 吊销 | gateway Accept、`login.renew` Push、`kim:revoke:{jti}` | 冻结 `app=kim`；续期复用同一 `jti` |
| 互踢 | `exclusive_device`：mobile/ios/android 单端；web/desktop/cli 可并存 | `LoginReq.device` 客户端自报 |
| 单聊 / 群聊写扩散 | `insert_user` / `insert_group` | 群：1 content + N index，单事务 |
| ACK + 离线 Pull | `chat.talk.ack`、`chat.offline.index/content` | 默认高水位；`KIM_PENDING_RECEIPT=1` 后是 per-jti receipt |
| 会话列表 / 历史 / 会话级已读 | `chat.inbox.*`、`chat.history`、`conversation_reads` | inbox 每次全量聚合 |
| 好友全流程 + 黑名单 | `chat.friend.*`、`chat.block.*` | 申请 Push 失败无离线补偿 |
| 群 CRUD | create/join/quit/detail/members | create 强制 owner=session；join 禁用自助；quit/detail/members 须是自己/成员。无角色/邀请 |
| R2 图片 | `sdk/media` Worker | 永久公开 URL |
| Consul + 灰度 zone + 智能路由 | naming、gateway `RouteSelector`、router lookup | account 白名单；zone 空不回退正式池 |
| Prometheus | `kim-metrics` | 有 `kim_dispatch_fail_total`、`kim_heartbeat_revoke_error_total`、`kim_mailbox_full_total`、`kim_send_to_ack_seconds`；handler 白名单 29 条。无 Royal RPC 直方图；无告警规则 |
| Web / Flutter 客户端 | `sdk/web`、`sdk/mobile` + `kim-client` | Flutter supervisor 登录后 sync；G-14 仍开（Web isRetryable + chat_ui leftover） |

协议消息类型常量已有 TEXT=1、IMAGE=2、VOICE=3、VIDEO=4。SDK 与过滤器只用前两个。无 FILE，无类型白名单。

`Header`（`pkt.proto`）字段是 command / channelId / sequence / flag / status / dest / bodyLength / meta。**没有 version。**

## 已关闭

| 条目 | 形状 |
|---|---|
| G-02 Chat `offline.content` 越权 | [reliable-delivery.md](reliable-delivery.md)。越权 id 跳过。直打 Royal 要 HMAC |
| G-08 Chat 群指令鉴权 | [group-royal.md](group-royal.md) |
| G-09 insert 成功仍回 99 | [control-layer-chat.md](control-layer-chat.md)。漏 Push 补偿见 G-03 / G-14 |
| G-01 控制面 HMAC / Redis 密码 / Consul mTLS+ACL | [group-royal.md](group-royal.md)、[deploy.md](deploy.md) |
| G-12 生产拒 demo JWT/HMAC | 并入控制面 strict 启动 |
| G-11 Router JWT 进 URL / 哈希 raw token | [routing.md](routing.md)。`GET /api/lookup` 只接受 Authorization；哈希 `acc`/`jti`，renew 复用 jti 不换桶 |
| G-07 / G-32 SIGTERM 先摘发现再 drain | [deploy.md](deploy.md)。unix SIGTERM+SIGINT；Container 先 Consul deregister，再停 accept、JoinSet 有界 drain、关连接；TcpClient 心跳 abort；Royal/Router HTTP graceful。无「请换网关」Push；未在 K8s 杀进程验证 |
| G-31 心跳 Redis 错误踢全员 | [link-layer-login.md](link-layer-login.md)、[observability.md](observability.md)。仅 `Ok(true)` 立刻关；存储错误连续 3 次（`HEARTBEAT_REVOKE_ERROR_GRACE`）后断开；期内 warn + `kim_heartbeat_revoke_error_total`、不续签 JWT。登录 revoke 仍 fail-closed |
| G-29 读循环与 handler 按 channel 串行隔离 | [communication-layer.md](communication-layer.md)。Binary 入 per `header.channel_id` 串行 lane；Ping/Pong/Close 留读专员；满 in-flight 停读。MessageListener 不再假设与读同任务，同一 channel 仍 FIFO |
| G-30 下行 try_send / 满信箱 Disconnect | [communication-layer.md](communication-layer.md)。`ChannelOpts.write_full`：网关 Disconnect + `kim_mailbox_full_total`；内部链路默认 Block |
| G-34 keepalive / 连接上限 / TGateway TLS | [communication-layer.md](communication-layer.md)、[architecture.md](architecture.md)。`TcpConn<S>` + `FrontendState`；`try_acquire_owned`；进程内 rustls（`tls_cert` 空则明文）。reuseport / vectored write 仍延后 |
| G-16 Royal 发现 + 熔断 + 好友短缓存 | [group-royal.md](group-royal.md)。`RoyalPool`：半开先探测再 RR 健康实例；Consul `find`；5xx 计熔断；Chat 侧 30s 社交缓存；compose `royal-2` |

---

## P0 —— 丢消息或可被打穿

### G-03 全局 ACK 高水位在消息空洞时丢数据（单设备也丢）

代码已落地：`pending_delivery` + 按进程 `KIM_PENDING_RECEIPT`，见 [reliable-delivery.md](reliable-delivery.md)。compose 默认 0，生产仍走 Redis 高水位。未完成 Gateway `KIM_REQUIRE_JTI=1` 持续生效、SCAN `login:loc:v2:*` 空 jti = 0、Royal writer=1 再 Chat reader=1 **之前，不得从本文件删 G-03 / G-04 / G-10**。

**文件**

- `services/chat/src/store/redis_ack.rs` — `key` / `set`（约 19–50 行）：`SET chat:ack:{account}`
- `services/chat/src/store/postgres.rs` — `offline_index`：`send_time > start LIMIT 2000`
- `sdk/web/src/client.ts` — `messageAckLoop`（约 819–840 行）：只 ACK `lastMessage`
- `docs/reliable-delivery.md`：读索引只保存每个账号最新 ACK 的 `messageId`

**问题**

投递是 at-least-once、按网关分发，**不保证 id 连续到达**。客户端把最后一条成功 Push 的 `messageId` 写成全局 ACK。若 id=10 dispatch 失败、id=11 到达并被 ACK，离线查询从 11 的 `send_time` 之后开始，**10 永远不会再被 pull**。

多会话同样：私聊 10、群聊 11，ACK 11 等于宣称私聊 10 已同步。

这与「两台设备抢游标」独立。只把 key 改成 `(account, device_id)` **仍然是高水位**，跨会话和并发提交空洞照样丢（G-04 / G-10）。

**建议**

Snowflake `message_id` **不能**当无洞 ACK 游标。G-03、G-04、G-10 必须选下面之一，禁止 `ack_id = max(message_id)`：

1. **逐消息 receipt / pending-delivery（推荐）**：每条 index 对每个 (account, device) 有未确认集合或投递状态；ACK 是确认具体 id（或一批 id），不是「这个 id 以下都收到了」。
2. **会话内 `delivery_seq`**：按 `(app, account, device, conversation)` **提交时**串行分配，保证 seq 与可见顺序一致。离线拉 `seq > cursor`。seq 不能在 `begin` 前用雪花发号。

在改完之前，`offline.index` 不要用请求里的 `messageId` 去 `ack.set`。

---

### G-04 多设备抢同一个读索引

**文件**

- `services/chat/src/ack.rs` — `do_talk_ack`：`store.ack(app, account, message_id)`，不用 `session.device`
- `services/chat/src/store/redis_ack.rs` — `chat:ack:{account}`
- `services/chat/migrations/0006_user_social_inbox.sql` — `conversation_reads` 主键 `(app, account, peer, group_id)`
- `crates/kim-session/src/lib.rs` — `exclusive_device`（约 24–29 行）
- `crates/kim-router/src/context.rs` — `dispatch` 跳过发送方自己的 `channel_id`（约 101–103 行）

**问题**

web/desktop/cli 本来就可以多端在线；dispatch 会推到发送方其它 channel。缺的是 **per-device 读游标**，不是登录模型。

手机 ACK 100 之后，电脑 `offline.index` 起点也是 100。电脑当时若离线或 Push 没到，那一段永远拉不到。

`conversation_reads` 也是 per-account：手机标已读，电脑未读角标被清掉，历史还在。不要和 ACK 混成一个问题——ACK 丢消息，reads 只搞坏角标。

ACK 是 `SET` 不是 `GREATEST`。电脑若 ACK 了更小的 `lastMessage`，游标回退，和 2000 截断叠在一起分页会乱。

把游标收成 `(account, device_id) -> ack_id` 并取 max，**只是把全局高水位缩到设备上**。同一设备上私聊 10 未到、群聊 11 已 ACK，10 仍丢。雪花 id 在 `insert_fanout` 里于 `tx.begin()` **之前**发放（`postgres.rs` 约 90–93 行）：较小 ID 的事务可以晚于较大 ID 提交；客户端 ACK 较大 ID 后，`message_id > cursor` 会跳过后提交的较小 ID。

**建议**

G-04 只解决「哪台设备的游标」。无洞语义强制选 G-03 的两条之一，**不要**把 Snowflake `message_id` 当 ACK 游标。`device_id` 用 G-13 的稳定凭证，不能绑一次性 `jti`。租户已冻结为 `kim`。会话漫游仍用 `history`。

补测试：两个并发 insert，小 id 事务后提交；设备 ACK 了大 id 之后，小 id 仍能被该设备 pull / 出现在 pending 集合里。

---

### G-10 离线游标类型错、隐藏截断、1 天 fallback

**文件**

- `services/chat/src/store/mod.rs` — `OFFLINE_SYNC_INDEX_COUNT = 2000`、`EXPIRES_NANOS = 15 天`
- `services/chat/src/store/postgres.rs` — `offline_index` / `sent_time`（约 200–277 行）
- `sdk/web/src/client.ts` — `loadOfflineMessage`（约 759–779 行）会按 last messageId 循环

**问题**

原报告「超过 2000 条静默丢窗口」过重。数据仍在 PG。Web SDK 已分页。缺口是协议与游标：

| 行为 | 后果 |
|---|---|
| 单次 LIMIT 2000，无 `has_more` | 不循环的客户端以为同步完成 |
| 请求游标是 `messageId`，过滤是 `send_time > start` | 同时刻/回拨会漏或重 |
| `message_id > 0` 时 `ack.set(请求 id)` | 一次 index 推动全局 ACK（叠 G-03/G-04） |
| content 找不到该 id | 起点变成 **now − 1 天**，不是 15 天 |
| 15 天只在查询里 clamp，无 GC | 超窗表现为静默不可见 |
| 若改成 `message_id > cursor` | **仍无洞保证**：id 在 `begin` 前发号，小 id 可晚提交（G-03/G-04） |

**建议**

分页协议：返回 `has_more` + 稳定 `next`（receipt 集合的续拉，或 `delivery_seq`，**不是** Snowflake `message_id`）。ACK 与 index 请求解耦。找不到 id 时用 15 天或明确错误，不要 1 天。无洞语义见 G-03，不要在 G-10 另发明一套 `id > cursor`。

---

### G-13 服务端已能签发可选 device credential；客户端未持久化；`target_id` 仍是 `jti`

**文件**

- `crates/kim-protocol/proto/pkt.proto` — `AuthReq` / `AuthResp` / `LoginReq` 可选凭证字段
- `services/royal/src/device.rs` / `auth.rs` — enroll / 出示；仅成功时 JWT 写 `did`
- `services/gateway/src/lib.rs` — 有 `did` 或 `deviceCredential` 才校验 `kim:device:{did}`
- `crates/kim-session/src/lib.rs` — `exclusive_device` 仍只看 `LoginReq.device`

**已落地（B2 服务端半边）**

可选凭证独立于会话 `jti`。旧客户端只编 field 1–2 → JWT 无 `did` → 今日互踢与今日全端登出。`kim:device:{did}` 热 key 写入失败则 login/register 失败，不签发带 `did` 的 token。吊销该 `did` 后带 `did` 的 Accept 失败；**无 `did` 的会话仍能登**。

**仍开**

- 客户端未持久化 credential（重装后接不上同一设备）
- `pending_delivery.target_id` 仍是 `jti`，不迁 `device_id`
- logout 仍 `kick_account` 全端踢（避免未出示凭证的 web 登出不再踢手机）
- `LoginReq.device` 仍是可伪造的互踢提示，无 Attestation

---

## P1 —— 常见路径错误或扩大爆炸半径

### G-14 客户端补偿对不上服务端语义

**文件**

- `sdk/web/src/status.ts` — `isRetryable`：仅 300–399
- `sdk/web/src/client.ts` — 登录后 `loadOfflineMessage`；`talk()` 复用 `clientId`
- `crates/kim-client/src/client.rs` — `send_message` 的 `client_id` 由调用方持有；`talk_to_user` 仍是一次性 UUID 薄包装
- `sdk/mobile` — FFI supervisor + Dart outbox 已接线；聊天页仍用 flutter_chat_ui，未自研 ChatList

**问题**

| 能力 | Web | kim-client / Flutter |
|---|---|---|
| 登录后 offline.index 循环 | 有 | Flutter：supervisor SyncEngine + persist-then-ack；Web：有 |
| clientId 跨重试稳定 | 同一次 `talk()` 内稳定 | Flutter outbox 持稳定 id（文本发送）；Web `talk()` 内稳定。聊天页 flutter_chat_ui 仍在，G-14 不算关 |
| 99 / 3 当可重试 | 否 | 否 |
| 默认 device | 调用方传 | `mobile`（互踢） |

没有系统推送时，移动端 = 在线 Push 或什么都没有。叠 G-03 / G-14。

本地 `KeyValueStore.lastId` 与服务器 ACK 是两套。清 localStorage 会从 0 再拉；另一台设备的服务器 ACK 会让这台的本地游标显得落后。

**建议**

Flutter 登录后走与 Web 相同的 sync。服务端已落库不再回 99；`isRetryable` 仍可不覆盖 99。权威游标只放服务端。

---

### G-15 可观测性不够定位「偶发延迟」

**已落地**

- `COMMANDS` 29 条（含 friend / inbox / history / user / block）
- `kim_send_to_ack_seconds`：ACK 路径直方图，不是 pending 表 AVG

**仍开**

- `deploy/prometheus.yml` 只有 scrape，无 `rule_files`
- 无 Royal RPC 延迟/错误率（path_group）
- `pending_delivery` backlog / oldest-age 只打日志，未接 gauge
- 无 OpenTelemetry，无跨进程 trace id

e2e 覆盖面不小（`services/chat/tests/` 12 个文件量级），主路径偏 happy path。故障注入（Royal 5xx、Redis 断、网关 kill 后消息不丢）缺。

---

### G-17 inbox 全量聚合 + N+1

**已落地**

- `do_inbox_list` 对私聊 dest 一次 `users.profiles`（不再逐行 `profile`）
- `conversation_inbox` 双写（unread 累加；last 按 `(send_time, id)`）
- 读路径可切 `KIM_INBOX_MATERIALIZED`（compose / Royal 进程读取；**默认 0**，仍走 `message_index` GROUP BY）

**仍开**

- 群会话仍逐行 `groups.detail`
- 默认读仍是全量 GROUP BY；物化回填终态与默认开关于 Royal
- Memory：`indexes: Vec<InboxRow>` 全表扫，一把 `RwLock<Inner>`

inbox 最多 100、无游标。历史已有 `before_id`。

---

### G-18 无速率限制

登录、发消息、加好友、群 join、查找用户均无限流。`ContentFilter` 只做内容子串。可对注册（Argon2 更易被打成 CPU）、好友申请、群 join 做洪水。

---

### G-19 内容审核挡在写路径且可绕过

**文件**

- `services/chat/src/filter.rs` — `TextWordFilter` / `ImageFilter`：大小写敏感 substring；非 TEXT/IMAGE 直接 `Ok(())`

VOICE/VIDEO 以及任意 `type=99` 不审。无 message type 白名单。talk 层无比帧更小的 body cap（TCP `MAX_PAYLOAD` / WS 1MB）。生产要先发后审或至少不阻塞 P99，且补白名单。

---

### G-20 账号能力不完整（无验证 / 找回 / 注销）

Royal 有 register / login / logout / `POST /api/v1/auth/password`（Argon2）。**改密吊销旧会话已落地（B1）**：`live_claims`（签名 + `app` + jti revoke + `ver >= token_epoch`）供 `/me` 与改密共用；改密一条语句写新哈希并 `users.token_epoch += 1`，再写 Redis `kim:token_epoch:{account}`、revoke 当前 jti、`kick_account`。`/me` 与 `/internal/token-epoch` 读 `max(缓存, 用户表)`，缓存 miss 不以 0 放行。改密 **不发新 token**，调用方必须重新 login。`users.upsert` 仍可创建无密码用户（epoch 保持 0）；无密码账号改密 401。

**仍开**

无邮箱/手机验证、无找回、无 2FA、无注销。原报告「无账号体系」过重。

---

### G-21 R2 对象永久公开、无生命周期

**文件**

- `sdk/media/src/index.ts` — 校验 JWT 后 `BUCKET.put` + `publicUrl`
- `sdk/media/src/object.ts` — key `{account}/{yyyy}/{mm}/{uuid}.ext`

无签名 URL、无过期、无删除、无引用计数。消息 body 存公开 URL。以后做撤回也撤不掉对象。仓库内 **没有** listing API；「账号可枚举」只在 R2/CDN 打开公开列举时成立，是条件性运维风险，不是代码路径。WS 网关不查 Origin；media worker 有 Origin 白名单，两边不一致。

---

### G-22 Snowflake 碰撞面

**已落地（B5 半边）**

生产 Chat / Royal `SnowflakeGen::try_new` 失败即退出，不再降 `SequenceIdGen`。`resolve_snowflake_node` 对 `>31` 返回 `Err`（不回退 1）。compose：royal node=10，`royal-2` `KIM_SNOWFLAKE_NODE=11`。测试仍显式 `SequenceIdGen`。

**仍开**

机器位 5 bit（32 节点）。`data_center_id` 仍写死 0。Chat 走 Royal 时本进程仍创建 idgen（未用于 insert）。发号仍分散在 Chat Memory 路径与 Royal。

---

### G-23 幂等表不回收；Memory 幂等有竞态窗口

`message_idempotency` 无 TTL。Memory 先读锁查、再写锁插入，中间可双发。Postgres 用 `ON CONFLICT DO NOTHING` 再回读，这条是对的。

Web 同一次 `talk()` 内 `clientId` 稳定；`kim-client` 每次新 UUID，上层重试会双发。

---

### G-24 Redis 主挂则读路径雪崩；mirror 只双写

**文件**

- `crates/kim-session/src/dual.rs` — 读 primary；mirror 失败只 warn
- `deploy/compose.yml` — Redis 64MB、`volatile-lru`

主 Redis 挂：session 查不到 → `SessionNotFound`。gateway 心跳吊销查询存储错误有界宽限（连续 3 次后断开）；登录 revoke 仍 fail-closed。

`volatile-lru` 会先踢带 TTL 的 ACK（30 天）和 session（48h），表现为有人突然从 15 天窗口重拉。

心跳 `touch_session` 会续 `login:sn` 和 `login:loc` 的 EXPIRE，不会让 loc **cache** 过期。

---

### G-25 群写扩散单事务；空成员幽灵行

500 人群 = 500 行单事务。Memory 持全局写锁。

`hash_partition.sql` 写 `PARTITION BY HASH (account)`，列名是 `account_a`，sketch 不能用，且不被 migrate 应用。

空 members 仍写 content、不写 index（单测明确允许）。insert 要 HMAC，不能再裸打。

大群中期要分批提交 + 幂等续传，或切读扩散。

---

### G-26 协议无版本；错误无 trace

`Header` 无 version 字段。原报告「现在恒 0」不成立，**现在没有这个字段**。现在加成本低。

`resp_with_error` 明确不把 `err` Display 发给客户端（ABI 稳定）。调试应在 meta 里带 optional trace-id，而不是把错误字符串写进 body。

---

### G-33 已接入库用得浅：Redis / SQLx / Axum / Tokio

**热路径已落地（B4）**

- redis-rs：六个 `ConnectionManager` 共用 `open_connection_manager`（连接/响应 3s）；多账号 `get_locations` 是 pipeline `HVALS`，失败逐个回退
- sqlx：业务池 `statement_timeout=5s`；migrate 走独立连接；`min_connections` / `max_lifetime` 可配
- reqwest：Royal 重试带退避；私聊目录 RPC 总预算 800ms

**仍开**

| 库 | 缺口 |
|---|---|
| axum | Royal/Router 仍裸路由；无 `tower-http` Trace / Timeout / BodyLimit |
| tokio | 仍 `#[tokio::main]` 默认；无显式 worker / blocking 池 |
| redis-rs | Cluster 作 feature，不必换 fred |
| sqlx | 无 pool 指标；`.sqlx` 离线数据是子任务 |
| prometheus | command 白名单已 29 条；Royal RPC / 告警见 G-15 |

Dual-write 默认仍 fail-open，镜像失败打 `kim_session_mirror_fail_total`。

---

## P2 —— 规模化与产品完整性

### G-27 原报告 P1 产品缺口（代码确认仍缺）

| 项 | 现状 |
|---|---|
| 撤回 | 无 command、无时限状态机、无对端 Push |
| 已读回执 | 只有自己的 `conversation_reads`，发送方不可见 |
| 系统推送 APNs/FCM/Web Push | 无 |
| 语音/视频/文件消息 | 常量有 VOICE/VIDEO；无 FILE；SDK 不渲染 |
| 在线状态 / 正在输入 | 无；session 可做 presence，要注意好友广播风暴 |
| 转发 / 引用回复 / 本地删除 | 无 |
| 消息搜索 / E2EE / 消息编辑 | 无 |
| 大群读扩散混合 | 仍是纯写扩散 |

### G-28 其它边角

- 自聊跳过好友/黑名单，index 插 SEND+RECV 两行（同一 account），inbox 可能重复。
- 好友申请 Push 失败只打日志；离线方靠 `incoming` 主动拉。
- `kim-ws` Upgrade 不查 Origin。
- 网关连接上限已落地（G-34）；默认不限，tgateway 可配 `max_connections`。
- inbox/history 的 Royal HTTP 要 HMAC；Chat `/internal/kick` 同样要 HMAC。
- `do_user_profile` 可查任意账号资料（文档按搜索产品写的，叠加无限流）。
- Chat `db_max_connections` 默认 5，inbox 重查询容易把池打满。
- metrics HTTP 与 `/internal/kick` 同绑定；容器内网可达。

---

## 对原报告的订正

这些句子不要再写进计划。

| 原句 | 代码事实 |
|---|---|
| gateway 的 main 没有 ctrl_c；Container shutdown 无人触发完整链路 | 已关：unix SIGTERM+SIGINT；先 deregister 再 drain。见 [deploy.md](deploy.md) |
| LogicPkt 无版本号（现在恒 0） | `Header` **没有** version 字段 |
| 只有 MESSAGE_TYPE_TEXT/IMAGE | 协议已有 VOICE=3、VIDEO=4；SDK/过滤未接；无 FILE；无白名单 |
| 超过 2000 条静默丢窗口 | 库内仍在。Web 会连拉。丢数据发生在 send_time 游标、1 天 fallback、全局 ACK 高水位 |
| 一次 talk 串 5 次 HTTP 到 Royal | 私聊 3 次目录 RPC + Redis locations + 1 次 insert。`get_locations` 不是 Royal |
| insert_group members 为空会再查一次（每次群聊） | Chat talk 先取 members 再 POST。fallback 在 Royal handler，给直接 HTTP 用 |
| e2e 都是 happy path | 大体对；另外 talk 单元测试把错误 dispatch 语义锁成契约 |
| Push 只发给当时在线的设备 | 接收方成立。发送方其它在线 channel 会收到（dispatch 跳过自己） |
| 无账号体系 | 有账号密码、改密、JWT。缺的是验证、找回、2FA、注销 |
| 读索引 per-account，两台设备会丢 | 成立，且 **单设备消息空洞也会丢**（G-03） |
| 会话漫游靠 history 补 | history 按账号限定；Chat `offline.content` 按 session 过滤。直打 Royal 要 HMAC |
| 会话 key 不含 app 只会导致互踢 / ACK 串 | 已用冻结 `app=kim` + loc/session v2 + Chat 拒非 kim session 关闭（见 [gray.md](gray.md)） |

原报告成立、且仍应保留的判断：多设备读索引、inbox 全量聚合、无系统推送、无撤回/已读回执、无限流、DEMO secret、无分布式追踪、无告警规则、Memory 一把大锁。Royal 多实例 + Chat 熔断已落地；Consul / Redis / PG 仍是单节点运维。群长连接已鉴权；无角色/邀请仍是功能缺口。

---

## 对库/运行时报告的订正

这份硬化规划（Conn/Channel 合同不动、先正确再快、已有库用尽、排除 io_uring/Raft/OpenSSL）**作为二期施工图合理**，不能当第一期，也不能原文落地 Phase 0。

| 原句 / 决策 | 代码事实或冲突 |
|---|---|
| Phase 0：`inserted.duplicate` 保持不二次 dispatch | 已否。identical 重试从落库再 Push |
| Phase 0：dispatch 失败靠离线 Pull 补洞 | 补洞被 G-03 全局 ACK 高水位打穿 |
| PR 顺序把 persist-first / try_send 放第 1 步 | persist-first 与控制面 HMAC 已落地；try_send/lanes 已关（G-29/G-30），仍排在游标 rollout 之后作为通信层 |
| SDK / Flutter 不在规划范围 | persist-first 依赖客户端重连 sync。Flutter supervisor 已拉离线；G-14 仍开因为 Web `isRetryable` 与 chat_ui leftover |
| 引入 `tokio-util` 为了 JoinSet | `tokio::task::JoinSet` 已在 tokio。tokio-util 只为 `CancellationToken` 才值得加 |
| `write_all_vectored` 示例 | 稳定 tokio 上该 API 常要 `tokio_unstable`。未验证前不要写进热路径 |
| ChannelMap 立刻分片 | `get` 已 clone `Channel` 再放锁，登录写锁才会挡全表。先做 G-29，profile 后再分片。拒绝 DashMap 作为第一刀是对的 |
| HASH 分区脚本做成迁移 | 草稿列名是 `account` 不是 `account_a`。按人 HASH 对 `offline.content` 按 id 取不友好。更合理的是 `message_index` RANGE(时间) + 索引；**不要**进默认 `sqlx::migrate!` |
| 手写 Consul HTTP，无官方 consul crate | 可接受。生产已是 ACL token + 私有 CA mTLS；缺的不是换官方 crate |
| 心跳 Redis 错误 fail-open | 已关：仅确认吊销立刻踢；存储错误连续 3 次后断开，期内不续签 JWT。登录仍 fail-closed（G-31） |
| 生产 JWT 仍是 DEMO 则退出 | 已并入控制面 strict 启动 |
| 雪花 init 失败禁止 Sequence 降级 | 与 G-22 一致 |
| 群仍写扩散，>200 分批 | 与 G-25 一致；现在就改读扩散会推翻 ACK/inbox，拒绝得对 |
| Dual-write 默认 fail-open | 对 |
| `panic = "abort"` 不第一期切 | 对 |
| 明确不引入 tungstenite / diesel / monoio / 自研 Raft | 对 |

**原则里成立、应保留的：** 合同不动；落库是真相、在线推是尽力（须补 G-03 / G-14）；TGateway rustls 不靠把 App 改成 WSS；redis-rs 用尽再评估 fred；trait 对象继续 `async_trait`。

---

## 运行时与开源库（订正后的二期）

**不要按原文 H0～H6 原样开工。** 租户已冻结为 `app=kim`。停机顺序已落地（见 [deploy.md](deploy.md)）。然后才是下面这套硬化。每期独立可编译、可测。合同：`Conn` / `Acceptor` / `MessageListener` / `Naming` / `SessionStorage` / `MessageStore` 保持。不要给自聊加 `(app, account_a, message_id)` 唯一约束。

### 目标形状（通信层）

```text
accept
  → handshake (timeout)
  → Channel::pair
       读专员: read_frame → Ping 本地回 → Binary 入该 channel 的串行 lane (try_send)
       写专员: mailbox 批量 write + 一次 flush
       每 lane 单 worker（或分片到固定 worker）：Semaphore 内跑 MessageListener::receive
  → idle: 读超时 / 应用 Ping 仍由读专员独占
```

`ChannelOpts` 增加 `write_full`（Block 仅内部链路 / Disconnect 为网关下行）和 `max_in_flight`。Ping 永不进 SQL。同一 `channel_id` 上请求/响应 FIFO。

### 库策略

```text
保留并加深
  tokio, bytes, thiserror, tracing, prost, fastwebsockets, hyper 1,
  redis-rs, sqlx, axum, rustls, jsonwebtoken, snowflake_me, prometheus

缺口处新引入（小、专职）
  socket2            OS 套接字选项（只进 kim-tcp）
  tokio-util         仅 CancellationToken（JoinSet 用 tokio::task）
  tokio-rustls       TGateway 服务端 TLS（客户端 kim-ws 已有 rustls）
  rustls-pemfile     证书
  tower-http         Royal/Router REST 中间件
  moka 或 lru        有界会话缓存（loc cache 已 opt-in 默认关）
  governor           握手/发言限流（放 gateway / Royal，不放 kim-tcp）
  tikv-jemallocator  Linux 网关 feature；macOS 保持系统分配器

明确不引入
  tokio-tungstenite, native-tls, diesel, sea-orm, OpenSSL,
  monoio/glommio 作默认 runtime, 自研 Raft, clickhouse（无分析需求前）
```

OS 能力：`TCP_NODELAY` 与 keepalive 已开。BufWriter 8KiB。reuseport / vectored write / io_uring、QUIC、一致哈希等 flamegraph 证明后再做（Phase 延后表）。

### 与条目对应的施工期

下面替代库报告原文的 Phase 0–6 顺序。语义条目仍以本文 G-xx 为准。

**H0 — 语义（无新库，叠 G-15/G-30）**

- persist-first **已做**，形状见 [control-layer-chat.md](control-layer-chat.md)。
- 网关下行 `try_send` **已做**（G-30）：`WriteFullPolicy::Disconnect` + `kim_mailbox_full_total`；内部链路默认 Block。
- ACK **不是** Snowflake 高水位（G-03）。

**H1 — 隔离（G-29）**。停机（G-07 / G-32）已合入，形状见 [deploy.md](deploy.md)。读循环隔离 **已做**，形状见 [communication-layer.md](communication-layer.md)。

- 读帧与业务解耦；**同一 `header.channel_id` 一条串行 lane**，只允许不同逻辑 channel 并发。join 未完成不得 talk 乱序；Ping 留读专员。
- 每 lane / 每进程 Semaphore；满则停读。
- 全进程连接上限已落地（`set_max_connections`，默认不限）。
- 保持 `kim-ws` `read_frame` 超时即拆连接（非 cancel-safe，禁止半帧 `select!` 再读）。

**H2 — 套接字与 TGateway TLS（G-34，已关）**

- socket2 keepalive、连接上限、`TcpConn<S>`、TGateway 进程内 rustls **已做**。形状见 [communication-layer.md](communication-layer.md)。
- Linux reuseport 多 accept、vectored write 仍延后（稳定 `write_all_vectored` 常要 `tokio_unstable`）。
- kim-ws 保持明文 + Caddy。证书只放 tgateway 配置。

**H3 — 用尽 redis / sqlx / axum / jwt（G-33）**

- `get_locations` pipeline + ConnectionManager 3s timeout **已做**。
- 心跳 Redis 错误有界宽限已落地（G-31）：连续 3 次失败后断开；登录 revoke 仍 fail-closed。
- sqlx `statement_timeout` / 池可配 **已做**；migrate 不用业务 5s timeout。
- Royal 重试退避 + 目录 800ms 预算 **已做**。
- Royal/Router `tower-http`、显式 Tokio runtime Builder **仍开**。
- 空 secret / DEMO secret：**生产启动失败**。

**H4 — 内存与 lint**

- loc cache：已 opt-in 默认关。有界 TTL / pubsub 失效仍后置。
- ChannelMap 分片 `RwLock<HashMap>`（N=16/32）——在 G-29 之后、有 profile 再做。
- Linux `jemalloc` feature。
- workspace clippy `unwrap_used` 对 `crates/*` deny（测试 allow）。
- release：`lto = "thin"`、`codegen-units = 1`；**不要**第一期 `panic = "abort"`。
- edition 2024 单开 PR，不与行为变更混。

**H5 — 存储规模（G-17, G-22, G-25）**

- additive `conversation_inbox`：双写 → 切读 → 再考虑删 GROUP BY。
- 大群：阈值可配（默认 200）；content 先提交，index 分批；失败进 outbox。
- outbox **必须**在与 content 同一事务里写入目标成员快照（或待 fanout 的 recipient 行）。重试只消费该快照，禁止再读当前 `group_members`——否则后加入者收到旧消息，已退出者漏收或重收。
- 雪花：`data_center_id` 可配；init 失败进程退出，测试仍显式 SequenceIdGen。雪花只做 content 主键，**不做** ACK 游标。
- 分区用运维脚本 + `CREATE INDEX CONCURRENTLY`，不进事务迁移。RANGE(时间) 优于错误的 HASH(account)。

**H6 — 限流、熔断、追踪（G-18, G-15）**

- Royal 发现 / 实例熔断 / 好友短缓存 **已做**（原 G-16）。
- gateway `governor`：每账号 talk QPS、每 IP 握手。
- Container `forward`：连续失败摘 Adult，半开探测；重连指数退避 + jitter，上限约 5s。
- `#[instrument]` 盖 accept/forward/talk。command 白名单 29 条 **已做**。
- `otel` feature 默认关。

**延后（有 flamegraph / 运维需求再做）**

| 项 | 库 | 何时 |
|---|---|---|
| Linux `SO_REUSEPORT` 多 accept | socket2 | accept 成为瓶颈且 flamegraph 证明 |
| vectored write | tokio `write_all_vectored` | 稳定 API 可用且 write syscall 占 CPU |
| 一致哈希 | rendezvous / jump hash | Chat 扩容导致会话打散成为问题 |
| QUIC `Conn` | quinn，新 crate `kim-quic` | 移动网抗丢包有产品需求 |
| io_uring 写路径 | tokio-uring 仅网关写出 | samply 显示 write syscall 占 CPU |
| fred | fred | Cluster + 自动 pipeline 成运维标配且 redis-rs 不够 |
| tonic 内部 RPC | tonic | Royal 对内 QPS 需要 gRPC 时 |
| permessage-deflate | 等 fastwebsockets | 文本收益小、CPU 大，默认不做 |

### 触及面（crate）

一期不必全改。语义修复仍以 G-xx 的文件为准。

- `crates/kim-core` — 读循环隔离、`try_push`、可选 ChannelMap 分片
- `crates/kim-tcp` / `kim-ws` — keepalive / 连接上限 / TLS 前端已落地；ws 仍明文；reuseport / vectored 延后
- `crates/kim-container` — token、退避、forward 熔断
- `crates/kim-session` — pipeline、有界 cache、mirror 指标
- `crates/kim-metrics` — dispatch/mailbox/push_drop；command 白名单
- `crates/kim-protocol` — secret 校验；非 HS256 只留注入点
- `services/chat`（talk / idgen / RoyalPool / 社交缓存 / postgres pool）、各 `main` 的 runtime 与 SIGTERM
- `deploy/compose.yml` 与 docs：PgBouncer / Redis 容量写进 [deploy.md](deploy.md)，不要把 64MB 当成生产默认不说明

### 明确不改

- 不用 tungstenite 换 fastwebsockets
- 不把登录写进 `TcpServer`/`WsServer`
- 不把 Consul 换成 etcd（Naming 已隔离）
- 不上自研 Raft / 2PC；可靠性靠 PG 事务 + 幂等 + at-least-once
- 不改内核、不用 DPDK
- 不承诺 exactly-once，不引入 `delivered` 列
- 协议指令名与 JWT HS256 默认保持；persist-first 是客户端可观察行为变化（已落库不再回 99）。Web `isRetryable` **不改**

---

## 客户端如何放大服务端问题

服务端 persist-first 之后已落库回 Success；剩下的 99 是 insert / 目录故障。Web 仍不重试 99。漏 Push 仍可能静默（G-03 / G-14）。

服务端无主动 sync + Flutter 不拉离线 + 无 APNs → 杀进程后消息静默，直到用户碰巧再打开且将来补了 sync。

服务端全局 ACK + Web 只 ACK `lastMessage` → 单端空洞直接变成永久丢失。

服务端 device 自报 + Flutter 默认 `mobile` → 两台手机互踢；电脑若填 web 可并存，但电脑不拉离线。

---

## 部署隐含假设

- Caddy 只把 auth 和 lookup、WS 暴露到公网。不要把「没反代」写成安全设计。
- `CHAT_URL=http://chat:9002` 让 Royal 打 Chat admin（HMAC + nonce）。Redis `requirepass` + `noeviction`；Consul HTTPS/mTLS 8501 + ACL deny。
- Consul bootstrap-expect=1，Redis/PG 单节点。
- 生产 `KIM_JWT_SECRET` / `KIM_INTERNAL_HMAC_SECRET` 不得为 demo 或 `change-me`（strict 拒启动）。`bootstrap.sh` 用 openssl hex，不写 `change-me`。

---

## 验证（修复时用，不是现在的绿灯）

应新增、目前没有的：

- 自聊 insert 仍成功（两条 index）；不得依赖 `(app, account_a, message_id)` 唯一约束
- 非 `kim` JWT 登录失败（105）；预置 `app=kim-gray` 的 session 不得 talk / insert
- 设备 A ACK 后设备 B 仍能 pull 未被自己 ACK 的段
- 两个并发 insert：小 snowflake id 后提交；ACK 大 id 之后小 id 对该设备仍可见（G-03/G-04）
- id=10 dispatch 失败、id=11 已 Push：10 对该设备仍可见（不是被高水位吃掉）
- Chat 节点缓存的 location，它机 delete 后不得再 push 到旧 channel
- 无 Consul token 时生产配置拒绝 register（或测试替身）
- SIGTERM：**先摘发现**，drain 窗口内 lookup 不得再返回该实例（单元测试已覆盖顺序；未做 K8s 杀进程）
- 同一 channel 上 join 未完成时并发 talk 的顺序（H1 lane）
- 改密后旧 JWT 不可调 `/auth/me`；第二台预先签发的 JWT 同样 401；kick 仍打对应 account（B1 已覆盖）
- 未签名的 Chat `/internal/kick` 被拒

命令（仓库惯例）：

- `cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && env -u REDIS_URL cargo test --workspace`
- 通信层改动另跑 `cargo test -p kim-tcp --test echo` 与 `cargo test -p kim-ws --test echo --test wss`
- ACK 语义：`cargo test -p chat --test e2e_talk --test e2e_offline`
- 改协议或 ACK 语义时同步 [reliable-delivery.md](reliable-delivery.md)、[gray.md](gray.md)、[group-royal.md](group-royal.md)、[web-sdk.md](web-sdk.md)、[mobile-client.md](mobile-client.md)、[perf.md](perf.md)、[observability.md](observability.md)、[deploy.md](deploy.md)
