# KIM

Rust 实现的分布式即时通讯骨架。对照 King IM Cloud 的分层来学后台：先把长连接、分帧、连接生命周期做对，再往上长业务包、服务发现和转发。

当前本机可跑：**TCP echo**、**WebSocket echo**（第一帧仍是名字），以及 **假网关 + 假 Chat 登录 Demo**（JWT 握手、会话、互踢、登录后再 echo）。默认会话是进程内 Memory，不需要 Redis / Docker / Consul。

## 当前进度

| 层 | 状态 | 仓库里对应什么 |
|---|---|---|
| 通信层 | 已落地 | `kim-core` + `kim-tcp` + `kim-ws`。TCP / WS 都履行 `Conn` |
| 容器层 | 已落地 | `kim-naming`（静态配置）+ `kim-container`（Young → Adult 后 Forward） |
| 业务包 | 已落地 | `kim-protocol`：Magic + BasicPkt / LogicPkt + JWT HS256 |
| 链路层 | **M3 已落地** | [docs/link-layer-login.md](docs/link-layer-login.md)：Router + JWT 登录 + 会话 + 互踢 |
| 控制层 | 以后 | 单聊、群聊、离线 |

进程：`pkt-client` → `fake-gateway`（`:8001`）→ `fake-chat`（`:8002`）。Upgrade 后第一帧是 `login.signin`（JWT），网关生成 `wg-1_alice_N`（不再是 `"alice"`）。`BasicPkt` ping 在网关本地回 pong；`chat.demo.echo` 登录之后才 Forward 到 Chat。规格与词表在 [docs/](docs/README.md)。

## 本机怎么跑

需要 [Rust](https://rustup.rs/)。工具链钉在 `rust-toolchain.toml`（当前 1.95.0），clone 之后 rustup 会自动用这个版本。

TCP 回声（App / TGateway 路径，`:8000`，第一帧仍是名字）：

```bash
cargo run -p echo-server
cargo run -p echo-client -- alice
```

WebSocket 回声（同一套 `EchoHandler`，换电线，第一帧仍是名字）：

```bash
cargo run -p ws-echo-server
cargo run -p ws-echo-client -- alice
```

登录 Demo（Memory 会话。必须先 Chat 再网关，再客户端）：

```bash
# 终端 1
RUST_LOG=info cargo run -p fake-chat

# 终端 2（等 Chat listen）
RUST_LOG=info cargo run -p fake-gateway

# 终端 3
RUST_LOG=info cargo run -p pkt-client -- alice
```

成功时客户端打印的 `channel_id` 形如 `wg-1_alice_1`，**不是** `"alice"`。随后本地 ping，再 `chat.demo.echo`。

```bash
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

通信层 echo 的第一帧是名字；连 `fake-gateway` 的第一帧必须是 JWT `login.signin`。两条握手不要混：客户端连网关是 WebSocket Upgrade，网关连 Chat 是 TCP `InnerHandshakeReq`。

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
examples/               echo / ws-echo / fake-gateway / fake-chat / pkt-client
docs/                   词表、分层合同、登录规格
```

原则：**换传输只加 `Conn` 实现，不改业务。** 登录、互踢、群聊都不进 `TcpServer` / `WsServer`。

## 学习文档

1. [docs/glossary.md](docs/glossary.md) — 进程、端口、帧、channel_id、JWT / 会话
2. [docs/architecture.md](docs/architecture.md) — crate 职责、进门怎么走
3. [docs/communication-layer.md](docs/communication-layer.md) — 两专员、查表放锁
4. [docs/protocol-container.md](docs/protocol-container.md) — 已落地的业务包与容器
5. [docs/link-layer-login.md](docs/link-layer-login.md) — **M3 已落地**：登录与会话

## 开发

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
env -u REDIS_URL cargo test --workspace
```

push 和 pull request 会跑同一套检查（见 `.github/workflows/ci.yml`）。

## 许可

MIT。见 [LICENSE](LICENSE)。
