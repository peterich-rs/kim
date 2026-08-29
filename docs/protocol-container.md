# WebSocket、业务包与容器（已落地规格）

面向：刚学后台、靠复刻 KIM 入门的工程师。  
本文记录 **已经实现的** WebSocket `Conn`、Magic/BasicPkt/LogicPkt、静态 Naming 与 Container。登录 JWT、会话、互踢见 [link-layer-login.md](link-layer-login.md)。`MessageReq` / `MessageResp` / `MessagePush` 与 talk 指令见 [control-layer-chat.md](control-layer-chat.md)，本文不展开。本文不含 Consul、VPS、Royal。

crate 测试（`kim-tcp` / `kim-ws` / `kim-container` 的 echo）仍是第一帧名字。`pkt-client` → `fake-gateway` 已改为 JWT `login.signin`；下文若仍写 utf8 `"alice"` 或 `echo-server`，那是 **M2 当时的 Demo 历史**，以 [link-layer-login.md](link-layer-login.md) 和根 README 为准。echo 二进制已删，验收走 crate 测试。

阅读前请先扫 [glossary.md](glossary.md) 和 [communication-layer.md](communication-layer.md)。本文出现的新词会在第一次使用时解释。

---

## 1. 背景 / 当前状态

### 1.1 已经落地的（M1a）

通信层 TCP + echo 已跑通。业务只依赖 `kim-core` 的合同，传输闷在 `Conn` 里。

| 路径 | 职责 |
|---|---|
| `crates/kim-core` | 说明书：`Conn` / `Frame` / `OpCode` / `Channel::pair` / `ChannelMap` / `Acceptor` / `MessageListener` / `StateListener` / `Agent` / `Server` / `Client` |
| `crates/kim-tcp` | TCP 履行者：`opcode 1B \| len 4B LE \| payload`，`TcpServer` / `TcpClient` / `IdentityDialer` |
| `crates/kim-tcp/tests/echo.rs` | 进程内 TCP 回声验收：第一帧当名字，原文加 ` from server` |

已经拍板、且代码里已经是这样的：

- 每条连接 **两个专员**：写走 `mpsc` + 唯一写任务；读是独占循环。`ChannelMap` 的 `RwLock` **只护字典**；`get` 时 clone 出 `Channel`（里面是写信箱）立刻放锁。
- `Conn` 合同：**可靠、有序、有边界**。TCP / WebSocket / 以后的 QUIC 可靠流都能履行；裸 UDP 不能。
- `TcpClient` 写侧仍是 `Mutex` 包着写半边。本阶段 **不改**。

链路（已经存在，WS 必须复用，禁止再抄一套）：

```
张三 push ─┐
李四 push ─┼──► Alice 的写信箱（mpsc）──► 写专员（唯一 write）──► 网线
心跳 Pong ─┘
Alice 打字 ──► 网线 ──► 读专员（唯一 read）──► receive（业务）
                                      └── Ping 也丢给写信箱
```

### 1.2 M1b+M2 当时要补的缺口（已落地）

1. **第二种 `Conn` 履行者**：HTTP Upgrade 之后的 RFC 6455 WebSocket。用 **同一个** `EchoHandler` 证明合同与传输无关。
2. **业务包**：通信层帧的 Binary **payload** 里再套 Magic + BasicPkt / LogicPkt。心跳（BasicPkt ping）在网关本地回；业务（LogicPkt）按 `command` 前缀转给 Chat。
3. **容器 + 静态 Naming**：网关拨号 **配置里列出的全部** Chat 实例（不是 HTTP 那种挑一台），新实例先 Young 再 Adult，Adult 之后才承接转发。

### 1.3 和小册的差异（已拍板，不要再争论）

三件事不要混。**HTTPS 只包住短 HTTP（REST）；长连接仍按小册分 Web / App。** App 用 HTTPS 拿 Token，和它再用 TCP 聊天 **不冲突**。不要把 App 长连接改成 WSS。

```
所有客户端
  ├─ HTTPS REST     Token / Router / 用户     （小册 HTTP，我们 HTTPS）
  ├─ Web  长连接 ── WSS ──► WGateway
  └─ App  长连接 ── TCP(+TLS) ──► TGateway     （小册就是 TCP；公网套 TLS，无 HTTP 升级）
```

| 流量 | 小册 | 我们 |
|---|---|---|
| Royal / Router 等 REST | HTTP | **HTTPS**（Cloudflare 证书）。SSL：**Full 或 Full (strict)**，不用 Flexible |
| Web 长连接 | WebSocket → WGateway | 本机 `ws://`；公网 **WSS** → WGateway。橙云可转 |
| App 长连接 | **TCP** → TGateway | **仍走 TGateway / `kim-tcp`**。本机明文 TCP（已有 echo）；公网以后 **TCP+TLS**（加密外壳，无 HTTP 升级） |
| 网关 ↔ Chat | TCP | TCP（内网，`kim-tcp`） |

Cloudflare 橙云 **不转发任意 TCP**：这只决定公网 **怎么暴露** TGateway（以后灰云，或独立 IP:port + TLS），**不**决定 App 改走 WSS。

本阶段（M1b+M2）因此：

- 实现 `kim-ws`，因为 **Web 需要它**，并证明第二种 `Conn`。
- **保留** `kim-tcp`：App / TGateway，以及网关↔Chat。不要求 App 走 WS。
- 本机 Demo 的 `pkt-client` 用 `ws://`，扮演 **Web 客户端**。App / TGateway 的 TCP 电线仍是 `kim-tcp`；回声验收在 crate 测试，没有独立 echo 二进制。
- 公网 TGateway 的 TLS **后做加密外壳**，不是「用 WSS 代替 App 长连接」。
- `kim.ainexc.com` 以后和 `minos.ainexc.com` 共存于现有反向代理。**部署发生在本机跑通之后**，本阶段不上 VPS。

---

## 2. 本阶段目标与非目标

### 2.1 目标

**M1b — `kim-ws`（Web / WGateway 的电线）**

- 实现 `kim_core::Conn`：`read_frame` / `write_frame` / `flush` / `shutdown`。Web 长连接需要这条线；同时证明第二种传输履行同一合同。
- **`kim-tcp` 继续承担 App / TGateway 以及网关↔Chat。** 本阶段不把 App 改成 WS，不套 TLS，不写业务 `if command`，**不**把写侧 Mutex 改成 mpsc。允许小改：`TcpClient::read(&self)`，以及 `TcpServer::shutdown` 用 closed+`notify_one`（见 §6.3、PR3.5）。
- HTTP Upgrade 一次，之后是 RFC 6455 帧；把 WS opcode 映射到现有 `kim_core::OpCode`。
- `WsServer` 复用 `Channel::pair` + `ChannelMap` + 两专员。禁止在 `kim-ws` 里再写一套连接表或写锁。
- 同一份 `EchoHandler` 在 `kim-ws/tests/echo.rs` 上跑通回声。TCP echo 在 `kim-tcp/tests/echo.rs`。
- 本机 `ws://127.0.0.1:<port>/` 即可。库：`fastwebsockets`（帧级、`after_handshake`）；`hyper` 只做 Upgrade，不做业务。

**M2 — 业务包 + 容器 + 静态 Naming**

- 新 crate `kim-protocol`：Magic + BasicPkt + LogicPkt（protobuf Header）。包只活在 Binary **payload** 里。
- 新 crate `kim-naming`：`Naming` trait + `StaticNaming`（读配置，不是 Consul）。
- 新 crate `kim-container`：托管 `Server`；对每个依赖服务 **拨号全部实例**；Young → Adult 之后才 `Forward`。
- Demo：本机一个假网关进程 + 一个假 Chat 进程。客户端发 LogicPkt，收到 **相同 sequence** 的 Response；BasicPkt ping **绝不出现在 Chat 日志**。

### 2.2 非目标（M2 当时禁止；登录已另文落地）

- 登录 JWT、互踢、会话 Redis（**M3 已做**，见 [link-layer-login.md](link-layer-login.md)；本文不重复）。
- Consul / etcd / K8s 服务发现。
- VPS、Cloudflare 接入、`kim.ainexc.com` 上线。
- 给公网 TGateway 套 TLS（加密外壳后做；本机明文 TCP 已有，不要用 WSS 顶替 App）。
- 1:1 聊天、群聊、离线、Royal CRUD。
- 把 `if command == login` 写进 `TcpServer` / `WsServer`。
- 把 IM 头塞进 HTTP Upgrade 的 URL query / Header。
- 自定义 WebSocket opcode 承载业务（浏览器只能发 text/binary）。
- 改造 `TcpClient` 写路径为 mpsc（记下即可）。**允许**把 `read` 改成 `&self`（读半边也进 Mutex），见 §6.3。
- 用 axum 的 WebSocket extractor 当 IM 网关（会丢掉 Conn/Channel 专员模型）。

---

## 3. 本阶段总架构

### 3.1 进程与电线（本机 Demo）

```
pkt-client（Web 客户端）        fake-gateway（WGateway）          fake-chat
(examples/pkt-client)          (examples/fake-gateway)           (examples/fake-chat)
                               WsServer :8001                    TcpServer :8002
                               ChannelMap（Web 客户端）           ChannelMap（网关连进来）
                               Container                         Container（无 deps）
                                 └─ TcpClient × N Chat

  ws://127.0.0.1:8001/                 内网 TCP 127.0.0.1:8002
  ① HTTP Upgrade 101                   ③ InnerHandshakeReq（第一帧）
  ② 第一帧 login.signin + JWT          ④ 之后只走 LogicPkt
  ⑤ Binary payload = Magic+Pkt
     （echo / e2e_echo 仍用第一帧名字；见 link-layer-login.md）

App / TGateway 路径（已有，本阶段保持）：
  echo-client ── TCP ──► echo-server :8000
```

两条完全不同的「握手」（Demo 的 **Web** 线）：

| 谁连谁 | 传输 | 握手 |
|---|---|---|
| Web 客户端 → WGateway | WebSocket | HTTP Upgrade，然后第一帧 LogicPkt `login.signin` + JWT。echo 路径仍用名字字符串。 |
| 网关 → Chat | TCP（`kim-tcp`） | TCP 连上后第一帧 Binary = protobuf `InnerHandshakeReq { service_id }`。Chat 的 `Acceptor` 读出来当 channel_id。 |
| App 客户端 → TGateway | TCP（`kim-tcp`） | `examples/fake-tgateway` `:8003`，与 WGateway 共用 `run_gateway`。公网以后在这条线上套 TLS，无 HTTP 升级。 |

服务之间 **只允许 TCP**。不要给 Chat 再开一个 WS 口。  
Web 进网关走 WS；App 进网关走 TCP。Demo 的 `pkt-client` 只扮演 Web，**不要求 App 改用 WS**。

### 3.2 一包业务数据怎么走（LogicPkt）

```
pkt-client
  │  WS Binary 帧，payload =
  │  MagicLogicPkt + header_len + Header(protobuf) + body
  ▼
fake-gateway 读专员
  │  Channel 已把 WS Ping/Pong/Close 吃掉
  │  MessageListener 拿到 payload
  │  kim_protocol::read() 认出 LogicPkt
  │  填 Header.channel_id = agent.id()
  │  填 Meta dest.server = "wg-1"（本网关）
  │  command="chat.demo.echo" → 前缀 "chat"
  ▼
container.forward("chat")
  │  只在 Adult 实例里 HashSelector(channel_id)（列表按 service_id 排序）
  │  TcpClient.send(完整 payload)
  ▼
fake-chat 读专员
  │  MustReadLogicPkt（不是 Logic 就 warn 丢掉）
  │  command == chat.demo.echo → Flag=Response，原 sequence，原 body
  │  Meta dest.server="wg-1"  dest.channels="wg-1_alice_N"
  │  container.push("wg-1", pkt)   // 内部 server.push，Handler 不直接摸 Server
  ▼
fake-gateway 的 Client 读循环（容器内部）
  │  校验 dest.server == 自己
  │  按 dest.channels 拆开，ChannelMap.get("wg-1_alice_N").push
  ▼
pkt-client 收到 LogicPkt，sequence 相同，Flag=Response
```

### 3.3 一包心跳怎么走（BasicPkt ping）——不到 Chat

```
pkt-client  BasicPkt ping
    │
    ▼
fake-gateway  receive
    │  认出 MagicBasicPkt，code=1
    │  立刻 agent.push(BasicPkt pong)
    │  return          ← 不调用 Forward
    ▼
pkt-client  收到 pong

fake-chat 的日志里不允许出现这条 ping。
```

### 3.4 新 crate 在仓库里的位置

```
im/
  crates/kim-core          已有。本阶段不改 `channel.rs`（超时语义见 §4.7）
  crates/kim-tcp           已有。App / TGateway + 网关↔Chat。不改业务分支、不套 TLS；PR3.5：`TcpClient::read(&self)` + `TcpServer` shutdown 不丢唤醒
  crates/kim-ws            新增。Web / WGateway 的第二种 Conn
  crates/kim-protocol      新增。Magic / BasicPkt / LogicPkt
  crates/kim-naming        新增。Naming + StaticNaming
  crates/kim-container     新增。托管 Server、拨号、Forward/Push
  examples/echo-*          已有。TCP 回声 = App/TGateway 路径，回归用
  examples/ws-echo-*       新增。同一 EchoHandler，Web/WS 传输
  examples/fake-gateway    新增。M2 假 **WGateway**（Web）
  examples/fake-chat       新增。M2 假 Chat
  examples/pkt-client      新增。Web 客户端：发 ping + LogicPkt（ws://）
  proto/pkt.proto          新增。给 kim-protocol 的 build.rs 用
                           （也可以放 crates/kim-protocol/proto/pkt.proto）
```

原则仍然是：**换传输只加 `Conn` 实现，不改业务。** 长连接按小册双网关，HTTPS 只包 REST。

```
Web  ── kim-ws  ──► WGateway
App  ── kim-tcp ──► TGateway          （本机明文；公网以后 TCP+TLS）
网关 ── kim-tcp ──► Chat

业务 Handler ──► kim-core（表 + 信箱 + 两专员）──► kim-ws 或 kim-tcp ──► 内核 TCP
                     ▲
                     │  payload 里的意思由 kim-protocol 解释
                     │  找谁、转给谁由 kim-container + kim-naming 决定
```

`WsServer` / `TcpServer` 不知道 `chat.demo.echo` 是什么。

---

## 4. 详细设计：M1b `kim-ws`

### 4.1 依赖与 crate 清单

`crates/kim-ws/Cargo.toml`：

```toml
[package]
name = "kim-ws"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "KIM 通信层的 WebSocket 实现：HTTP Upgrade 之后的 RFC6455 帧"

[dependencies]
async-trait.workspace = true
bytes.workspace = true
kim-core.workspace = true
tokio.workspace = true
tracing.workspace = true
http = "1"
hyper = { version = "1", features = ["http1", "server", "client"] }
hyper-util = { version = "0.1", features = ["tokio"] }  # 只要 TokioIo；不要 server-auto
http-body-util = "0.1"
fastwebsockets = { version = "0.10", features = ["upgrade", "unstable-split"] }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
tracing-subscriber.workspace = true
```

工作区 `Cargo.toml` 增加：

```toml
members = [
    "crates/kim-core",
    "crates/kim-tcp",
    "crates/kim-ws",
    "crates/kim-protocol",
    "crates/kim-naming",
    "crates/kim-container",
    "examples/echo-server",
    "examples/echo-client",
    "examples/ws-echo-server",
    "examples/ws-echo-client",
    "examples/fake-gateway",
    "examples/fake-chat",
    "examples/pkt-client",
]

[workspace.dependencies]
# 已有 bytes / thiserror / tokio / tracing / async-trait / kim-core / kim-tcp
kim-ws = { path = "crates/kim-ws" }
kim-protocol = { path = "crates/kim-protocol" }
kim-naming = { path = "crates/kim-naming" }
kim-container = { path = "crates/kim-container" }
prost = "0.13"
```

**不要**把 axum 的 `WebSocketUpgrade` 当网关。`fastwebsockets` 的 `upgrade` feature 已经会用 hyper 做 101 Switching Protocols。axum 若版本和 `with_axum` 对不齐，直接走 hyper，少一层。

### 4.2 文件布局

```
crates/kim-ws/src/lib.rs      再导出
crates/kim-ws/src/opcode.rs   fastwebsockets::OpCode ↔ kim_core::OpCode
crates/kim-ws/src/conn.rs     WsConn / WsReadHalf / WsWriteHalf
crates/kim-ws/src/server.rs   WsServer（hyper accept + Upgrade + Channel::pair）
crates/kim-ws/src/client.rs   WsClient / WsIdentityDialer
crates/kim-ws/tests/echo.rs   与 kim-tcp/tests/echo.rs 同构
```

### 4.3 HTTP Upgrade（只发生一次）

服务端监听明文 TCP，用 HTTP/1.1 读请求。合法 Upgrade 才继续；否则 400。

客户端请求（本机）：

```
GET / HTTP/1.1
Host: 127.0.0.1:8001
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: <24 字节 base64>
Sec-WebSocket-Version: 13
```

服务端成功响应：

```
HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: <RFC6455 计算>
```

实现要点（写在 `server.rs`），对齐 `fastwebsockets` 0.10 的 upgrade 测试，**不要**再包一层 `TokioIo`：

```
TcpListener.accept 得到 TcpStream
    │
    ▼
hyper::server::conn::http1::Builder::new()
    .serve_connection(TokioIo::new(stream), service_fn(handle))
    .with_upgrades()     ← 没有这一步，101 返回了也不会完成升级

handle(mut req):
    path 不是 / 也不是 /ws → 404
    不是 Upgrade → 400
    let (response, fut) = fastwebsockets::upgrade::upgrade(&mut req)?;
    spawn {
        let mut ws = fut.await?;
        // 类型已经是 WebSocket<TokioIo<hyper::upgrade::Upgraded>>，Role::Server
        // 禁止再 TokioIo::new(ws) / after_handshake
        ws.set_auto_pong(false);
        ws.set_auto_close(false);
        ws.set_writev(true);
        ws.set_max_message_size(1024 * 1024);  // 库默认 64MiB，必须改成与 TCP 相同
        let mut conn = WsConn { ws };
        acceptor / Channel::pair / read_loop   // 见 4.8
    };
    Ok(response)   // 必须作为 service 的返回值，hyper 才会写出 101
```

**此后不再解析 HTTP。** 业务包不准出现在 URL、Query、Cookie、自定义请求头里。

路径：同时接受 `GET /` 和 `GET /ws`，其它 path 返回 404。小册用 `/`；`/ws` 方便以后反代按路径分流。

### 4.4 拆成读写半边（对齐 `TcpConn::into_split`）

`fastwebsockets` 开 `unstable-split`：

`UpgradeFut::await` 已经得到 `WebSocket<TokioIo<Upgraded>>`（`Role::Server`）。握手阶段整条流给 `Acceptor`，再 `split`：

```
fut.await → ws: WebSocket<TokioIo<Upgraded>>
    │  set_auto_pong(false) / set_auto_close(false)
    │  set_writev(true) / set_max_message_size(1MiB)
    ▼
WsConn { ws }                    ← 握手阶段整条用
    │  acceptor.accept(&mut conn)  读第一帧 Binary 当 id
    ▼
conn.into_split()
    │  内部：ws.split(tokio::io::split)
    ▼
(WsReadHalf, WsWriteHalf) ──► Channel::pair(id, reader, writer, opts)
```

不要自己再调 `WebSocket::after_handshake`。那条 API 只在「已经拿到裸 TCP、自己完成了 HTTP 握手」时才用。

`Role::Server`：读来的客户端帧带 mask，`read_frame` 会解 mask；写出的帧不 mask。  
`Role::Client`（`WsClient`）：写出的帧必须 mask，库会做。

**禁止** `set_auto_pong(true)`。库若在读路径里直接写 Pong，就和写专员抢同一条流，两专员模型被打破。

### 4.5 帧映射

| RFC 6455 / fastwebsockets | `kim_core::OpCode` | payload | Channel 读循环怎么做（已有代码，不要改语义） |
|---|---|---|---|
| Binary (0x2) | `OpCode::Binary` | 原样 | `MessageListener::receive` |
| Text (0x1) | `OpCode::Text` | 原样 | 同样进 `receive`（**没有 opcode**。echo 当字节回声；M2 网关按 Magic 解，解失败就 warn，见 6.4） |
| Ping (0x9) | `OpCode::Ping` | 原样或空 | 自动 `WriteOp::Pong`，业务看不见 |
| Pong (0xa) | `OpCode::Pong` | 原样或空 | debug 日志 |
| Close (0x8) | `OpCode::Close` | 原因字节 | 结束读循环 |
| Continuation (0x0) | `OpCode::Continuation` | — | 忽略 |

写出时 **FIN 一律 true**，不把业务包拆成多帧。小册同样如此。单帧上限与 TCP 对齐：`1 * 1024 * 1024` 字节。必须在 split 前 `set_max_message_size(1024 * 1024)`（库默认 64MiB）。超过则断开。映射到 `Error::FrameTooLarge` 若库错误能转；否则 `Error::other`。

映射函数放 `opcode.rs`，双向 `From`。未知 WS opcode → `Error::other("unknown ws opcode")`。

应用 BasicPkt ping **仍然需要**：浏览器 JS **不能**发、也看不见 RFC 6455 的 ping/pong。Channel 的 `OpPing` 给原生 TCP/WS 客户端和网关保活用；网页心跳走 BasicPkt（见第 6 节）。

### 4.6 `Conn` 实现要点

`WsReadHalf::write_frame` 返回 `Error::other("read half cannot write")`。  
`WsWriteHalf::read_frame` 返回 `Error::other("write half cannot read")`。  
与 `kim-tcp` 的半边完全对称。

`write_frame`：

```
fastwebsockets::Frame::new(true /* fin */, mapped_opcode, None, payload.into())
writer.write_frame(frame).await
```

`shutdown`：写一个 Close 帧再 shutdown 写半边。

`flush`：若库没有单独 flush，返回 `Ok(())`（帧写路径已经写出）。

### 4.7 取消安全（和现有 Channel 超时）

`fastwebsockets::read_frame` **不是 cancel-safe**：半帧中间取消后再用 **同一个** reader 继续读，会丢字节。

现有 `ChannelReadLoop`：

```
timeout(read_wait, reader.read_frame())
超时 → return Err(Error::Closed)   // 整个 ReadLoop 被 drop，Conn 一起扔掉
```

这是安全的：**超时必须结束这条连接**，禁止取消之后继续用同一个 `Conn`。本阶段 **不必改** `channel.rs` 逻辑；在 `kim-ws/src/conn.rs` 文件头写清这条约束。

### 4.8 `WsServer` 生命周期（对齐 `TcpServer`）

公开 API：

```rust
impl WsServer {
    pub async fn bind(listen: impl ToSocketAddrs) -> Result<Self, Error>;
    pub fn local_addr(&self) -> SocketAddr;
    pub fn channel_map(&self) -> ChannelMap;
}

#[async_trait]
impl Server for WsServer { /* 与 TcpServer 相同的 set_* / start / push / shutdown */ }
```

`start` 伪流程（HTTP 部分见 4.3；下面是 **每个** `fut.await` 之后）：

```
ws = fut.await                          // 已是 WebSocket<TokioIo<Upgraded>>
set_auto_pong(false) / set_auto_close(false) / set_max_message_size(1MiB)
WsConn { ws }
timeout(login_wait, acceptor.accept(&mut conn))
      失败 → 写 Close 帧，shutdown
channels.contains(id)? 重复 → Close "channelId is repeated"
(reader, writer) = conn.into_split()
(channel, read_loop) = Channel::pair(...)
channels.add(channel)                   // 只握表锁插入
read_loop.run(messages)
      结束 → channels.remove → StateListener::disconnect
```

`push(channel_id, payload)`：`channels.get` clone，立刻放锁，`channel.push`。和 TCP 相同。

`shutdown`：`closed.store(true)` + `notify_waiters()` + `notify_one()`。`start` 的 HTTP accept 循环进 `select` 前若 `closed` 则返回。只 `notify_waiters` 会在「立刻 shutdown」时丢给还没 wait 的 accept 循环。

默认 `Acceptor`：与 `TcpServer` 一样生成 `ch-{seq}`。echo 会换成读第一帧。

### 4.9 `WsClient` / `WsIdentityDialer`

`WsClient` 镜像 `TcpClient` 的 inherent 方法（`TcpClient` 目前也没有 `impl Client for TcpClient`，不要突然改合同）：

- `connect(url)`：`ws://` 明文 TCP 再 Upgrade；`wss://` 先 TLS 再同一套 Upgrade。`WsServer` 仍只听明文，公网 WSS 由反代终止。
- 用 hyper 客户端发 Upgrade，`fastwebsockets` handshake 之后 `Role::Client`。
- 拆读写半边；心跳任务发 `OpCode::Ping`（和 TcpClient 一样）。写侧本阶段可以用 `Mutex<WsWriteHalf>`，与 TcpClient 对齐，不强制 mpsc。
- `send(payload)`：写 Binary 帧。
- `read()`：吞掉 Ping/Pong/Close，把 Binary/Text 交给调用方。

`WsIdentityDialer`：Upgrade 成功后立刻 `write_frame(Binary, id)`，与 `IdentityDialer` 同构。

### 4.10 例子

`examples/ws-echo-server`：把 `examples/echo-server` 的 `EchoHandler` 原样拷过来，`TcpServer::bind` 换成 `WsServer::bind`。默认听 `127.0.0.1:8001`。

`examples/ws-echo-client`：`WsClient` + `WsIdentityDialer`，连 `ws://127.0.0.1:8001/`，发 5 条 `hello {i}`（与 TCP echo-client **相同字节**，带空格：`hello 0` … `hello 4`），打印 `hello {i} from server`。

`Cargo.toml`：所有新 example 包都设 `publish = false`，与现有 echo-* 一致。

不要把这两个例子写成网关。它们的唯一任务是：**同一套 EchoHandler，Web 的第二种电线。** TCP echo 继续当 App 路径，不要删。

---

## 5. 详细设计：M2 `kim-protocol`

### 5.1 两层不要记混

```
内核 TCP 段          操作系统的事，我们不管
    ▲
kim-tcp 应用层帧      opcode 1B | len 4B LE | payload     （App / TGateway / 网关↔Chat）
kim-ws RFC6455 帧    opcode + payload                      （Web / WGateway）
    ▲
payload 里的业务包    Magic 4B + BasicPkt 或 LogicPkt        ← 本 crate
```

业务包：

- **永远**放在通信层 Binary 帧的 payload。
- **绝不**放进 HTTP Upgrade 的 URL / Header。
- **绝不**用自定义 WS opcode（浏览器发不出去）。

TCP 粘包由 `kim-tcp` 解决；WS 自带帧边界。`kim-protocol` 假定传入的是 **一整块 payload**，不再做流式拆包。网关 `receive` 里：`kim_protocol::read(&payload[..])`。

### 5.2 Magic（小册原值）

```rust
// crates/kim-protocol/src/magic.rs
pub type Magic = [u8; 4];

/// 逻辑协议
pub const MAGIC_LOGIC_PKT: Magic = [0xc3, 0x11, 0xa3, 0x65];
/// 基础协议
pub const MAGIC_BASIC_PKT: Magic = [0xc3, 0x15, 0xa7, 0x65];
```

比较用 **四个字节全等**，不要先 `u32::from_be_bytes` 再比——避免和 BasicPkt 的小端混在一起。

### 5.3 BasicPkt（网关本地心跳）

布局（小端，小册第 14 章「小头字节序」）：

```
| MagicBasicPkt 4B | code u16 LE | length u16 LE | body N 字节 |
```

```rust
pub const CODE_PING: u16 = 1;
pub const CODE_PONG: u16 = 2;

pub struct BasicPkt {
    pub code: u16,
    pub body: Bytes, // len = body.len()，encode 时写 length
}
```

空 body 时整包 8 字节：`4 magic + 2 code + 2 length`。

规则：

- 网关 `receive` 见 `CODE_PING` → `agent.push(marshal(BasicPkt { code: CODE_PONG, body: empty }))`，**return**。
- 不 Forward。Chat 进程若解码到 BasicPkt：打 `warn` 并丢弃（验收时用这条证明 ping 没到 Chat）。
- `length > 4096` 拒绝（心跳不该有大 body）。

### 5.4 LogicPkt Header（protobuf）

文件：`crates/kim-protocol/proto/pkt.proto`  
`build.rs`：`prost_build` 编译进 `kim-protocol`。

```protobuf
syntax = "proto3";
package kim.pkt;

enum Flag {
  Request = 0;   // 客户端发起
  Response = 1;  // 处理完回给发送方
  Push = 2;      // 服务端主动推（本阶段 Demo 不用，字段先留着）
}

enum Status {
  Success = 0;
  InvalidPacket = 1;
  CommandNotFound = 2;
  ServiceUnavailable = 3; // 没有 Adult 的 Chat
  SystemException = 99;
}

message Meta {
  string key = 1;
  string value = 2;
}

message Header {
  string command = 1;      // "service.action"，如 chat.demo.echo
  string channelId = 2;    // 连接临时身份证，不是用户名
  uint32 sequence = 3;     // 发送方生成；响应必须原样带回
  Flag flag = 4;
  Status status = 5;
  string dest = 6;         // 以后的用户/群；本阶段可空
  uint32 bodyLength = 7;   // body 字节数
  repeated Meta meta = 8;  // 容器寻址：dest.server / dest.channels
}

message InnerHandshakeReq {
  string serviceId = 1;
}
```

**为什么有 `meta`：** 小册第 14 章表格写了 `bodyLength`，第 15 章真正的 Header 还有 `Meta`。容器下行靠：

- `dest.server`：这条回包要送到哪一个网关实例（Chat 的 ChannelMap 用网关 service_id 当 id）。
- `dest.channels`：网关再拆成多个 channel_id（逗号分隔）。登录 Demo 里是 `wg-1_alice_N`，不是账号字符串。

没有 `meta`，Chat 就不知道回给哪条网关连接。本阶段 **带上**，值只有这两个 key。

常量（`kim-protocol/src/wire.rs`）：

```rust
pub const META_DEST_SERVER: &str = "dest.server";
pub const META_DEST_CHANNELS: &str = "dest.channels";

pub const SN_WGATEWAY: &str = "wgateway";
pub const SN_CHAT: &str = "chat";

pub const CMD_DEMO_ECHO: &str = "chat.demo.echo";
```

`command` 路由：

```rust
pub fn service_name(command: &str) -> &str {
    command.split_once('.').map(|(s, _)| s).unwrap_or("default")
}
// "chat.demo.echo" → "chat"
// "login.signin"  → "login"（本阶段客户端不要发这个）
// "nopath"        → "default"（Forward 会失败，网关可回 Status::CommandNotFound）
```

### 5.5 LogicPkt 字节布局

```
| MagicLogicPkt 4B | header_len u32 BE | Header protobuf | body |
```

- `header_len`：**大端**。小册 Web SDK（第 23 章）用 `writeInt32BE`。Magic 是原始 4 字节；长度用网络序，以后写 JS 客户端不会踩坑。
- `body` 长度以 Header.`bodyLength` 为准。解码时：读完 Header 再读 `bodyLength` 字节。不够 → `Error::Incomplete`；多出来的尾部忽略并 `debug` 日志。
- **不要**再额外写 4 字节 payload_len。小册 JS 那样做是因为当时 Header 没有 `bodyLength`。我们按第 14 章表格以 Header 字段为准，去掉重复。

`LogicPkt`：

```rust
pub struct LogicPkt {
    pub header: crate::pkt::Header, // prost 生成
    pub body: Bytes,
}

impl LogicPkt {
    pub fn new(command: impl Into<String>, sequence: u32, body: Bytes) -> Self { /* 填 bodyLength */ }
    pub fn service_name(&self) -> &str { service_name(&self.header.command) }
    /// 按 key **替换**：已有同 key 先删光再插入一条。禁止 append（否则
    /// dest.server 会出现两份，网关 del 一次还漏一条给客户端）。
    pub fn set_meta(&mut self, key: &str, value: &str);
    pub fn get_meta(&self, key: &str) -> Option<&str>; // 第一条；替换语义下最多一条
    pub fn del_meta(&mut self, key: &str);             // 删掉该 key 的全部条目
}
```

`sequence`：客户端从 1 自增。Chat 响应必须复制请求的 sequence、command、channel_id，改 `flag=Response`、`status=Success`。

### 5.6 编解码 API

```rust
pub enum Packet {
    Basic(BasicPkt),
    Logic(LogicPkt),
}

pub fn read(buf: &[u8]) -> Result<Packet, ProtocolError>;
pub fn marshal(pkt: &Packet) -> Bytes;

pub fn read_logic(buf: &[u8]) -> Result<LogicPkt, ProtocolError>; // Chat 用：不是 Logic 就报错
```

`read`：前 4 字节对 Magic，其余按类型解。未知 Magic → `ProtocolError::BadMagic`。

`InnerHandshakeReq` **不是** Magic 包。它是通信层第一帧的 **裸 protobuf**（和小册一致）：`prost::Message::encode_to_vec` 作为 Binary payload。不要给它套 Magic，否则和 echo 的「第一帧是名字」两条握手路径搅在一起。

### 5.7 `kim-protocol` 的 Cargo 与 build.rs

```
crates/kim-protocol/Cargo.toml
crates/kim-protocol/build.rs
crates/kim-protocol/proto/pkt.proto
crates/kim-protocol/src/lib.rs
crates/kim-protocol/src/magic.rs
crates/kim-protocol/src/basic.rs
crates/kim-protocol/src/logic.rs
crates/kim-protocol/src/error.rs
crates/kim-protocol/src/wire.rs
```

```toml
[dependencies]
bytes.workspace = true
prost.workspace = true
thiserror.workspace = true

[build-dependencies]
prost-build = "0.13"
protobuf-src = "2"   # 自带 protoc，干净机器上 cargo test -p kim-protocol 不依赖 PATH
```

`build.rs` 开头：

```rust
std::env::set_var("PROTOC", protobuf_src::protoc());
prost_build::Config::new()
    .compile_protos(&["proto/pkt.proto"], &["proto"])
    .unwrap();
```

没有 `protobuf-src` 时才会去找系统 `protoc`（macOS：`brew install protobuf`）。默认走 vendored，避免第一次编译卡死。

单测（`src/basic.rs` / `src/logic.rs` 的 `#[cfg(test)]`）：

- Basic ping/pong 往返；空 body 总长 8。
- Logic 往返：command、sequence、body 不变；`bodyLength` 等于 body。
- 错误 Magic 失败。
- `service_name("chat.user.talk") == "chat"`。
- 半截 buffer → Incomplete。

---

## 6. 详细设计：Naming + Container + Demo

### 6.1 词：服务 vs 实例

- **服务（service）**：角色名，如 `chat`、`wgateway`。
- **实例（instance）**：一次具体运行，有 `service_id` + 可拨号地址。Naming 登记的是实例。

本机两个进程和一个两台 VPS，对网关是同一件事：拿到地址列表，**每个都拨号**。

### 6.2 `kim-naming`

```
crates/kim-naming/src/lib.rs
crates/kim-naming/src/registration.rs
crates/kim-naming/src/naming.rs
crates/kim-naming/src/static_naming.rs
```

```rust
#[derive(Clone, Debug)]
pub struct DefaultRegistration {
    pub service_id: String,
    pub service_name: String,
    pub protocol: String,         // "tcp" | "ws"；服务之间必须 "tcp"
    pub public_address: String,   // 拨号用，本机 127.0.0.1
    pub public_port: u16,
    pub tags: Vec<String>,
    pub meta: HashMap<String, String>,
}

impl DefaultRegistration {
    pub fn dial_url(&self) -> String {
        format!("{}:{}", self.public_address, self.public_port)
    }
}

#[async_trait]
pub trait Naming: Send + Sync {
    async fn find(&self, service_name: &str, tags: &[&str]) -> Result<Vec<DefaultRegistration>, Error>;
    async fn subscribe(
        &self,
        service_name: &str,
        callback: Arc<dyn Fn(Vec<DefaultRegistration>) + Send + Sync>,
    ) -> Result<(), Error>;
    async fn unsubscribe(&self, service_name: &str) -> Result<(), Error>;
    async fn register(&self, service: DefaultRegistration) -> Result<(), Error>;
    async fn deregister(&self, service_id: &str) -> Result<(), Error>;
}
```

错误类型放 `kim-naming` 自己的 `thiserror`，或复用 `kim_core::Error::Other`。推荐独立 `kim_naming::Error`，Container 再转换。

**`StaticNaming`（对齐小册：Find = 快照，Subscribe = 以后的变更）**：

- 构造：`StaticNaming::from_slice(Vec<DefaultRegistration>)`。**不**在 crate 里读 TOML。
- `find`：当前快照，过滤 `service_name`（tags 本阶段可空）。
- `subscribe`：**不**立刻回调当前列表。只在之后列表变化时回调。小册 Consul 的 Subscribe 是 watch，不是 dump。
- `#[cfg(test)] insert(reg)`：写入 map，然后对这个 `service_name` 的订阅者回调 **插入后的完整列表**。这是 Young 窗口单测的「新实例出现」路径。
- `register` / `deregister`：内存 HashMap；**不**发 HTTP 到 Consul。Demo 可以不 register——对端地址写在网关配置的 `[[services]]` 里，由网关进程自己 `from_slice`。

配置里 **启动时就已经有的** 实例只走 `find`，Container 标 Adult。`insert()` 只走 Subscribe，标 Young，等 `adult_delay`。

TOML **只在 examples 里解析**，不放进 `kim-naming`。示意结构（约 15 行）：

```rust
// examples/fake-gateway/src/config.rs
#[derive(Deserialize)]
struct File {
    #[serde(rename = "self")]
    this: SelfSection,
    #[serde(default)]
    services: Vec<DefaultRegistration>, // public_address + public_port
}
#[derive(Deserialize)]
struct SelfSection {
    service_id: String,      // InnerHandshakeReq 和 dest.server 都用这个，网关固定 "wg-1"
    service_name: String,    // "wgateway"
    listen: String,          // bind，如 "127.0.0.1:8001"；不是 dial_url
    protocol: String,        // "ws"
}
// listen → TcpListener / WsServer::bind
// services → StaticNaming::from_slice
// identity.service_id = this.service_id（"wg-1"）
// identity.public_address 可空：空则 Container 跳过 naming.register
```

`examples/fake-gateway/config.toml`：

```toml
[self]
service_id = "wg-1"
service_name = "wgateway"
listen = "127.0.0.1:8001"
protocol = "ws"

[[services]]
service_id = "chat-1"
service_name = "chat"
protocol = "tcp"
public_address = "127.0.0.1"
public_port = 8002
```

`examples/fake-chat/config.toml`：

```toml
[self]
service_id = "chat-1"
service_name = "chat"
listen = "127.0.0.1:8002"
protocol = "tcp"
# 没有 public_address → 不 register。网关侧 [[services]] 已经写死 chat 地址。
```

Chat 本阶段 **没有 deps**。网关 deps = `["chat"]`。解析用 `serde` + `toml = "0.8"`，只挂在 example 的 Cargo.toml。

### 6.3 `kim-container`

Rust **不用**小册那种进程级单例 `container.Default()`。一个进程一个 `Arc<Container>`，测试能并排起两套。Go 单例把 Handler↔Container 的环藏起来了；这里用两阶段装配。**禁止** `static OnceLock<Container>`；`Server` 句柄用字段级 `OnceLock` 是另一回事（见 6.3.4）。

```
crates/kim-container/src/lib.rs
crates/kim-container/src/container.rs
crates/kim-container/src/client_map.rs
crates/kim-container/src/selector.rs
crates/kim-container/src/dialer.rs
```

#### 6.3.1 公开 API

```rust
pub struct ContainerOpts {
    pub naming: Arc<dyn Naming>,
    pub identity: DefaultRegistration,   // service_id 如 "wg-1" / "chat-1"
    pub dialer: Arc<dyn TcpDialer>,      // 服务间只 TCP
    pub deps: Vec<String>,
    pub adult_delay: Duration,           // 默认 10s；Demo 0；单测 50ms
    pub selector: Arc<dyn Selector>,     // 默认 HashSelector
}

impl Container {
    pub fn new(opts: ContainerOpts) -> Arc<Self>;
    /// start 之前调用一次（同步）。server 必须已经 set_* listener。
    /// 实现：`OnceLock::set`，不是 `tokio::sync::Mutex`。
    pub fn attach_server(&self, server: Arc<dyn Server + Send + Sync>);
    /// 拨号 deps、可选 register、spawn server.start()，然后等 shutdown()。
    /// **不**在库里等 ctrl-c。
    pub async fn start(&self) -> Result<(), Error>;
    pub async fn shutdown(&self) -> Result<(), Error>;
    /// 网关上行：填 dest.server = 本实例 identity.service_id，再发给 Adult Chat。
    pub async fn forward(&self, service_name: &str, pkt: LogicPkt) -> Result<(), Error>;
    /// Chat 下行：把 LogicPkt 推到「连进来的网关 Channel」。
    /// channel_id = gateway_id（如 "wg-1"）。Handler 事先填好 dest.channels。
    pub async fn push(&self, gateway_id: &str, pkt: LogicPkt) -> Result<(), Error>;
}
```

`kim_core::Server` 只有 `Send`，没有 `Sync`。Container **不要改 trait**，字段写成 `Arc<dyn Server + Send + Sync>`（`TcpServer` / `WsServer` 实际都是 Sync）。`tokio::spawn(server.start())` 才能过编译。

#### 6.3.2 两阶段装配（examples 照抄）

`Server::set_*` 要 `&mut self`，不能先 `Arc` 再设 listener。Handler 又要 `Arc<Container>` 才能 `forward` / `push`。顺序：

```
1. let mut server = WsServer::bind(&cfg.listen).await?;   // 或 TcpServer（Chat）
2. let container = Container::new(ContainerOpts { … });    // 还没有 server
3. let handler = GatewayHandler { container: container.clone() };
4. server.set_acceptor(handler.clone());
   server.set_message_listener(handler.clone());
   server.set_state_listener(handler);
5. container.attach_server(Arc::new(server));              // 此后 Server 不可再 set_*
6. container.start().await?;                               // 拨号 + spawn server.start + 等 shutdown
```

Chat 进程同样 6 步，Handler 调 `container.push("wg-1", resp)`，deps 为空。

`examples/fake-*` 的 `main`：`tokio::signal::ctrl_c` 之后 `container.shutdown()`。单测断言完也调 `shutdown()`，不要发 SIGINT。允许 `spawn(start)` 后立刻 `shutdown()`，中间不必 sleep：Container 用 closed+Notify；**还要**再调一次 `server.shutdown()`（见 6.3.5），因为 `TcpServer` 自己的 accept 循环是另一把 Notify。

`shutdown()`：置 `closed`、`server.shutdown()`、unsubscribe、deregister、关掉 ClientMap 里的 TcpClient、唤醒 `start`。

#### 6.3.3 `TcpClient` 必须能并发 `send` + `read`（PR3.5）

今天的 `read(&mut self)` + 私有 `reader` 让 `Arc<TcpClient>` 编不过；`Mutex<TcpClient>` 会在 `read` 等 socket 时卡住 `send`，Demo 死锁。

**选定形状**（写侧 Mutex 保留，不改 mpsc）：

```rust
// crates/kim-tcp/src/client.rs
pub struct TcpClient {
    reader: Option<Mutex<TcpReadHalf>>,          // 原来是 Option<TcpReadHalf>
    writer: Option<Arc<Mutex<TcpWriteHalf>>>,    // 已有
    …
}

impl TcpClient {
    pub async fn send(&self, payload: Bytes) -> Result<(), Error> { /* 不变 */ }
    pub async fn read(&self) -> Result<Frame, Error> {  // &self，不再是 &mut self
        let mut guard = self.reader.as_ref().ok_or(NotConnected)?.lock().await;
        loop { /* 现有 Ping/Pong/Binary 逻辑，用 guard.read_frame() */ }
    }
}
```

`ClientSlot` 持有 `Arc<TcpClient>`：`read_loop` 调 `client.read()`，`Forward` 调 `client.send()`。echo-client 仍可 `let mut client`，inherent 方法兼容。

禁止：`into_io` 拆出去后 Container 自己管半边（重复发明 Channel）。禁止：对整颗 `TcpClient` 加一把 Mutex。

#### 6.3.4 内部结构

```rust
pub struct Container {
    naming: Arc<dyn Naming>,
    /// 只 set 一次。用 std OnceLock，不要 tokio Mutex：
    /// `attach_server` 是同步 fn，tokio::sync::Mutex 锁不了。
    server: std::sync::OnceLock<Arc<dyn Server + Send + Sync>>,
    identity: DefaultRegistration,
    dialer: Arc<dyn TcpDialer>,
    deps: Vec<String>,
    clients: Arc<RwLock<HashMap<String, ClientMap>>>, // service_name →
    selector: Arc<dyn Selector>,
    adult_delay: Duration,
    shutdown: Notify,
    closed: AtomicBool,  // shutdown 已叫过。配合 Notify 防丢失唤醒
    state: AtomicU8, // uninit / started / closed
}
```

`attach_server`：`self.server.set(s).map_err(|_| already attached)`。  
`start` / `push` / `push_message`：`let srv = self.server.get().ok_or(...)?.clone();` 然后才 `.await`。OnceLock 的 get 不跨 await。

`start` 末尾等退出（**禁止**只写 `shutdown.notified().await`，测试里 `spawn(start)` 立刻 `shutdown` 会丢 notify_waiters）：

```
if self.closed.load(SeqCst) { return Ok(()); }
self.shutdown.notified().await
// shutdown() 里：closed=true；notify_waiters()；notify_one()（没人 wait 时存一张票）
```

```rust
pub struct ClientSlot {
    pub reg: DefaultRegistration,
    pub client: Arc<TcpClient>,
    pub state: Arc<AtomicU8>, // Young=0 Adult=1
}

pub trait Selector: Send + Sync {
    fn lookup(&self, header: &Header, srvs: &[DefaultRegistration]) -> Option<String>;
}

pub struct HashSelector;
```

`crc32fast = "1"`。`HashSelector::lookup`：

```
srvs 为空 → None          // 禁止 % 0
i = crc32fast::hash(channel_id.as_bytes()) as usize % srvs.len()   // IEEE，和小册 crc32.NewIEEE 一样
→ srvs[i].service_id
```

`ClientMap::adult_services()` 必须返回 **按 `service_id` 升序** 的 `Vec<DefaultRegistration>`。HashMap `.values()` 顺序不稳定，不排序则同一 `channel_id` 会跳实例。双实例单测：列表不变时同一 `channel_id` 两次 `lookup` 得到同一 `service_id`。

#### 6.3.5 启动与拨号

`start()`（`srv` 是 attach 时 clone 出的 `Arc<dyn Server + Send + Sync>`）：

```
attach_server 必须已调用（OnceLock::get），否则 Err
对每个 dep：connect_to_service(name)
若 identity.public_address 非空：naming.register(identity)
若 closed { let _ = srv.shutdown().await; return Ok(()); }   // spawn 之前
spawn { srv.start().await }
若 closed { let _ = srv.shutdown().await; return Ok(()); }   // spawn 之后立刻关
                                                              // 只 return 不够：TcpServer::shutdown
                                                              // 只有 notify_waiters，accept 循环可能还没进 select
shutdown.notified().await
```

`TcpServer` / `WsServer` 的 `shutdown` 必须和 Container 一样 **不丢唤醒**：`closed` AtomicBool + `notify_waiters()` + `notify_one()`。`start` 进 accept 循环前若 `closed` 则立刻返回。这是现有 `TcpServer` 的几行加固（PR3.5 顺手做；`WsServer` 一开始就这样写），不是写侧 mpsc，也不是 TLS。

`connect_to_service`（**先 Subscribe，再 Find**，和小册一致）：

```
Subscribe 的 callback 是 **sync Fn**，里面不能 .await。
回调里：对 ClientMap 没有的 id → tokio::spawn(async {
    match build_client(...).await {          // 见返回值
        Ok(true) => {                        // 刚插入 Young slot
            sleep(adult_delay);
            CAS Young→Adult                  // 仍是 Young 才升；Find 可能已经升过
        }
        _ => {}                              // 没插入：不要 sleep，更不要 unwrap CAS
    }
});
Find：对快照里每一条 **await** build_client，状态 **直接 Adult**
      若 Subscribe 抢先插入了同一 id 且仍是 Young → 这里升级成 Adult
      已有 id：Find 负责 Adult，Subscribe 侧禁止 CAS
```

`#[cfg(test)]` 暴露 `slot_state(service_name, id) -> Option<u8>`（0=Young，1=Adult，None=还没插入）。Young 单测必须先等到 `Some(0)`，不能在 `insert()` 后立刻 Forward（那时 spawn 的 dial 可能还没插入，失败原因是「没有实例」而不是 Young）。

配置里启动就有的 Chat 只出现在 Find → Adult。测试 `insert()` 只走 Subscribe → Young。

**`build_client` → `Result<bool, Error>`**（`true` = **新插入了 Young slot**；connect 失败 **不致命**）：

```
protocol != "tcp" → warn，return Ok(false)
已有同样 service_id → return Ok(false)     // 不要 CAS；Adult 归 Find
TcpClient::new(service_id, service_name, opts)
set_dialer(InnerTcpDialer { local_service_id: identity.service_id })
match connect(dial_url) {
    Err(e) → warn!("dial {} failed: {e}")，不插入，return Ok(false)
             // start() 继续；Server 照样 listen
             // 没有 slot，禁止事后 sleep+CAS（否则以后同 id 再插入会被误升 Adult）
    Ok(()) → spawn read_loop(client.clone())
             插入 ClientMap，state=Young
             return Ok(true)
}
```

调用方：`Ok(true)` 才 `sleep` + CAS；`get(id)==None` 直接返回。禁止对空 slot `unwrap`。

本阶段不做重试/backoff。Chat 后于网关启动且 StaticNaming 不再变化时，成功路径要求 **先起 fake-chat 再起 fake-gateway**（写进 §9.5）。断言 7「只起 gateway」依赖这条：dial 失败 → 空 Adult → ServiceUnavailable，进程还在听 WS。

`InnerTcpDialer`（`dialer.rs`）：

```
TcpStream::connect
TcpConn::new
let bytes = InnerHandshakeReq { service_id: self.local_service_id }.encode_to_vec();
conn.write_frame(Binary, bytes)
conn.flush()     // 与 IdentityDialer 一样，漏 flush 对端可能永远等第一帧
```

Chat 的 `Acceptor`：读一帧 → `InnerHandshakeReq::decode` → 返回 `req.service_id`。

#### 6.3.6 Young / Adult 与上下行

```
Forward(service_name, pkt)
    pkt.channel_id / command 为空 → 错
    pkt.set_meta(META_DEST_SERVER, identity.service_id)
    adult = ClientMap.adult_services()          // 已按 service_id 排序
    空 → Err 让 Handler 映成 ServiceUnavailable
    id = selector.lookup(&pkt.header, &adult)?
    client.send(marshal(Logic(pkt)))
```

为什么要窗口：Chat 必须先和 **全部** 网关建好长连，才能收转发。Demo `adult_delay=0`；库默认 10s；单测 50ms。

**下行 `push_message`**（容器读 Chat→网关的 Client，**不是**业务 Handler）：

```
MustReadLogicPkt
meta dest.server 必须等于本网关 identity.service_id
meta dest.channels 逗号拆分
删掉这两个 meta 再 marshal
对每个 channel_id：server.push(channel_id, bytes)
```

**Chat 侧 `Container::push(gateway_id, pkt)`**：

```
// Handler 已 set dest.server（请求里带来的）和 dest.channels（原 channel_id）
server.push(gateway_id, marshal(Logic(pkt)))
```

方向：Chat 的 Server 管 **网关连进来的 Channel**，id = `"wg-1"`。网关的 Server 管 **Web 客户端 Channel**，id = `"wg-1_alice_N"`（登录后；echo 例子仍用名字）。

网关 Handler 只调 `container.forward`；Chat Handler 只调 `container.push`。不要让 Handler 摸 `server` 字段。

### 6.4 WGateway Handler（只存在于 `examples/fake-gateway`）

不要写进 `WsServer`。这是 **Web** 网关的业务插槽，不是 TGateway。

**已被 M3 替换：** `fake-gateway` Accept 第一帧必须是 LogicPkt `login.signin` + JWT，生成 `wg-1_{account}_{seq}`，**不**再把 utf8 `"alice"` 当 channel_id。identity 第一帧仍用于 crate 测试：`kim-tcp/tests/echo.rs`、`kim-ws/tests/echo.rs`、`kim-container/tests/e2e_echo.rs`。详见 [link-layer-login.md](link-layer-login.md) §7。

M2 当时的 Accept（历史，已被登录 Demo 换掉）：

```
Accept:
    读第一帧 Binary，utf8 trim 当 channel_id。空 → Handshake 错。
    （和 EchoHandler 相同，没有 JWT。）

Receive:
    Packet = read(payload)     // Channel 把 Binary 和 Text 都当字节送来，没有 opcode
    解包失败（坏 Magic / 半包）→ warn，return
    Basic + ping → agent.push(pong)；info!("basic ping, local pong"); return
    Logic → header.channel_id = agent.id();
            container.forward(pkt.service_name(), pkt).await
            Forward 失败 → 构造 Response（同一 sequence），
              Status=ServiceUnavailable 或 SystemException，agent.push

Disconnect:
    info!(channel, "disconnect")
```

不要写「丢掉 Text 帧」：`ChannelReadLoop` 不把 opcode 传给 Handler，本阶段也不改 `kim-core`。业务包 **约定** 用 Binary；误发 Text 只要 Magic 不对就会 warn 丢弃。echo 例子不解析 Magic，Text 仍可回声。

### 6.5 Chat Handler（只存在于 `examples/fake-chat`）

**已被登录 Demo 替换：** 现 Receive 走 Router + 会话（`login.signin` / `login.signout` / `chat.demo.echo`）。下面是 M2 当时只回 echo 的历史。当前逻辑见 [link-layer-login.md](link-layer-login.md)。

```
Accept:
    InnerHandshakeReq → service_id

Receive:
    let pkt = read_logic(payload)?;  // Basic → warn!("unexpected basic pkt"); return
    match pkt.header.command.as_str() {
        "chat.demo.echo" => {
            let mut resp = pkt;
            resp.header.flag = Response as i32;
            resp.header.status = Success as i32;
            // sequence / command / channel_id / body 保持
            let gw = resp.get_meta(META_DEST_SERVER).unwrap_or("").to_string();
            let ch = resp.header.channel_id.clone();
            resp.set_meta(META_DEST_SERVER, &gw);
            resp.set_meta(META_DEST_CHANNELS, &ch);
            container.push(&gw, resp).await     // 不要直接 server.push
        }
        _ => { /* Status::CommandNotFound，同一条 container.push */ }
    }
```

Chat **没有** `Forward` 依赖。回包统一走 `Container::push`。

日志：`info!(command, sequence, channel_id, "chat recv logic")`。验收靠这条：ping 不得出现。

### 6.6 `pkt-client`（Web 客户端，不是 App）

`pkt-client` 扮演 **Web → WGateway**。当前握手是 JWT `login.signin`，见 [link-layer-login.md](link-layer-login.md) 与根 README。App → TGateway 继续是 TCP echo。

`e2e_echo.rs` 仍用 identity 第一帧（名字），不走 `pkt-client`。

解析：`args` 里以 `ws://` / `wss://` 开头的当 URL，其余第一个当 id（默认 `alice`）。环境变量单独读。禁止 `pkt-client -- alice --expect-unavailable`。

`publish = false`。

---

## 7. 它如何坐在现有 Channel 专员上

本阶段 **零复制** `Channel` / `ChannelMap`。新增传输只履行 `Conn`；新增业务只插三个 listener。

```
                    clone Channel（信箱）立刻放锁
ChannelMap RwLock ──────────────────────────────────► 多个业务任务 push
       ▲                                                │
       │ add/remove                                     ▼
       │                                          mpsc 写信箱（FIFO = 发出顺序）
WsServer / TcpServer                                      │
       │                                                  ▼
       │                                          写专员 唯一 write_frame
accept 后 Channel::pair(reader, writer)
       │
       ▼
读专员 唯一 read_frame
  OpPing ──► 信箱（Pong）
  Binary ──► MessageListener（网关解析 Magic；Chat 解析 LogicPkt）
```

对照表：

| 动作 | 用哪把锁 / 哪条队列 | 不要做什么 |
|---|---|---|
| 登录/断线改表 | `ChannelMap` 的 `RwLock` | 握着表锁 `await` 写网络 |
| 多个人给 Alice 发 | Alice 的 mpsc | 在业务里直接 `conn.write_frame` |
| 读字节 | 每连接一个读任务 | 两个任务 `read_frame` 同一条 Conn |
| WS 库 auto_pong | **关掉** | 让库在读任务里写 socket |

网关 → Chat 的 `TcpClient` **写侧仍是 Mutex**（不要对号入座服务端专员图）。PR3.5 只把 `read` 改成 `&self`，让 `send` 与 `read` 能同时进行；**不**改写路径为 mpsc。

假 **WGateway** 同时拥有：

- 面向 Web 客户端的 `WsServer.channel_map`（alice、bob…）
- 面向 Chat 的 `Container.clients`（chat-1、chat-2…）

两张表职责不同，不要合成一张。

---

## 8. 安全：HTTPS / WSS / TCP+TLS vs 本机明文

HTTPS 只包住 REST。WSS 只包住 **Web 长连接**。App 长连接是 TCP，公网再套 TLS，**不是** WSS。

### 8.1 本阶段（必须明文本机）

| 线 | 地址 | TLS | 扮演谁 |
|---|---|---|---|
| Web ↔ WGateway | `ws://127.0.0.1:8001/` | 无 | pkt-client |
| App ↔ TGateway | TCP（`kim-tcp`） | 无 | crate 测试；本机 Demo 尚未起 TGateway |
| 网关 ↔ Chat | `127.0.0.1:8002` TCP | 无 | 内网 |

`WsClient::connect` 接受 `wss://`（rustls + webpki-roots）。`WsServer` 仍明文。`TcpClient` TLS 是后做的 TGateway 外壳。

绑定地址默认 `127.0.0.1`，不要 `0.0.0.0`（examples 如此；库的 `bind` 尊重调用方）。

### 8.2 以后公网（本阶段只设计、不实现）

```
所有客户端
  ├─ HTTPS REST ──► Cloudflare 橙云 ──► 反代 ──► Royal / Router
  │                   SSL Full 或 Full (strict)，不要 Flexible
  │
  ├─ Web  ── WSS ──► Cloudflare 橙云 ──► 反代 ──► kim-ws（WGateway）
  │
  └─ App  ── TCP+TLS ──► TGateway
              ▲
              └─ 橙云不转裸 TCP：公网用灰云，或独立 IP:port + TLS
                 无 HTTP 升级。本阶段不做这层加密外壳。
```

证书：REST 和 WSS 走边缘或源站证书；`WsServer` 不内嵌 rustls（反代终止）。Rust 客户端 `wss://` 用 rustls。TGateway 的 TCP+TLS 以后再说。

鉴权：小册把登录放在 Upgrade **之后** 的业务包。本阶段连登录都没有。禁止提前把 token 塞进 `ws://host/?token=`——Upgrade 只发生一次，token 会出现在反代日志和 Referer 里。App 的 token 走 HTTPS REST，再在 TCP 长连上带业务包（以后的课）。

---

## 9. 测试 / 验收

命令除非注明，一律在仓库根目录。

### 9.1 回归（M1a 不能坏）

```bash
cargo test -p kim-tcp --test echo
```

### 9.2 M1b：同一 EchoHandler，第二条电线

自动化：`crates/kim-ws/tests/echo.rs`，从 `kim-tcp/tests/echo.rs` 复制，把 `TcpServer`/`TcpClient`/`IdentityDialer` 换成 `WsServer`/`WsClient`/`WsIdentityDialer`，地址用 `127.0.0.1:0` 再拼 `ws://{addr}/`。

断言：`payload == b"hello from server"`。

```bash
cargo test -p kim-ws --test echo
```

额外单测（`kim-ws`）：

- opcode 映射表（Binary/Text/Ping/Pong/Close）。
- 非 WebSocket 的普通 GET `/` → 400 或 404，进程不崩。

### 9.3 M2 协议单测

`cargo test -p kim-protocol`

- Magic 字节等于小册。
- ping 编码：`c3 15 a7 65  01 00  00 00`（code=1 LE，len=0）。
- Logic 往返 sequence / command / bodyLength。
- `service_name`。

### 9.4 Naming / Container 单测

`cargo test -p kim-naming`：

- `find("chat")` 返回构造时那一条；未知名字空列表。
- `subscribe` 之后 **立刻** `find` 仍只有旧列表；`insert()` 才触发回调。

`cargo test -p kim-container`（**全部用本机 `TcpServer` 听 `127.0.0.1:0` + `InnerTcpDialer` / `TcpClient`**。没有内存假 `TcpConn`，`TcpDialer` 返回的是真 `TcpStream`）：

1. **Young 窗口（Subscribe / `insert` 路径）**：`adult_delay=50ms`。Container `start` 时 Naming 里还没有 chat；测里 `insert()` 一条已在听的 Chat。**先 poll `slot_state == Some(Young)`**（loopback 通常 <10ms，超时 1s 则测失败）。此时 `Forward` → `ServiceUnavailable`（证明是 Young，不是「slot 还没插上」）。再等 `adult_delay + 20ms`，`slot_state == Some(Adult)`，Forward 成功。
2. **Find 已有实例直接 Adult**：构造 Naming 时就把 Chat 地址放进去（Chat `TcpServer` 已 listen）。`start` 之后立刻 `Forward` 成功，不等 `adult_delay`。
3. **拨号全部 + HashSelector 稳定**：两个 `TcpServer` 听 `127.0.0.1:0`，两个 slot 都在 ClientMap。同一 `channel_id` 连续两次 `lookup` 得到同一 `service_id`（`adult_services` 按 id 排序）。

### 9.5 容器 Demo 端到端（`e2e_echo.rs`，identity 握手）

登录 Demo 的关门条件见 [link-layer-login.md](link-layer-login.md) 与根 README。下面只约束 **identity** 的 `e2e_echo.rs`（第一帧仍是名字）。

成功路径 **必须先起 Chat，再起网关**（静态列表不会重试 dial 失败的实例）：

```bash
# 终端 1
RUST_LOG=info cargo run -p fake-chat

# 终端 2（等 Chat listen 之后）
RUST_LOG=info cargo run -p fake-gateway

# 终端 3
RUST_LOG=info cargo run -p pkt-client -- alice          # URL 默认 ws://127.0.0.1:8001/
# Chat 日志安静检查：
KIM_PING_ONLY=1 cargo run -p pkt-client -- alice
# 断言 7（不起 Chat）：
KIM_EXPECT_UNAVAILABLE=1 cargo run -p pkt-client -- alice
```

必须同时成立：

| # | 断言 |
|---|---|
| 1 | pkt-client 退出码 0 |
| 2 | 客户端收到 BasicPkt pong（code=2） |
| 3 | 客户端收到 LogicPkt，`sequence == 1`，`flag == Response`，body 为 `hello` |
| 4 | fake-gateway 日志含 `basic ping` / `local pong` |
| 5 | fake-chat 日志含 `chat recv logic` 且 command=`chat.demo.echo` |
| 6 | fake-chat 日志 **不含** `ping`、`MagicBasicPkt`、`basic pkt` |
| 7 | 只起 gateway、不起 chat：`KIM_EXPECT_UNAVAILABLE=1 cargo run -p pkt-client -- alice` 收到 Response `status == ServiceUnavailable`；ping 仍在网关本地回。网关进程必须还在听（dial 失败不 abort `start`） |

进程内集成测试放 `crates/kim-container/tests/e2e_echo.rs`（**PR5 才加**；给 `kim-container` 增加 **dev-dependency** `kim-ws`，生产依赖始终没有 kim-ws）：

- 线程内 `TcpServer`（Chat）+ `WsServer`（Gateway）+ `WsClient`。
- Chat Handler 把收到的 command 推进 `Arc<Mutex<Vec<String>>>`。
- 先 ping 后 echo；断言 vec 里只有 `chat.demo.echo`。
- 第二个测试：不起 Chat，断言 `ServiceUnavailable`。

### 9.6 明确不验收（本文 / 容器层）

- 浏览器演示页、公网 WSS、公网 TGateway TLS、压测。Web SDK 见 [web-sdk.md](web-sdk.md)。
- JWT / 互踢：见 [link-layer-login.md](link-layer-login.md)，不在 `e2e_echo.rs`。
- 把 App 客户端改成走 WebSocket。
- 把 `TcpClient` 写侧 Mutex 改成 mpsc（读改成 `&self` 除外）。

---

## 10. 风险

| 风险 | 为什么会发生 | 处理 |
|---|---|---|
| 把业务写进 `WsServer` | 小册 demo 也容易把 Accept 和指令搅在一起 | 指令只在 examples 的 Handler；server 只做 Upgrade + Channel |
| axum / hyper / fastwebsockets 版本拧巴 | `with_axum` 跟 axum 0.8 曾不齐 | **不用 axum extractor**；hyper 1 + `fastwebsockets/upgrade` |
| `auto_pong=true` 抢写 | 库文档默认 true | 握手后立刻 `set_auto_pong(false)` |
| `read_frame` 取消后继续读 | 半帧状态丢失 | 超时只允许 drop 整条 Conn |
| Young 窗口让 Demo 像「卡住」 | 默认 10s | Demo `adult_delay=0`；库默认 10s；单测 50ms。成功路径先起 Chat |
| 依赖没起来就 abort | `connect` 失败若返回 Err，网关不听，断言 7 做不成 | dial 失败只 warn，不插入；Forward 遇空 Adult → ServiceUnavailable |
| `Arc<TcpClient>` 编不过 / 死锁 | `read` 是 `&mut self` | PR3.5：`read(&self)` + 读半边 Mutex |
| HashSelector 跳实例 | HashMap 迭代无序 | `adult_services()` 按 `service_id` 排序 |
| 两种心跳搞混 | WS Ping vs BasicPkt ping | 文档 + 日志字段分开：`opcode ping` vs `basic ping` |
| Magic 当整数比 | 和大端/小端搅在一起 | 只比 `[u8;4]` |
| 浏览器自定义 opcode | 发不出去 | 业务只用 Binary |
| 静态配置漏实例 | 只拨号了「第一个 Chat」 | API 禁止 pick-one；测试两个实例 |
| Cloudflare 橙云 + 裸 TCP | 橙云不转任意 TCP，公网 TGateway 不能挂在橙云后面 | 协议仍是 App→TCP。公网以后灰云或 IP:port+TLS。本阶段不部署 |
| 在 Upgrade URL 塞 token | 日志泄漏、小册也把鉴权放握手后 | 禁止；登录是以后的课 |

---

## 11. 被拒绝的方案

| 方案 | 拒绝原因 |
|---|---|
| 用 WSS 代替 App 长连接 | 小册 App 走 TGateway（TCP）。HTTPS 只包 REST，与 TGateway **不冲突**。橙云不转裸 TCP 只影响公网 **怎么暴露** TGateway，不改变协议 |
| 本阶段给 TGateway 套 TLS | 只是加密外壳，后做。本机 `kim-tcp` echo 已是 App 路径。不要把「还没套 TLS」理解成「改走 WSS」 |
| IM 头放在 HTTP Upgrade Query | Upgrade 只发生一次；小册把鉴权放在握手之后的业务包；token 进 URL 会进日志 |
| 自定义 WS opcode 表示 ping/登录 | 浏览器只能 text/binary；原生 opcode 已被映射到 `kim_core::OpCode` |
| 本阶段上 Consul | 先证明 Naming 合同和全连接。`StaticNaming` 足够 Demo。Consul 客户端在 Rust 里偏薄，留给以后 |
| 用 axum `WebSocketUpgrade` / tungstenite 当网关 | 消息级、碎片会拼起来、写路径不在我们的专员手里。热路径必须是 `after_handshake` 之后的帧 |
| 网关像 HTTP LB 一样只挑一台 Chat | 长连回包要原路；逻辑服务对网关有状态窗口。必须拨号 **全部** 列出的实例 |
| 给 BasicPkt 也套 protobuf | 心跳要小、网关本地处理。4+2+2 足够 |
| LogicPkt 再写一份 payload_len（小册 JS） | 与 Header.bodyLength 重复。本复刻以 Header 字段为准 |
| `Container` 做成进程级 `OnceLock` 单例 | 测试起不了两套；用 `Arc<Container>` + `attach_server`。**字段** `OnceLock<Server>` 可以，那不是单例 |
| 本阶段把 TcpClient 写侧改成 mpsc | 服务端专员已经完整。允许的只有 `read(&self)`，否则 Container 死锁 |
| `StaticNaming::subscribe` 立刻 dump 当前列表 | 会把配置实例标成 Young，Find 捷径消失。Subscribe = 以后的变更；Find = 快照 |

---

## Key Decisions

1. **本阶段 = M1b + M2，不含登录。**  
   通信层合同已经用 TCP echo 证明。下一步是「第二种电线」+「payload 里的业务包和容器」。JWT/Redis/Consul/部署会淹没这两件事。

2. **长连接按小册双网关：Web → WSS → WGateway；App → TCP(+TLS) → TGateway。HTTPS 只包 REST。**  
   不要让 App 先走 WSS。橙云不转裸 TCP，只影响公网 TGateway 怎么暴露（灰云或 IP:port + TLS），不改变协议。本阶段：`kim-ws` 给 Web；`kim-tcp` 给 App 和网关↔Chat（本机明文）。公网 TGateway 的 TLS 是后做的加密外壳。Cloudflare 上的 REST/WSS：SSL Full 或 Full Strict，不用 Flexible。

3. **`kim-ws` 只履行 `Conn`；Channel 专员 100% 复用。**  
   换传输不加连接表。`fastwebsockets` 帧级 + `unstable-split` 对齐 `TcpConn::into_split`。hyper 只负责 Upgrade。

4. **关掉库的 auto_pong / auto_close。**  
   Ping/Pong/Close 的语义已经在 `ChannelReadLoop`。库若在读任务写 socket，两专员模型作废。

5. **业务包只在 Binary payload；Magic 用小册原字节。**  
   `MAGIC_LOGIC_PKT = {0xc3,0x11,0xa3,0x65}`，`MAGIC_BASIC_PKT = {0xc3,0x15,0xa7,0x65}`。HTTP 头和自定义 WS opcode 都不承载 IM。

6. **BasicPkt 小端；LogicPkt 的 `header_len` 大端。**  
   Basic 跟小册「小头字节序」。Logic 的长度跟小册 Web SDK 的 `Int32BE`，方便以后写 JS。Magic 本身按字节比。

7. **Header 同时有 `bodyLength` 和 `meta`。**  
   前者是第 14 章表格（本阶段用户已拍板的字段）；后者是容器寻址所必需。`dest.server` / `dest.channels` 不塞进 `dest`（`dest` 留给以后的用户/群）。

8. **网关按 `command` 第一个 `.` 之前的前缀路由。**  
   `chat.demo.echo` → 服务名 `chat`。登录上行必须 `forward(SN_LOGIN)`（值为 `"chat"`），禁止 `service_name("login.signin")`。

9. **Naming 先静态配置；Find = 快照（Adult），Subscribe = 以后的变更（Young）。**  
   网关拨号全部 Chat 实例，不是 HTTP 负载均衡。`adult_delay` 库默认 10s，Demo 0。配置里已有的实例绝不能走 Subscribe dump。

10. **容器是 `Arc<Container>` 不是全局单例；两阶段 `new` → 设 Handler → `attach_server` → `start`。**  
    Handler 持有 `Arc<Container>`，只调 `forward` / `push`。`attach_server` 同步，`server` 字段是 `std::sync::OnceLock`（不是 tokio Mutex）。`start` 用 `closed` AtomicBool + `Notify::notify_one` 防丢失唤醒，不是 ctrl-c。spawn `server.start()` 前后若已 closed，必须再调 `srv.shutdown()`——只退出 Container 的 wait 关不掉 accept 循环。`TcpServer`/`WsServer` 同样 closed+`notify_one`。不改 `kim-core` 的 Server trait。

11. **echo 进服务端仍是第一帧名字；`pkt-client` 进 WGateway 是 JWT `login.signin`；网关进 Chat 是 `InnerHandshakeReq` protobuf。**  
    echo 证明 WS `Conn`。登录见 [link-layer-login.md](link-layer-login.md)。App 进 TGateway 继续用 TCP echo 握手，不另做假 TGateway。

12. **`TcpClient` 写侧 Mutex 保留；`read` 改成 `&self`（读半边也 Mutex）。**  
    这是 Container 能同时 Forward 和 read_loop 的前提。不要对整颗 Client 加锁，也不要把写侧改成 mpsc。

13. **超时取消 `read_frame` 之后禁止继续用该 Conn。**  
    现有 ReadLoop 超时即 drop 连接，WS 可接受。不要改 `channel.rs`。

14. **Chat 回包统一 `Container::push(gateway_id, pkt)`；网关下行由容器 Client 读循环拆 `dest.channels`。**  
    不要 Handler 直接摸 `server.push`。`set_meta` 按 key 替换（禁止 append），`del_meta` 删光该 key。

15. **dial 失败不 abort `start`；`build_client` 返回是否插入了 Young slot，只有 `true` 才 sleep+CAS。**  
    空 Adult → `Forward` 报错 → Handler 映 `ServiceUnavailable`。成功 Demo 先起 Chat。已有 id / 非 tcp / dial 失败都是 `Ok(false)`，禁止对空 slot unwrap。

16. **`adult_services()` 按 `service_id` 排序；空列表 `HashSelector` 返回 `None`。**  
    HashMap 迭代顺序不能当路由输入。

---

## Open Questions

本阶段能拍板的都已拍板。真正留到以后的只有这些（**本阶段实现时按括号内默认做，不要再卡住**）：

1. 公网 WGateway 回源是反代终止 TLS 再本机明文连 `kim-ws`，还是进程内 rustls —— **以后部署时再选；默认反代终止。** 公网 TGateway 用灰云还是独立 IP:port+TLS —— **同样以后再选；协议仍是 TCP。**
2. Consul 还是 etcd 做 `Naming` 第二实现 —— **以后。本阶段只有 StaticNaming。**
3. 多 Chat 实例时 HashSelector 在列表变化后会迁移用户 —— **可接受；智能路由是很后面的章。**
4. `Server` trait 要不要长出 `ServiceID()` —— **本阶段 identity 外挂在 Container 上，不改通信层 trait。**

---

## PR Plan

每个 PR 必须能单独 review：有自己的测试，不依赖下一个 PR 的例子才能编译（workspace members 可以先写上，未实现的 crate 不要提前加进 members，避免 `cargo test` 裂）。

### PR1 — `kim-ws`：Web / WGateway 的第二种 Conn + WS echo

- **标题：** `feat(kim-ws): WebSocket Conn via HTTP Upgrade and Channel specialists`
- **文件 / 组件：**  
  `Cargo.toml`（加 `kim-ws`、`ws-echo-*` members 与依赖）  
  `crates/kim-ws/**`  
  `examples/ws-echo-server/**`、`examples/ws-echo-client/**`（`publish = false`）  
  可选：`docs/communication-layer.md` 末尾指向 protocol-container
- **依赖：** 无。不要改 `kim-core`。不要删 kim-tcp。
- **内容：** `http1::Builder.serve_connection(...).with_upgrades()`；`UpgradeFut` 已是 `WebSocket<TokioIo<Upgraded>>`，禁止再包 TokioIo。`set_auto_pong(false)` / `set_max_message_size(1MiB)`。`WsServer::shutdown` 用 `closed` + `notify_one`（进 accept 循环前若 closed 则返回）。同一 EchoHandler，发 `hello {i}`。  
- **验收：** `cargo test -p kim-ws`；`cargo test -p kim-tcp` 仍绿。

### PR2 — `kim-protocol`：Magic + BasicPkt + LogicPkt

- **标题：** `feat(kim-protocol): Magic, BasicPkt and protobuf LogicPkt`
- **文件 / 组件：** `crates/kim-protocol/**`（含 `proto/pkt.proto`、`build.rs` + `protobuf-src`）
- **依赖：** 无（可与 PR1 并行）
- **内容：** 编解码、`service_name`、`InnerHandshakeReq`。`build.rs` 用 vendored `protoc`。  
- **验收：** 干净环境 `cargo test -p kim-protocol`（不要求事先 brew install protobuf）。

### PR3 — `kim-naming`：trait + StaticNaming

- **标题：** `feat(kim-naming): Naming trait and StaticNaming from config`
- **文件 / 组件：** `crates/kim-naming/**`
- **依赖：** 无（可与 PR1/PR2 并行）
- **内容：** `find` = 快照；`subscribe` = **只通知以后的变更**（不 dump）。`insert()` 单测。不解析 TOML。  
- **验收：** `cargo test -p kim-naming`。没有 Consul。

### PR3.5 — `TcpClient::read(&self)` + Server shutdown 不丢唤醒

- **标题：** `fix(kim-tcp): concurrent send/read and loss-free Server shutdown`
- **文件 / 组件：** `crates/kim-tcp/src/client.rs`、`crates/kim-tcp/src/server.rs`
- **依赖：** 无。可与 PR1–PR3 并行，必须在 PR4 之前合入。
- **内容：** `reader: Mutex<TcpReadHalf>`；`read(&self)`。写侧 Mutex **不动**。`TcpServer`：`closed` AtomicBool + `notify_waiters` + `notify_one`；`start` 进 accept 前若 closed 则返回。单测：loopback `read` 挂起时 `send` 仍能完成；`bind` 后立刻 `shutdown` 能让 `start` 退出。无 TLS、无业务分支、无写侧 mpsc。  
- **验收：** 新单测 + 原 `crates/kim-tcp/tests/echo.rs` 仍绿。

### PR4 — `kim-container`：全连接、Young/Adult、Forward/Push

- **标题：** `feat(kim-container): dial all instances, Young/Adult, Forward`
- **文件 / 组件：** `crates/kim-container/**`  
  生产依赖：kim-core、kim-tcp、kim-protocol、kim-naming。**没有 kim-ws。**
- **依赖：** PR2、PR3、PR3.5。
- **内容：** 公开 API（`new` / `attach_server` / `start` / `shutdown` / `forward` / `push`）；`dyn Server + Send + Sync`；dial 失败非致命；`build_client → Result<bool>`（`true` 才 sleep+CAS）；`start` 在 spawn `server.start` 前后若 closed 则再调 `srv.shutdown()`；`adult_services` 排序；`InnerTcpDialer` 含 `flush`。单测全部 loopback `TcpServer`：Young（insert）、Find→Adult、双实例稳定路由。  
- **验收：** `cargo test -p kim-container`（此时还没有 `e2e_echo.rs`）。

### PR5 — Demo：假 WGateway + 假 Chat + Web 版 pkt-client

- **标题：** `feat(demo): fake WGateway and Chat with BasicPkt local ping`
- **文件 / 组件：**  
  `examples/fake-gateway/**`、`fake-chat/**`、`pkt-client/**`（`publish = false`，TOML 解析在 example）  
  `crates/kim-container/tests/e2e_echo.rs`  
  `kim-container` **dev-dependencies** += `kim-ws`（只在这一 PR 加）
- **依赖：** PR1–PR4
- **内容：** 两阶段装配 Handler；`forward` / `push`；pkt-client 默认 URL、`KIM_EXPECT_UNAVAILABLE`、`KIM_PING_ONLY`。e2e：ping 不进 Chat；不起 Chat 时 ServiceUnavailable。  
- **验收：** `cargo test -p kim-container --test e2e_echo`；手工三进程见 §9.5（先 Chat 再网关）。

PR 顺序：`PR1 ∥ PR2 ∥ PR3 ∥ PR3.5` → `PR4` → `PR5`。  
Reviewer 看 PR1 只问「EchoHandler 换电线是否仍通」；看 PR5 只问「ping 是否漏进 Chat、sequence 是否回来」。

---

## 附录 A. 实现时对照的类型名

| 概念 | Rust 名 | 所在 crate |
|---|---|---|
| 连接合同 | `Conn` | kim-core |
| 帧 | `Frame` / `OpCode` | kim-core |
| 两专员 | `Channel::pair` / `ChannelReadLoop` | kim-core |
| 连接表 | `ChannelMap` | kim-core |
| TCP 履行（App / TGateway / 网关↔Chat） | `TcpConn` / `TcpServer` / `TcpClient`（`read(&self)`） | kim-tcp |
| WS 履行（Web / WGateway） | `WsConn` / `WsServer` / `WsClient` | kim-ws |
| 业务包 | `Packet::{Basic,Logic}` / `read` / `marshal` | kim-protocol |
| 服务发现 | `Naming` / `StaticNaming` | kim-naming |
| 容器 | `Container` / `HashSelector` / `InnerTcpDialer` | kim-container |
| Demo 指令 | `CMD_DEMO_ECHO` = `"chat.demo.echo"` | kim-protocol |

## 附录 B. 本机默认端口

| 进程 | 监听 |
|---|---|
| echo-server（已有，App/TGateway 路径） | `127.0.0.1:8000` TCP |
| ws-echo-server / fake-gateway（Web / WGateway） | `127.0.0.1:8001` WS |
| fake-chat | `127.0.0.1:8002` TCP |

端口冲突时 examples 用第一个 CLI 参数覆盖，和现在 `echo-server` 一样。
