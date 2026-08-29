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

进程：`examples/fake-royal`，默认 `127.0.0.1:8080`。消息/群：`Content-Type` / `Accept` `application/x-protobuf`。Token：JSON。

Chat `ROYAL_URL` 或 `config.toml royal_url` 非空时，`MessageStore` 与 `GroupDirectory` 都走 HTTP。空则仍是进程内 Memory（默认测试、本机 `cargo run`）。生产 compose 必设 `ROYAL_URL`；Postgres 只由 Royal 写。

| 方法 | 路径 | 格式 |
|---|---|---|
| GET | `/health` | text |
| POST | `/api/:app/token` | JSON `{account}` → `{token, exp}` |
| POST | `/api/:app/message/user` | protobuf |
| POST | `/api/:app/message/group` | protobuf |
| POST | `/api/:app/message/ack` | protobuf |
| POST | `/api/:app/offline/index` | protobuf |
| POST | `/api/:app/offline/content` | protobuf |
| POST | `/api/:app/group` | protobuf |
| POST | `/api/:app/group/member` | protobuf |
| DELETE | `/api/:app/group/member` | protobuf |
| GET | `/api/:app/group/members/:group` | protobuf |
| GET | `/api/:app/group/:group` | protobuf |

Token：HS256，claims `acc` / `app` / `exp`，密钥与网关相同（`KIM_JWT_SECRET`）。可选 `KIM_TOKEN_ISSUE_KEY` + 头 `X-KIM-Issue-Key`；空则开放签发（demo）。公网 Caddy 只反代 `/api/lookup*` 与 `/api/*/token`。

本机：先 `cargo run -p fake-royal`，再 Chat 带 `ROYAL_URL=http://127.0.0.1:8080`。

Consul HTTP catalog 是 `kim-naming` feature `consul`。`subscribe` 用 blocking query；DNS 不占宿主机 53。默认测试不连 Consul。

---

## pkt-client

| 变量 | 行为 |
|---|---|
| `KIM_GROUP_JOIN` | dest=该 group id，join |
| `KIM_GROUP_QUIT` | quit |
| `KIM_GROUP_DETAIL` | detail |
| HOLD 收到 create Push | 打 `GroupCreateNotify` |

e2e：`examples/fake-chat/tests/e2e_group.rs`（Memory）、`e2e_royal.rs`（HTTP）。
