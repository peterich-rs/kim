# 群管理与 Royal（已落地）

对照小册第 22 章。在线 talk 仍以 [control-layer-chat.md](control-layer-chat.md) 为准。本文只记 **现在代码里的** 群 join/quit/detail 和可选 Royal HTTP。

网关对 `chat.group.*` 仍然 `forward("chat")`。**不要**把群逻辑写进 `TcpServer` / `WsServer`。

---

## 长连接指令

| command | dest | 行为 |
|---|---|---|
| `chat.group.create` | 空 | 建群。`owner` 强制 session；初始成员只有创建者。请求里其它 `members` 丢掉。Notify 只打创建者其它设备 |
| `chat.group.join` | group id | 默认私有群，禁用自助加入。已是成员 → Success；否则 `Unauthorized`。代他人操作 → `Unauthorized`。邀请协议落地前，入群只有 create |
| `chat.group.quit` | group id | 只能退自己。未知群或非成员 → `NotGroupMember` |
| `chat.group.detail` | group id | 须是成员。非成员或未知群 → `NotGroupMember`（无 body） |
| `chat.group.members` | group id | 须是成员。非成员或未知群 → `NotGroupMember`（无 body） |

group id 仍是雪花 **base36**，与 talk dest 相同。**不要**改成小册 REST Base32。

`GroupError::NotFound` 与目录 `Backend` 分开：未知群/非成员 → `NotGroupMember`；SQL / Royal 5xx → `SystemException`。Chat→Royal 群 HTTP 走 `InternalGroupCreate` / `InternalGroupQuery` / `InternalGroupMember`（带 session.app），不用客户端 `GroupQueryReq` 当租户字段。无 HMAC 直打 Royal 是 401。

---

## Royal HTTP

进程：`services/royal`，默认 `127.0.0.1:8080`。消息/群：`Content-Type` / `Accept` `application/x-protobuf`。Token：JSON。

Chat `ROYAL_URL` 或 `config.toml royal_url` 非空时，`MessageStore` 与 `GroupDirectory` 都走 HTTP。空则仍是进程内 Memory（默认测试、本机 `cargo run`）。生产 compose 必设 `ROYAL_URL`；Postgres 只由 Royal 写。

除 `/health` 与 `/api/v1/auth/*` 外，Royal HTTP 都要 HMAC-SHA256：`x-kim-timestamp`、`x-kim-nonce`、`x-kim-signature`。密钥是 `KIM_INTERNAL_HMAC_SECRET`（空则 demo 默认值；生产 `strict_runtime` 拒 demo/`change-me`）。canonical 串是 `METHOD`、`PATH`、timestamp、nonce、body 各占一行。允许 ±60s 时钟差。验签成功后 Redis `SET kim:hmac-nonce:{nonce} NX EX 121` 占 nonce；重放 401，Redis 故障 503。缺签或错签返回 401，body 是 `unauthorized`，不含 Fanout。Chat `RoyalClient` 与网关 `HttpRevoke` 自动带签。Chat `POST /internal/kick` 走同一合同；Royal `kick_account` 签名后 2s 超时 POST，失败只打日志，logout 仍 204。

| 方法 | 路径 | 格式 |
|---|---|---|
| GET | `/health` | text |
| POST | `/api/v1/auth/register` | protobuf `AuthReq` → `AuthResp` |
| POST | `/api/v1/auth/login` | protobuf `AuthReq` → `AuthResp` |
| POST | `/api/v1/auth/logout` | `Authorization: Bearer` → 204；吊销当前 `jti` 并 `kick_account`（仍全端踢） |
| POST | `/api/v1/auth/password` | `Authorization: Bearer` + protobuf `PasswordChangeReq` → 204；一条语句改哈希并 bump `token_epoch`、revoke 当前 jti、`kick_account`；不发新 token |
| GET | `/api/v1/auth/me` | `Authorization: Bearer` → JSON `{account,app}`；jti 吊销或 `ver < epoch` 则 401 |
| POST | `/api/v1/message/user` | protobuf |
| POST | `/api/v1/message/group` | protobuf |
| POST | `/api/v1/message/ack` | protobuf |
| POST | `/api/v1/offline/index` | protobuf |
| POST | `/api/v1/offline/content` | protobuf |
| POST | `/api/v1/group` | protobuf `InternalGroupCreate` |
| POST | `/api/v1/group/member` | protobuf `InternalGroupMember` |
| POST | `/api/v1/group/quit` | protobuf `InternalGroupMember` |
| POST | `/api/v1/group/members` | protobuf `InternalGroupQuery` → `GroupMembersResp` |
| POST | `/api/v1/group/detail` | protobuf `InternalGroupQuery` → `GroupDetail` |

`app` 不在 URL 里。群与 offline content 的内部 body 带 Chat session 的 `app`；Royal 用请求值，不用进程 `KIM_APP`。其它仍走进程 `KIM_APP` / 配置（默认 `kim`）。Token：HS256，claims `acc` / `app` / `exp` / `jti` / `ver`（`token_epoch`，旧 token 缺省 0）/ 可选 `did`（仅 enroll 或出示有效 device credential 时写入），密钥与网关相同（`KIM_JWT_SECRET`）。产品页走 `/api/v1/auth/register|login|logout`，不再开放签发。公网 Caddy 反代 `/api/lookup` 与 `/api/v1/auth/*`。**不要**反代 `/internal/*`。

内部（loopback / compose 内网）：

| 方法 | 路径 | 谁调用 |
|---|---|---|
| POST | `/internal/user/lookup` | Chat：dest 是否存在 |
| POST | `/internal/user/upsert` | Chat：长连登录写入用户表 |
| POST | `/internal/revoke/check` | 网关无 Redis 时查 `jti` |
| POST | `/internal/token-epoch` | 网关无 Redis 时读账户 `token_epoch`（`max(缓存, users.token_epoch)`） |
| POST | `/internal/device/check` | 网关无 Redis 时校验 `did` / credential |
| POST | `{CHAT_URL}/internal/kick` | Royal logout / 改密：Kickout 该账号全部长连接 |

`REDIS_URL` 时 logout 把 `jti` 写入 `kim:revoke:{jti}`；改密另写 `kim:token_epoch:{account}`。网关 Accept 与心跳查 jti 吊销 **或** `ver < epoch`，失败则拒绝。`/me` 与 `/internal/token-epoch` 以 `users.token_epoch` 为权威，缓存 miss 或 Royal 重启不以 0 放行旧 JWT。enroll / 出示成功才写 JWT `did`；`kim:device:{did}` 热 key 写入失败则 login/register 失败，不签发带 `did` 的 token。Chat 生产路径必须 `ROYAL_URL`，否则 dest 查的是空 Memory。

本机：先 `cargo run -p royal`，再 Chat 带 `ROYAL_URL=http://127.0.0.1:8080`。

Consul HTTP catalog 是 `kim-naming` feature `consul`。`subscribe` 用 blocking query；DNS 不占宿主机 53。默认测试不连 Consul。

---

## pkt-client

| 变量 | 行为 |
|---|---|
| `KIM_GROUP_JOIN` | dest=该 group id，join |
| `KIM_GROUP_QUIT` | quit |
| `KIM_GROUP_DETAIL` | detail |
| HOLD 收到 create Push | 打 `GroupCreateNotify` |

e2e：`services/chat/tests/e2e_group.rs`（Memory）、`e2e_royal.rs`（HTTP）。
