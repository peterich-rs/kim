# KIM

Rust 实现的分布式即时通讯骨架。对照 King IM Cloud 的分层来学后台：先把长连接、分帧、连接生命周期做对，再往上长业务包、服务发现和转发。

当前本机可跑：**Royal + 网关 + Chat**（注册/登录 JWT、会话、互踢、在线单聊 / 群聊、ACK、Pull 离线）。产品 Web 是 `sdk/web/app`；小册 H5 仍是 `sdk/web/demo`。CLI 是 `pkt-client`。默认会话和消息都是进程内 Memory，不需要 Redis / Docker / Postgres / Consul。VPS 用 `deploy/compose.yml` 跑 gateway / chat / chat-gray / royal / router 加 Redis / Postgres / Consul，见 [docs/deploy.md](docs/deploy.md)。通信层 TCP / WS 回声由 crate 测试覆盖，不再另起 echo 二进制。

## 当前进度

| 层 | 状态 | 仓库里对应什么 |
|---|---|---|
| 通信层 | 已落地 | `kim-core` + `kim-tcp` + `kim-ws`。TCP / WS 都履行 `Conn` |
| 容器层 | 已落地 | `kim-naming`（本机 StaticNaming；生产 Consul blocking watch）+ `kim-container`（Young → Adult 后 Forward） |
| 业务包 | 已落地 | `kim-protocol`：Magic + BasicPkt / LogicPkt + JWT HS256 |
| 链路层 | 已落地 | [docs/link-layer-login.md](docs/link-layer-login.md)：Router + JWT 登录 + 会话 + 互踢 |
| 控制层 | 在线 + 离线 + 群管理 + Royal | [docs/control-layer-chat.md](docs/control-layer-chat.md)、[docs/reliable-delivery.md](docs/reliable-delivery.md)、[docs/group-royal.md](docs/group-royal.md) |
| Web SDK | 已落地 | [docs/web-sdk.md](docs/web-sdk.md)：`sdk/web`，登录 / 收发 / 离线 / ACK / 群 |
| 进阶 25–32 | 已落地 | [docs/bench.md](docs/bench.md)、[docs/perf.md](docs/perf.md)、[docs/routing.md](docs/routing.md)、[docs/gray.md](docs/gray.md)、[docs/observability.md](docs/observability.md)、[docs/deploy.md](docs/deploy.md)（Docker / GHCR） |

进程：`pkt-client` → `gateway`（`:8001`）→ `chat`（`:8002`）。Upgrade 后第一帧是 `login.signin`（JWT），网关生成 `wg-1_alice_N`（不再是 `"alice"`）。`BasicPkt` ping 在网关本地回 pong；`chat.demo.echo` 登录之后才 Forward 到 Chat。规格与词表在 [docs/](docs/README.md)。

## 本机怎么跑

需要 [Rust](https://rustup.rs/)。工具链钉在 `rust-toolchain.toml`（当前 1.95.0），clone 之后 rustup 会自动用这个版本。

产品 Web 默认同生产后台（只起页面，不必本机 Royal / Chat / 网关）：

```bash
cd sdk/web && npm run app
```

打开 http://127.0.0.1:5173/ 。Vite 把 `/api` 代理到 `https://kim.ainexc.com`，长连接走 `wss://kim.ainexc.com/`。

本机全套（Memory。先 Royal，再 Chat，再网关，再页面）：

```bash
# 终端 1
RUST_LOG=info cargo run -p royal

# 终端 2
RUST_LOG=info cargo run -p chat

# 终端 3
RUST_LOG=info cargo run -p gateway

# 终端 4
cd sdk/web && npm run app:local
```

打开 http://127.0.0.1:5173/ 。`app:local` 把 `/api` 代理到 Royal `:8080`，WebSocket 连本机网关 `:8001`。

CLI（本地签 JWT，不必 Royal）：

```bash
RUST_LOG=info cargo run -p pkt-client -- alice
```

成功时客户端打印的 `channel_id` 形如 `wg-1_alice_1`，**不是** `"alice"`。默认随后本地 ping，再 `chat.demo.echo`。连生产 Royal 用 `KIM_AUTH_URL` + `KIM_PASSWORD` 走 `/login`。

小册 H5（本机 mint，不打 Royal）：`cd sdk/web && npm run demo`，见 [docs/web-sdk.md](docs/web-sdk.md)。

```bash
# 1:1：终端 A HOLD 等 Push，终端 B 发给 A
KIM_HOLD=1 RUST_LOG=info cargo run -p pkt-client -- bob
KIM_TALK_TO=bob RUST_LOG=info cargo run -p pkt-client -- alice

# 建群并群聊
KIM_GROUP_MEMBERS=alice,bob,carol RUST_LOG=info cargo run -p pkt-client -- alice

# 可选 Royal HTTP（先 Royal 再 Chat）
RUST_LOG=info cargo run -p royal
ROYAL_URL=http://127.0.0.1:8080 RUST_LOG=info cargo run -p chat

# 互踢：终端 A 先 HOLD，终端 B 再登录同一账号
KIM_HOLD=1 RUST_LOG=info cargo run -p pkt-client -- alice
RUST_LOG=info cargo run -p pkt-client -- alice

# 坏 token（Unauthorized 或 WS Close）
KIM_BAD_TOKEN=1 cargo run -p pkt-client -- alice

# 登录后只 ping（不到 Chat）
KIM_PING_ONLY=1 cargo run -p pkt-client -- alice

# 只起网关、不起 Chat：握手失败（ServiceUnavailable 或 Close）
KIM_EXPECT_UNAVAILABLE=1 cargo run -p pkt-client -- alice
```

连 `gateway` 的第一帧必须是 JWT `login.signin`。客户端连网关是 WebSocket Upgrade，网关连 Chat 是 TCP `InnerHandshakeReq`。通信层「第一帧当名字」的回声只存在于 `crates/kim-tcp/tests/echo.rs`、`crates/kim-ws/tests/echo.rs`、`crates/kim-container/tests/e2e_echo.rs`，不要和登录握手混。

```bash
env -u REDIS_URL cargo test --workspace
```

测试只构造 Memory 会话，不读 `REDIS_URL`，不需要活 Redis / Consul。

## 仓库结构

```
crates/kim-core         通信层说明书：Conn / Frame / Channel / ChannelMap
crates/kim-tcp          TCP 分帧（App / 网关↔Chat）
crates/kim-ws           WebSocket（HTTP Upgrade 之后）
crates/kim-protocol     Magic + BasicPkt + LogicPkt + JWT
crates/kim-naming       静态服务发现（不是 Consul）
crates/kim-container    全连接拨号、Young/Adult、Forward / Push
crates/kim-router       指令 Router / Context / Dispatch
crates/kim-session      会话存储（默认 Memory，可选 Redis feature）
services/               gateway / tgateway / chat / royal / router
examples/               pkt-client / kimbench
deploy/                 VPS Compose（gateway / chat / Redis / Postgres）
sdk/web                 TypeScript Web SDK（第 23–24 章）
docs/                   词表、分层合同、登录与控制层规格、进阶篇 as-built
```

原则：**换传输只加 `Conn` 实现，不改业务。** 登录、互踢、群聊都不进 `TcpServer` / `WsServer`。

## 学习文档

1. [docs/glossary.md](docs/glossary.md) — 进程、端口、帧、channel_id、JWT / 会话
2. [docs/architecture.md](docs/architecture.md) — crate 职责、进门怎么走
3. [docs/communication-layer.md](docs/communication-layer.md) — 两专员、查表放锁
4. [docs/protocol-container.md](docs/protocol-container.md) — 已落地的业务包与容器
5. [docs/link-layer-login.md](docs/link-layer-login.md) — 登录、会话、互踢
6. [docs/control-layer-chat.md](docs/control-layer-chat.md) — 在线单聊 / 群聊
7. [docs/reliable-delivery.md](docs/reliable-delivery.md) — ACK / 写扩散 / 离线 Pull
8. [docs/group-royal.md](docs/group-royal.md) — 群 join/quit/detail、可选 Royal
9. [docs/web-sdk.md](docs/web-sdk.md) — TypeScript Web SDK
10. [docs/bench.md](docs/bench.md) — kimbench
11. [docs/perf.md](docs/perf.md) — 写路径 / 缓冲 / 寻址缓存
12. [docs/routing.md](docs/routing.md) — HTTP 智能路由
13. [docs/gray.md](docs/gray.md) — 租户 / zone 灰度
14. [docs/observability.md](docs/observability.md) — Prometheus `/metrics`
15. [docs/deploy.md](docs/deploy.md) — Docker Compose / GHCR / VPS

## 开发

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
env -u REDIS_URL cargo test --workspace
cd sdk/web && npm ci && npm test
```

push 和 pull request 会跑同一套检查（见 `.github/workflows/ci.yml`）。

## 许可

MIT。见 [LICENSE](LICENSE)。
