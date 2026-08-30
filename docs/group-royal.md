# 群管理与 Royal（已落地）

对照小册第 22 章。在线 talk 仍以 [control-layer-chat.md](control-layer-chat.md) 为准。本文只记 **现在代码里的** 群 join/quit/detail 和可选 Royal HTTP。

网关对 `chat.group.*` 仍然 `forward("chat")`。**不要**把群逻辑写进 `TcpServer` / `WsServer`。

---

## 长连接指令

| command | dest | 行为 |
|---|---|---|
| `chat.group.create` | 空 | 建群；在线成员收到 `Flag=Push` 的 `GroupCreateNotify`（同 command，跳过发送方） |
| `chat.group.join` | group id | 加入。账号优先 session，body `account` 可填 |
| `chat.group.quit` | group id | 退出。未知群也是 Success |
| `chat.group.detail` | group id | 返回 `GroupDetail` |
| `chat.group.members` | group id | 返回成员列表；未知群空列表 |

group id 仍是雪花 **base36**，与 talk dest 相同。**不要**改成小册 REST Base32。

未知群 join / detail → `SystemException`。`create` / join 失败不发 Notify。

---

## Royal HTTP

进程：`services/royal`，默认 `127.0.0.1:8080`。消息/群：`Content-Type` / `Accept` `application/x-protobuf`。Token：JSON。

Chat `ROYAL_URL` 或 `config.toml royal_url` 非空时，`MessageStore` 与 `GroupDirectory` 都走 HTTP。空则仍是进程内 Memory（默认测试、本机 `cargo run`）。生产 compose 必设 `ROYAL_URL`；Postgres 只由 Royal 写。

| 方法 | 路径 | 格式 |
|---|---|---|
| GET | `/health` | text |
| POST | `/api/v1/auth/register` | protobuf `AuthReq` → `AuthResp` |
| POST | `/api/v1/auth/login` | protobuf `AuthReq` → `AuthResp` |
| POST | `/api/v1/auth/logout` | `Authorization: Bearer` → 204；吊销 `jti` |
| POST | `/api/v1/auth/password` | `Authorization: Bearer` + protobuf `PasswordChangeReq` → 204 |
| GET | `/api/v1/auth/me` | `Authorization: Bearer` → JSON `{account,app}`；吊销后 401 |
| POST | `/api/v1/message/user` | protobuf |
| POST | `/api/v1/message/group` | protobuf |
| POST | `/api/v1/message/ack` | protobuf |
| POST | `/api/v1/offline/index` | protobuf |
| POST | `/api/v1/offline/content` | protobuf |
| POST | `/api/v1/group` | protobuf |
| POST | `/api/v1/group/member` | protobuf |
| POST | `/api/v1/group/quit` | protobuf `GroupQuitReq` |
| POST | `/api/v1/group/members` | protobuf `GroupQueryReq` → `GroupMembersResp` |
| POST | `/api/v1/group/detail` | protobuf `GroupQueryReq` → `GroupDetail` |

`app` 不在 URL 里：Royal 进程用 `KIM_APP` / 配置（默认 `kim`），各部署用各自的 base URL。Token：HS256，claims `acc` / `app` / `exp` / `jti`，密钥与网关相同（`KIM_JWT_SECRET`）。产品页走 `/api/v1/auth/register|login|logout`，不再开放签发。公网 Caddy 反代 `/api/lookup*` 与 `/api/v1/auth/*`。**不要**反代 `/internal/*`。

内部（loopback / compose 内网）：

| 方法 | 路径 | 谁调用 |
|---|---|---|
| POST | `/internal/user/lookup` | Chat：dest 是否存在 |
| POST | `/internal/user/upsert` | Chat：长连登录写入用户表 |
| POST | `/internal/revoke/check` | 网关无 Redis 时查 `jti` |
| POST | `{CHAT_URL}/internal/kick` | Royal logout：Kickout 当前长连接 |

`REDIS_URL` 时 logout 把 `jti` 写入 `kim:revoke:{jti}`；网关 Accept 与心跳都查，失败则拒绝。Chat 生产路径必须 `ROYAL_URL`，否则 dest 查的是空 Memory。

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
