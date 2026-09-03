# Harden TcpConn as Generic Stream and Terminate TLS at TGateway

| 字段 | 值 |
|---|---|
| 状态 | Draft（审查修订：连接上限 / TLS 握手超时 / 前端状态 / 构造函数名） |
| 日期 | 2026-09-02 |
| 覆盖 | G-34（通信层半边：`TcpConn<S>`、TGateway TLS、socket 选项、连接上限） |
| 父规格 | [next-stage.md](./next-stage.md)、[production-gaps.md](../production-gaps.md) G-34 |

## Breaking Change Notice

**无公共 API 破坏性移除。** `TcpConn` 从具体 `TcpStream` 泛化为 `TcpConn<S>`，但：

- 明文 `TcpConn::new(stream: TcpStream)` **签名不变**（`kimbench`、`e2e_tgateway`、`InnerTcpDialer`、`TcpClient`、`handle_conn` 共 5 处）。泛型构造另名 `with_peer`，禁止同时定义泛型 `new(stream, peer)` 和同名类型别名。
- `TcpServer` **不**泛型化（决策 2），`Server` trait 完全不动。
- 唯一新消费者是 `services/tgateway`，workspace 内部，无外部下游。

## Feasibility Assessment

- `Channel::pair`（`crates/kim-core/src/channel.rs:133`）要求 `R: Conn + 'static`，`WsConn<S>` 已证明泛型流可实现 `Conn` 并通过 `Channel::pair`（`crates/kim-ws/src/conn.rs`）。`TcpConn<S>` 的分帧逻辑（`fill_and_decode` / `write_frame_parts`）已是对 `AsyncReadExt + AsyncWriteExt + Unpin` 的自由函数，不需要改。
- `tokio_rustls::server::TlsStream<TcpStream>` 满足 `AsyncRead + AsyncWrite + Unpin + Send + 'static`，直接可作 `S`。
- `TcpServer::handle_conn` 现为模块级私有自由函数（`crates/kim-tcp/src/server.rs:239`），抽出为 `pub(crate)` 泛型函数只动签名，不动体。
- `tokio-rustls` 0.26 与 `rustls` 0.23 已在 `kim-ws` 验证过版本兼容。
- **Feasible with caveats:** 示例里的 `acquire_owned().await` 已改为 `try_acquire_owned`；TLS 前端必须共享 `FrontendState`。

## Current Surface Inventory

- `crates/kim-tcp/src/conn.rs` — `TcpConn { stream: TcpStream }`；`TcpReadHalf` / `TcpWriteHalf` 持 `ReadHalf<TcpStream>` / `BufWriter<WriteHalf<TcpStream>>`；`TcpConn::new` 里 `set_nodelay(true)`
- `crates/kim-tcp/src/conn.rs` — 自由函数 `fill_and_decode<R: AsyncReadExt + Unpin>` / `write_frame_parts<W: AsyncWriteExt + Unpin>`（已泛型，不动）
- `crates/kim-tcp/src/server.rs:239` — 私有 `async fn handle_conn(stream: TcpStream, peer: SocketAddr, ctx: ConnCtx)`
- `crates/kim-tcp/src/server.rs` — `TcpServer`：单 listener accept、`JoinSet` spawn、无连接上限、无 keepalive
- `crates/kim-tcp/src/client.rs` — `TcpClient` / `TcpDialer`（返回具体 `TcpConn`）；`IdentityDialer`
- `crates/kim-container/src/dialer.rs` — `InnerTcpDialer: TcpDialer`，返回 `TcpConn::new(stream)`
- `crates/kim-ws/src/conn.rs` — `WsConn<S>`：bound 写在 impl 上的参照模板
- `crates/kim-ws/tests/wss.rs` — TLS e2e 测试模板（rcgen 自签 + `TlsAcceptor` + 客户端 `ClientConfig`）
- `services/tgateway/src/main.rs` — 18 行：裸 `TcpServer::bind` + `run_gateway`，无 TLS、无配置
- `services/gateway/src/run.rs:145` — `run_gateway<S: Server + ...>` 已是泛型，tgateway 无需改 gateway crate
- `services/chat/src/main.rs:254` — `TcpServer::bind`（明文内部链路，不动）
- `deploy/` — 无 tgateway 容器/配置（tgateway 今天不在 compose；TLS 后补部署面在本切片 Phase 6 只加配置文件示例，不进 compose 默认）

## Design

### 决策

1. **泛型 `TcpConn<S>`，对齐 `WsConn<S>`**：struct 不带 bound，`impl<S> TcpConn<S> where S: AsyncRead + AsyncWrite + Unpin` 放 `into_split`；`impl Conn for TcpConn<S>` 再加 `Send + 'static`。拒绝 `trait Io` / `Box<dyn Io>`——本仓库没有「同一 listener 混明文与 TLS」的需求，热路径不吃 vtable。拒绝把 `TcpServer` 做成 `TcpServer<S>`——那会要求 `Server` trait 持有泛型参数，炸掉 `Arc<dyn Server>`（gateway/chat 的 `container.attach_server(Arc<dyn Server>)` 全部受影响）。
2. **明文 `new(stream)` 保留；泛型构造叫 `with_peer`**：`TcpConn::new(TcpStream)` 继续 `set_nodelay` + `peer_addr`（现 5 处调用方零改动）。`impl<S> TcpConn<S>` 用 `with_peer(stream: S, peer: Option<String>)`，TLS 路径走它。拒绝泛型 `new(stream, peer)`——会破坏 `TcpConn::new(stream)`。
3. **keepalive 用 `socket2` 0.6 `TcpKeepalive`，features = `["all"]`**：`SockRef::set_tcp_keepalive(&TcpKeepalive::new().with_time(idle).with_interval(interval).with_retries(retries))`。`retries` 在非 Linux 上 `with_retries` 可能 cfg 掉，用 `cfg` 包装，不要调用不存在的 `set_tcp_keepcnt`。默认 idle 30s / interval 10s / retries 3，可关。reuseport 本切片**不做**。
4. **连接上限 `Option<Arc<Semaphore>>`，满了立刻关，不排队**：`None` = 不限制（默认，**禁止** `Semaphore::new(usize::MAX)`——超过 `Semaphore::MAX_PERMITS`（`usize::MAX >> 3`）会 panic）。`Some(n)` 用 `try_acquire_owned()`：拿不到 → `warn!` + `shutdown` 流 + `continue`，**不要** `acquire_owned().await`（那会暂停 accept 循环，与「满了立即关闭」相反）。permit 在连接任务结束时 drop。
5. **TGateway 自管 TLS accept；抽出 `FrontendState`，不是只 `take_listener()`**：`TcpServer` 的 `acceptor` / `messages` / `states` / `channels` / `login_wait` / `opts` 是私有字段（`server.rs:24-30`）。TLS 前端拿了 listener 仍拼不出 `ServeConnCtx`。`FrontendState` 为 `Arc`，明文 `TcpServer::start` 与 `TlsFrontend::start` 共用。`take_listener` 只把 `Mutex<Option<TcpListener>>` 取出一次。TLS 握手：`timeout(handshake_wait, acceptor.accept(stream))`，默认 10s，超时 drop permit（测慢握手）。证书路径只在 tgateway 配置，**不进 `kim-tcp`**。
6. **TLS 库选 `tokio-rustls` + `rustls-pemfile`**：与 `kim-ws` 的 rustls 0.23 系对齐；拒绝 `native-tls`/OpenSSL/stunnel（gaps 库策略明确排除）。
7. **BufWriter 1024 → 8192**：一次 frame（header 5B + payload 常见 <4KiB）大多单次 flush；8KiB 覆盖大多数消息且不放大内存。拒绝 16KiB——每连接多占 8KiB 无证据收益。vectored write 不做（稳定 tokio 需要 `tokio_unstable`，gaps 已拍板）。
8. **reuseport 不做、ChannelMap 分片不做**：next-stage「vectored write、ChannelMap 分片、jemalloc、一致哈希、io_uring：有 flamegraph 再做」。本切片只落 keepalive + 连接上限 + TLS。

### 类型定义

```rust
// crates/kim-tcp/src/conn.rs
pub struct TcpConn<S> { stream: S, read_buf: BytesMut, peer: Option<String> }

pub struct TcpReadHalf<S> { stream: ReadHalf<S>, read_buf: BytesMut }
pub struct TcpWriteHalf<S> { stream: BufWriter<WriteHalf<S>> }

/// 明文别名：内部链路（Chat、TcpClient、InnerTcpDialer）继续用它。
pub type PlainTcpConn = TcpConn<tokio::net::TcpStream>;
pub type PlainTcpReadHalf = TcpReadHalf<tokio::net::TcpStream>;
pub type PlainTcpWriteHalf = TcpWriteHalf<tokio::net::TcpStream>;

impl<S> TcpConn<S>
where S: AsyncRead + AsyncWrite + Unpin {
    pub fn with_peer(stream: S, peer: Option<String>) -> Self;
    pub fn into_split(self) -> (TcpReadHalf<S>, TcpWriteHalf<S>);
}

impl TcpConn<tokio::net::TcpStream> {
    /// 保留旧签名：set_nodelay + peer_addr。
    pub fn new(stream: tokio::net::TcpStream) -> Self;
}

#[async_trait]
impl<S> Conn for TcpConn<S>
where S: AsyncRead + AsyncWrite + Unpin + Send + 'static { /* 与今体相同 */ }

#[async_trait]
impl<S> Conn for TcpReadHalf<S>
where S: AsyncRead + AsyncWrite + Unpin + Send + 'static { /* 同上 */ }

#[async_trait]
impl<S> Conn for TcpWriteHalf<S>
where S: AsyncRead + AsyncWrite + Unpin + Send + 'static { /* 同上 */ }
```

```rust
// crates/kim-tcp/src/server.rs —— 抽出的入口，tgateway 复用
pub struct ServeConnCtx {
    pub acceptor: Arc<dyn Acceptor>,
    pub messages: Option<Arc<dyn MessageListener>>,
    pub states: Option<Arc<dyn StateListener>>,
    pub channels: ChannelMap,
    pub login_wait: Duration,
    pub opts: ChannelOpts,
}

/// 明文入口（等价旧私有 handle_conn）：TcpServer::start 与测试用。
pub async fn serve_tcp_conn(
    stream: tokio::net::TcpStream, peer: SocketAddr, ctx: ServeConnCtx,
) -> Result<(), Error>;

/// 泛型入口：调用方先 `TcpConn::with_peer`。
pub async fn serve_conn<S>(
    conn: TcpConn<S>, ctx: ServeConnCtx,
) -> Result<(), Error>
where S: AsyncRead + AsyncWrite + Unpin + Send + 'static;
```

```rust
// crates/kim-tcp/src/opts.rs —— 新文件
#[derive(Clone)]
pub struct SocketOpts {
    pub keepalive: Option<Keepalive>,   // None = 不设
}
#[derive(Clone, Copy)]
pub struct Keepalive {
    pub idle: Duration,      // 默认 30s
    pub interval: Duration,  // 默认 10s
    pub retries: u32,        // 默认 3
}
impl SocketOpts { pub fn apply(&self, sock: &socket2::Socket) -> io::Result<()>; }
impl Default for SocketOpts { /* keepalive: Some(默认值) */ }
```

```rust
// services/tgateway/src/main.rs —— 目标形状
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // tracing init、load_config(&path) 同今
    let tls = load_tls(&cfg)?;               // None = 明文
    let server = TcpServer::bind(&cfg.listen).await?;
    server.set_max_connections(cfg.max_connections);
    match tls {
        None => run_gateway(cfg, server).await,        // 现路径
        Some(acceptor) => {
            // TGateway 自己 accept：socket opts → TLS accept → serve_conn。
            // Server trait 实现仍然需要（run_gateway 内部 push/close_channel 走
            // ChannelMap），所以包一个 TlsTcpServer：bind 只占端口，start() 里
            // 自己 accept TLS 并调 kim_tcp::serve_conn。
            let server = TlsFrontend::wrap(server, acceptor, cfg);
            run_gateway(cfg, server).await
        }
    }
}
```

`TlsFrontend` 实现 `Server` trait：`set_*` 写进共享 `FrontendState`（与明文 `TcpServer` 同一份 `Arc`），`start()` 用 `take_listener` 取出 listener 后自己跑 accept 循环。禁止「只 take_listener、再试图读 TcpServer 私有字段拼 ServeConnCtx」。

### 使用示例

```rust
// services/tgateway/src/tls.rs（TLS 分支核心）
let acceptor = tokio_rustls::TlsAcceptor::from(server_config);
let listener = state.take_listener().await
    .ok_or("tls frontend requires the listener")?;
loop {
    tokio::select! {
        _ = shutdown.notified() => break,
        accepted = listener.accept() => {
            let (stream, peer) = accepted?;
            socket_opts.apply(&socket2::SockRef::from(&stream))?;
            let permit = match limit.as_ref() {
                None => None,
                Some(sem) => match sem.clone().try_acquire_owned() {
                    Ok(p) => Some(p),
                    Err(_) => {
                        warn!("max connections reached");
                        let _ = stream.shutdown().await;
                        continue;
                    }
                },
            };
            let acceptor = acceptor.clone();
            let ctx = state.serve_ctx();
            tasks.spawn(async move {
                let _permit = permit;
                match timeout(handshake_wait, acceptor.accept(stream)).await {
                    Ok(Ok(tls)) => {
                        let conn = TcpConn::with_peer(tls, Some(peer.to_string()));
                        let _ = kim_tcp::serve_conn(conn, ctx).await;
                    }
                    Ok(Err(err)) => warn!(%err, "tls handshake failed"),
                    Err(_) => warn!("tls handshake timeout"),
                }
            });
        }
    }
}
```

## Phased Implementation

### Phase 1: `TcpConn<S>` 泛化（kim-tcp，无行为变更）

- **File: `crates/kim-tcp/src/conn.rs`**
  - `TcpConn` → `TcpConn<S>`；字段 `stream: S`。`TcpReadHalf<TcpStream>` → `TcpReadHalf<S>`，`TcpWriteHalf` 同理。
  - `impl<S> TcpConn<S> where S: AsyncRead + AsyncWrite + Unpin`：`with_peer(stream, peer)`、`into_split`（`tokio::io::split`）。
  - `impl TcpConn<TcpStream>`：保留 `new(stream)`（`set_nodelay` + peer_addr）。
  - 三个 `impl Conn` 块加 `<S>` 与 `where S: AsyncRead + AsyncWrite + Unpin + Send + 'static`。`shutdown` 对泛型 `S` 用 `AsyncWriteExt::shutdown`（TLS 流会发 close_notify，正确）。
  - 别名 `PlainTcpConn` / `PlainTcpReadHalf` / `PlainTcpWriteHalf`。
  - `fill_and_decode` / `write_frame_parts` 不动（已泛型）。
  - `TcpWriteHalf` 的 `BufWriter::with_capacity(1024, …)` → `8192`。
- **File: `crates/kim-tcp/src/lib.rs`** — 导出新别名与 `conn::{TcpReadHalf, TcpWriteHalf}`（已是）。
- **File: `crates/kim-tcp/src/client.rs`** — 返回/字段类型可写 `PlainTcpConn`；调用仍 `TcpConn::new(stream)`。
- **File: `crates/kim-container/src/dialer.rs`** — 继续 `TcpConn::new(stream)`。
- 验证：`cargo test -p kim-tcp -p kim-container && cargo clippy -p kim-tcp -p kim-container -- -D warnings`。`crates/kim-tcp/tests/echo.rs` 不改（走公开 API，应继续过）。

### Phase 2: 抽 `serve_conn<S>` + socket 选项 + 连接上限

- **File: `crates/kim-tcp/src/server.rs`**
  - 私有 `handle_conn` → `pub async fn serve_tcp_conn`（明文，`TcpConn::new`）+ `pub async fn serve_conn<S>`（泛型，`TcpConn::with_peer`）。
  - `ConnCtx` → 共享 `FrontendState`（`Arc`）+ `ServeConnCtx` 由它导出。明文 `TcpServer` 与 `TlsFrontend` 都持这份 state。
  - `TcpServer` 增加：`set_max_connections(Option<usize>)`（`None` = 不建 Semaphore）、`set_socket_opts`、`take_listener()`。
  - accept 循环：`socket_opts.apply`；有限连接用 `try_acquire_owned`，失败立刻关流。
  - `start()` 末尾保持现有 drain 逻辑。
- **File: `crates/kim-tcp/src/opts.rs`（新）** — `SocketOpts` / `Keepalive`；`apply` 走 `TcpKeepalive::with_time/with_interval/with_retries`，**不用** `set_tcp_keepcnt`。
- **File: `crates/kim-tcp/Cargo.toml`** — `socket2 = { version = "0.6", features = ["all"] }`。
- **File: `crates/kim-tcp/src/lib.rs`** — 导出 `serve_conn` / `serve_tcp_conn` / `ServeConnCtx` / `SocketOpts` / `Keepalive`。
- 验证：`cargo test -p kim-tcp && cargo clippy -p kim-tcp -- -D warnings`。

### Phase 3: TGateway TLS

- **File: `services/tgateway/Cargo.toml`** — 加 `kim-core`、`tokio-rustls = "0.26"`、`rustls = "0.23"`（default-features = false, features = ["std","tls12","ring"]，对齐 kim-ws）、`rustls-pemfile = "2"`、`socket2 = "0.6"`。
- **File: `services/tgateway/src/main.rs`**
  - 配置：`load_config` 读到 `GatewayConfig`（gateway 的 toml），tgateway 追加自有键 `tls_cert` / `tls_key` / `max_connections`——gateway 的 `SelfSection` 是 serde `Deserialize`，未知字段会被忽略吗？**不会**（serde 默认 deny_unknown_fields 关闭，即忽略未知字段），所以 tgateway 用自己的 `File` 结构：内嵌 `#[serde(flatten)] this: GatewayConfig` 行不通（GatewayConfig 不是无标签可展平的）——改为：tgateway 读同一个 toml 文件两次（gateway 的 `load_config` + 自己的 `TlsSection { tls_cert, tls_key, max_connections }`），简单且不动 gateway crate。
  - `load_tls(&cfg) -> Result<Option<TlsAcceptor>>`：cert/key 路径读 PEM（`rustls_pemfile::certs` / `private_key`），`ServerConfig::builder().with_no_client_auth().with_single_cert`；空路径 → `None`。
  - `TlsFrontend`（`tls.rs`）：持 `Arc<FrontendState>`；`set_*` 写 state；`start()` = `take_listener` + accept（socket opts + `try_acquire_owned` + `timeout(handshake_wait, TlsAcceptor::accept)` + `serve_conn`）；`push`/`close_channel` 走 state 里的 `ChannelMap`。加测试：满连接立即拒绝；慢 TLS 握手超时后 permit 释放、新连接可进。
  - 明文分支保持 `run_gateway(cfg, server)` 原样。
- **File: `deploy/tgateway.toml`（新）** — 复制 `gateway.toml` 改 `service_id = "tgw-1"` / `service_name = "tgateway"` / `listen = "0.0.0.0:8003"`，加 `tls_cert = "/etc/kim/tls/tgateway.pem"`、`tls_key = "/etc/kim/tls/tgateway-key.pem"`、`max_connections = 10000`。**不进 compose 默认**（tgateway 本就不在 compose；文档引用即可）。
- 验证：`cargo build -p tgateway`；本机明文跑 `tgateway deploy/tgateway-tls-off.toml` + `pkt-client` 冒烟（等价 gateway）。

### Phase 4: e2e 测试

- **File: `crates/kim-tcp/tests/tls.rs`（新）** — 仿 `crates/kim-ws/tests/wss.rs`：
  - rcgen 自签证书（dev-dependency `rcgen = "0.13"` 加进 kim-tcp）。
  - 起 `TcpServer`（占位 listener）+ TLS 前端 accept 循环（复刻 tgateway 形状，或直接构造 `serve_conn` 调用），echo Acceptor。
  - 客户端 `tokio::net::TcpStream::connect` + `tokio_rustls::TlsConnector` + `TcpConn::new` → 握手 → echo 断言。
  - 明文回归：现 `tests/echo.rs` 覆盖。
  - keepalive/上限单测：`SocketOpts::apply` 对 `TcpStream` 设置后 `getsockopt` 回读（Linux/macOS 均可读回 idle）；`set_max_connections(1)` 下第二个连接被立即关闭的断言。
- 验证：`cargo test -p kim-tcp --test echo --test tls && cargo clippy --workspace --all-targets -- -D warnings`。

### Phase 5: gateway/chat 接线与文档

- **File: `services/gateway/src/run.rs`** — `run_gateway` 不改（`S: Server`）。可在 `Server` 已有 `set_max_connections` 之外无动作。
- **File: `services/chat/src/main.rs`** — `TcpServer::bind` 后加 `server.set_socket_opts(SocketOpts::default())`（内部链路也吃 keepalive，检测半死连接）；不设连接上限（内网）。
- **File: `docs/architecture.md`** — 更新 tgateway TLS 已落地（原文写「公网 TGateway TLS 未做」）。
- **File: `docs/production-gaps.md`** — G-34 拆分：TLS/keepalive/连接上限已关，reuseport/vectored 移入「延后」表（本来就在）。
- **File: `docs/impl/README.md`** — 记录 B3 合入。
- 验证：全量 `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && env -u REDIS_URL cargo test --workspace`。

## Architectural Notes

- **Semver**：workspace 内部 crate。明文 `TcpConn::new(TcpStream)` 签名不变。
- **不建 `trait Io`**：泛型路径 `where` 直接约束；`Box<dyn Io>` 仅「混明文与 TLS listener」需要，不存在该需求。
- **`Send + 'static` bound 的必要性**：`async_trait` 生成的 future 要跨 `Channel::pair` 的 `spawn`；`into_split` 只需 `Unpin`。与 `WsConn<S>` 完全同构。
- **socket 选项位置**：明文 `TcpConn::new` 保 `set_nodelay`；keepalive 在 accept 循环对 `SockRef` 设置（TLS 包裹前）。
- **明确不改**：`kim-ws`（保持明文 + Caddy 终止）、Chat 业务 handler、ACK 语义、`sdk/*`、compose 默认端口。vectored write / reuseport / ChannelMap 分片不做（flamegraph 前）。
- **新依赖**：`socket2 0.6`（kim-tcp + tgateway）、`tokio-rustls 0.26` + `rustls-pemfile 2`（tgateway only）、`rcgen 0.13`（kim-tcp dev）。均为 gaps 库策略白名单内。
- **回滚**：tgateway `tls_cert` 置空即回明文，同一二进制。

## File Change Summary

- `crates/kim-tcp/Cargo.toml` -- 加 socket2；dev 加 rcgen、tokio-rustls
- `crates/kim-tcp/src/conn.rs` -- `TcpConn<S>` 泛化 + `with_peer` + 保留 `new(TcpStream)` + BufWriter 8KiB
- `crates/kim-tcp/src/opts.rs` -- 新：SocketOpts/Keepalive（socket2 `TcpKeepalive`）
- `crates/kim-tcp/src/server.rs` -- FrontendState、serve_conn<S>、Option<Semaphore>+try_acquire、socket_opts、take_listener
- `crates/kim-tcp/src/lib.rs` -- 导出新符号
- `crates/kim-tcp/src/client.rs` -- PlainTcp* 类型替换
- `crates/kim-tcp/tests/tls.rs` -- 新：TLS e2e + keepalive + 连接上限测试
- `crates/kim-container/src/dialer.rs` -- 仍 `TcpConn::new(stream)`（无行为变更）
- `services/tgateway/Cargo.toml` -- TLS 依赖
- `services/tgateway/src/main.rs` -- TLS 前端 + 配置
- `services/tgateway/src/tls.rs` -- 新：load_tls + TlsFrontend
- `services/chat/src/main.rs` -- set_socket_opts(default)
- `deploy/tgateway.toml` -- 新：tgateway 配置示例（不进 compose）
- `docs/architecture.md` -- tgateway TLS 状态更新
- `docs/production-gaps.md` -- G-34 部分关闭
- `docs/impl/README.md` -- B3 记录
