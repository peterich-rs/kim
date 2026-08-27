# KIM

Rust 实现的分布式即时通讯骨架，对照 King IM Cloud 的分层来学后台：通信层（TCP / WebSocket）、业务包、静态服务发现、容器转发。

组织：[peterich-rs](https://github.com/peterich-rs) / 仓库：`kim`

当前进度：本机可跑 TCP echo、WebSocket echo，以及「假网关 + 假 Chat」业务包 Demo。还没有登录、Redis、Consul、公网部署。

学习笔记：[docs/](docs/README.md)（词表、架构、通信层链路图、业务包与容器规格）。

## 本机怎么跑

需要 [Rust](https://rustup.rs/)。

TCP 回声（App / TGateway 路径）：

```bash
cargo run -p echo-server
cargo run -p echo-client -- alice
```

WebSocket 回声（同一套 EchoHandler，换电线）：

```bash
cargo run -p ws-echo-server
cargo run -p ws-echo-client -- alice
```

业务包 Demo（先 Chat 再网关）：

```bash
cargo run -p fake-chat
cargo run -p fake-gateway
cargo run -p pkt-client -- alice
```

```bash
cargo test --workspace
```

## 仓库结构

```
crates/kim-core         通信层说明书（Conn / Channel / ChannelMap）
crates/kim-tcp          TCP 帧与网关
crates/kim-ws           WebSocket（HTTP Upgrade 之后）
crates/kim-protocol     Magic + BasicPkt + LogicPkt
crates/kim-naming       静态服务发现
crates/kim-container    全连接拨号、Young/Adult、Forward
examples/               echo / ws-echo / fake-gateway / fake-chat / pkt-client
```

## 许可

MIT。见 [LICENSE](LICENSE)。
