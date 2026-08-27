# 架构

当前进度：**通信层 TCP + WebSocket、业务包、静态 Naming、容器 Demo、链路层登录（JWT / 会话 / 互踢）已落地**。可选 Redis 会话、Consul、公网部署还没有。读本文时把「以后」和「已经有」分开。交互总图：[diagrams/kim-overview.html](diagrams/kim-overview.html)。详细帧与容器规格见 [protocol-container.md](protocol-container.md)；登录见 [link-layer-login.md](link-layer-login.md)。

## 一句话

内核保证字节能到；`kim-tcp` 保证字节能切成帧；`kim-core` 保证帧属于某条有生命周期的连接；业务只处理帧里面的意思。

读写拆分、查表放锁、echo 时序图见 [communication-layer.md 链路图](communication-layer.md#链路图先看图)。

## 从机器往上

```text
┌──────────────────────────────────────────────────────────┐
│  业务（echo-* 身份握手；fake-gateway JWT 登录；fake-chat Router）│
│  登录插槽已在 examples 落地；单聊以后仍经同一套 listener        │
│  插槽：Acceptor / MessageListener / StateListener          │
├──────────────────────────────────────────────────────────┤
│  kim-core     通信层说明书 + 连接生命周期                     │
│               Channel（写信箱+写专员、读专员）、ChannelMap      │
│               自己不碰网卡                                   │
├──────────────────────────────────────────────────────────┤
│  kim-tcp      说明书的 TCP 实现                              │
│               向内核借 socket + 应用层分帧（粘包/半包）         │
│  以后 kim-ws / 可选 kim-quic  同一套说明书的其它实现           │
├──────────────────────────────────────────────────────────┤
│  操作系统内核 TCP   三次握手、序号、ACK、重传、拥塞控制          │
│  IP / 网卡                                                │
└──────────────────────────────────────────────────────────┘
```

```text
业务 Handler  ──►  kim-core（表 + 信箱 + 两专员）  ──►  kim-tcp（分帧、拆插座）  ──►  内核 TCP
```

小册原文的四层（通信 → 容器 → 链路 → 控制）是**整套 IM 云**的分层。通信 / 容器 / 链路登录已在 crate + examples 落地；控制层以后仍以独立二进制出现，**不要**把登录、聊天写进 `TcpServer`。

## Crate 职责

| Crate | 允许放什么 | 禁止放什么 |
|---|---|---|
| `kim-core` | trait、Frame、Channel、ChannelMap、超时默认值 | socket、具体编解码、JWT、SQL、Redis |
| `kim-tcp` | `TcpListener`/`TcpStream`、长度前缀编解码、TcpServer/TcpClient | 「这是登录包」「这是群聊」 |
| `examples/echo-*` | 最小业务：第一帧当名字、原文回声 | 假装自己是网关或 Chat 服务 |
| `kim-ws` | WebSocket 的 `Conn` 实现 | 复制一套 ChannelMap；JWT |
| `kim-protocol` | BasicPkt / LogicPkt / JWT HS256 | 再实现一遍 TCP 读写 |
| `kim-router` | command → Handler、Context.Resp / Dispatch | Redis、TCP |
| `kim-session` | Memory 会话；可选 Redis feature | 指令业务 |
| `examples/fake-*` | WGateway JWT Accept、Chat 登录/echo | 把 `if login` 写进 `WsServer` |

原则：**换传输只加 `Conn` 实现，不改业务。** 长连接按小册双网关：Web → WGateway（WS/WSS），App → TGateway（TCP，公网再套 TLS）。HTTPS 只包住 REST，不替代长连接。

## Conn 合同（业务到底能假设什么）

业务不关心底下是 TCP、WebSocket 还是 QUIC。它能假设的是实现 `Conn` 的那一层已经提供：

1. **可靠** — 发出去的帧，对方最终能收到，或你能知道失败  
2. **有序** — 先发的先到  
3. **有边界** — `read_frame` 得到的是完整一帧，不是半截字节流  

这三样合起来，才叫「把业务字节稳定送到 client」。

| 候选 | 能不能直接当现在的 `Conn` | 原因 |
|---|---|---|
| TCP | 能 | 内核给可靠有序流，我们补边界 |
| WebSocket | 能 | 底下是 TCP，帧自带边界 |
| QUIC 可靠流 | 能（以后） | 自己做可靠流，适合同一 trait |
| 裸 UDP | **不能** | 会丢、会乱序；音视频才常用。要当 `Conn` 得先自己做 ACK/重传 |

深度优化（大厂改内核、调 BBR、io_uring）是规模问题，见 [communication-layer.md](communication-layer.md) 末节。默认内核 TCP 已经在做拥塞控制。

## 当前仓库地图

```text
im/
  crates/kim-core          说明书
  crates/kim-tcp           TCP 实现（App / 内网）
  crates/kim-ws            WebSocket 实现（Web）
  crates/kim-protocol      业务包 + JWT
  crates/kim-naming        静态服务发现
  crates/kim-container     拨号、Young/Adult、转发
  crates/kim-router        指令 Router
  crates/kim-session       会话（Memory / 可选 Redis）
  examples/                echo / ws-echo / fake-gateway / fake-chat / pkt-client
  docs/                    本目录
  research/                可行性调研
```

本机跑法见根目录 `README.md`。

## 以后会有、现在不要提前写进通信层的

- **容器层（已有静态 Naming）**：Consul 以后再换实现，不要改 `TcpServer`  
- **链路层（M3 已有）**：JWT 登录、Memory 会话、指令 Router 在 examples / `kim-router` / `kim-session`。不要把 JWT 写进 `kim-ws`  
- **控制层**：单聊、群聊、离线  
- **部署**：`kim.ainexc.com` 与 `minos.ainexc.com` 共存（反向代理按域名分流）。通信层在本机跑通之前不上 VPS  

服务发现登记的是**实例**（可拨号的 IP:端口），不是「只发现进程」或「只发现机器」。本机多进程和多台 VPS，对网关是同一件事。

## 进门怎么走：跟小册双网关，HTTP 升级成 HTTPS

三件事不要混。**HTTPS 是短接口；长连接仍按小册分 Web / App。** App 用 HTTPS 拿 Token，和它再用 TCP 聊天 **不冲突**。

```text
所有客户端
  ├─ HTTPS REST     登录拿 Token、Router、用户/群     ← 小册是 HTTP，我们改 HTTPS
  │
  ├─ Web  长连接 ── WSS ──► WGateway     ← 小册 WebSocket，我们公网用 WSS
  └─ App  长连接 ── TCP(+TLS) ──► TGateway
                    ▲
                    └─ 小册就是这条裸 TCP；公网再套 TLS，不经过 HTTP 升级
```

| 流量 | 小册 | 我们 |
|---|---|---|
| Royal / Router | HTTP | **HTTPS**（Cloudflare 证书） |
| Web 长连接 | WebSocket（示例有 `wss://`） | **WSS**，橙云可转 |
| App 长连接 | **TCP**（TGateway） | **仍走 TGateway**。本机明文；公网 **TCP+TLS** |
| 网关 ↔ Chat | TCP | TCP（内网） |

App 的 HTTPS 只用于 REST，不会变成「App 也必须 WSS」。先前「App 先 WSS」只是迁就 Cloudflare 不转裸 TCP；**长连接协议仍按小册：Web=WS，App=TCP。**

Cloudflare：REST/WSS 用橙云，SSL 用 **Full / Full Strict**。TGateway 公网不要走橙云，用灰云或 IP:端口 + TLS。

这只换进门加密，不换帧、两专员、业务包头。

## 刻意保持瘦的东西

`TcpServer` 和 `kim-core` **不应随业务膨胀**。登录、互踢、群扩散是新进程 + 新 Handler。若发现自己在 `kim-tcp` 里写 `if 这是登录`，分层已经破了。
