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

## Royal HTTP（可选）

进程：`examples/fake-royal`，默认 `127.0.0.1:8080`。`Content-Type` / `Accept`：`application/x-protobuf`。

Chat `ROYAL_URL` 或 `config.toml royal_url` 非空时，`MessageStore` 与 `GroupDirectory` 都走 HTTP。空则仍是进程内 Memory（默认测试）。

| 方法 | 路径 |
|---|---|
| POST | `/api/:app/message/user` |
| POST | `/api/:app/message/group` |
| POST | `/api/:app/message/ack` |
| POST | `/api/:app/offline/index` |
| POST | `/api/:app/offline/content` |
| POST | `/api/:app/group` |
| POST | `/api/:app/group/member` |
| DELETE | `/api/:app/group/member` |
| GET | `/api/:app/group/members/:group` |
| GET | `/api/:app/group/:group` |

本机：先 `cargo run -p fake-royal`，再 Chat 带 `ROYAL_URL=http://127.0.0.1:8080`。

Consul HTTP catalog 是 `kim-naming` feature `consul`（`ConsulNaming`）。默认测试不连 Consul，也不占用 53 端口。

---

## pkt-client

| 变量 | 行为 |
|---|---|
| `KIM_GROUP_JOIN` | dest=该 group id，join |
| `KIM_GROUP_QUIT` | quit |
| `KIM_GROUP_DETAIL` | detail |
| HOLD 收到 create Push | 打 `GroupCreateNotify` |

e2e：`examples/fake-chat/tests/e2e_group.rs`（Memory）、`e2e_royal.rs`（HTTP）。
