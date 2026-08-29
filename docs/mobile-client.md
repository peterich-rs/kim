# Mobile client（已落地）

对照 Web SDK（[web-sdk.md](web-sdk.md)）：桌面仍是 `sdk/web`。App 长连接本里程碑走 **WGateway WSS/WS**，不是 TGateway TCP / 自定义 TLS 端口 / QUIC。

`kim-client` 是 TDLib 形：session / login / talk / ack 在 Rust。Flutter 只是壳。

## Crate

`crates/kim-client`。业务只碰 [`kim_core::Conn`]。本 PR 的 Conn 实现是 `kim_ws::connect_ws`（`ws://` 明文 Upgrade，`wss://` 先 TLS）。

```rust
let mut cli = KimClient::new(ClientConfig::local(token)); // ws://127.0.0.1:8001/
// 或 ClientConfig::production(token)  →  wss://kim.ainexc.com/
cli.connect().await?;          // HTTP Upgrade；token 不进 URL
let session = cli.login().await?; // 第一帧 JWT login.signin
cli.ping().await?;
cli.talk_to_user("bob", "hello").await?;
cli.ack(message_id).await?;
cli.recv().await?;             // Push / Kickout
cli.disconnect().await?;
```

内存会话：`MemorySession { channel_id, account, token }`。

默认 URL：

| 常量 | 值 |
|---|---|
| `DEFAULT_LOCAL_URL` | `ws://127.0.0.1:8001/` |
| `DEFAULT_PROD_URL` | `wss://kim.ainexc.com/` |

`KIM_WS_URL` 可覆盖。第一帧必须是 `login.signin`（`LoginReq.token` + `device=mobile`），与 [link-layer-login.md](link-layer-login.md) 相同。移动端会话互斥，Web / desktop / cli 可以同时在线。

## TCP / QUIC 以后怎么插

换传输只加新的 `Conn` 实现（`kim-tcp` 已有；QUIC 另开 crate）。`login_on_conn` / talk 编码不改。`KimClient::connect` 里把 `connect_ws` 换成 `connect_tcp` 即可。不要把 `if talk` 写进 `WsServer`。

本 PR **不**连 TGateway。

## CLI

```bash
env -u REDIS_URL cargo test -p kim-client
cargo run -p kim-client-demo -- alice
cargo run -p kim-client-demo -- alice wss://kim.ainexc.com/
KIM_TALK_TO=bob cargo run -p kim-client-demo -- alice
```

无 `KIM_TOKEN` 时 demo 用 `DEMO_DEFAULT_SECRET` 本地 mint（和 `pkt-client` 一样，仅本机）。生产 token 走 Royal `/api/v1/auth/login`，不要写进仓库。

## Flutter 壳

路径：`sdk/mobile`。**Flutter 3.47.2**（Dart 3.13.2）。钉在 `.fvmrc` / `.fvm/fvm_config.json` / `pubspec.yaml` `environment.sdk: ^3.13.2`。

```bash
export PATH=/workspace/flutter-3.47.2/bin:$PATH   # 或 fvm
cd sdk/mobile
flutter pub get
flutter run
```

FFI：`sdk/mobile/rust`（`kim_client_ffi`）用 **flutter_rust_bridge 2.13 Native Assets** 调 `kim-client`。`KimBridge.ffiReady == true`。UI 按钮走 `KimApi.connect/login/ping/talkToUser/disconnect`。编译走 `sdk/mobile/hook/build.dart`（`flutter_rust_bridge_hooks` / native_toolchain_rust），不是 `rust_builder` + cargokit / ffiPlugin。`rust/rust-toolchain.toml` 钉具体 toolchain。本仓库 workspace **不**收这个 crate（`unsafe` 生成代码），避免 `unsafe_code = deny`。

没有 NDK / Xcode 时仍可用 CLI 验证协议。

## 非目标

改 `sdk/web`、deploy、TGateway、把 JWT 放进 Upgrade URL、在 `kim-ws` 里写登录。
