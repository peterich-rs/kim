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
| `DEFAULT_LOCAL_HTTP_ORIGIN` | `http://127.0.0.1:8080` |
| `DEFAULT_PROD_HTTP_ORIGIN` | `https://kim.ainexc.com` |

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

无 `KIM_TOKEN` 时 demo 用 `DEMO_DEFAULT_SECRET` 本地 mint（和 `pkt-client` 一样，仅本机）。App 生产 token 走 `AuthClient` → Royal `POST /api/v1/auth/login|register`，不要写进仓库。

## Flutter 壳

路径：`sdk/mobile`。**Flutter 3.47.2**（Dart 3.13.2）。钉在 `.fvmrc` / `.fvm/fvm_config.json` / `pubspec.yaml` `environment.sdk: ^3.13.2`。

```bash
export PATH=/workspace/flutter-3.47.2/bin:$PATH   # 或 fvm
cd sdk/mobile
flutter pub get
flutter run
```

CI（`.github/workflows/ci.yml` `sdk-mobile`）：`dart format --output=none --set-exit-if-changed lib test hook`、`flutter analyze --fatal-infos --fatal-warnings`、`cargo fmt --manifest-path rust/Cargo.toml -- --check`、`flutter test`。Flutter 钉 `.fvmrc`（3.47.2）。`flutter test` 会走 Native Assets hook 编 `kim_client_ffi`，所以 job 里装 host Rust 1.95.0（`RUSTUP_TOOLCHAIN` 避免 rust-toolchain.toml 里那些 cross target）。

FFI：`sdk/mobile/rust`（`kim_client_ffi`）用 **flutter_rust_bridge 2.13 Native Assets** 调 `kim-client`。`KimBridge.ffiReady == true`。账号 HTTP 走 `KimAuth`（`register/login/logout/change_password`），长连接走 `KimApi.connect/login/ping/talkToUser/listen/ack/disconnect`。编译走 `sdk/mobile/hook/build.dart`（`flutter_rust_bridge_hooks` / native_toolchain_rust），不是 `rust_builder` + cargokit / ffiPlugin。`rust/rust-toolchain.toml` 钉具体 toolchain。本仓库 workspace **不**收这个 crate（`unsafe` 生成代码），避免 `unsafe_code = deny`。

`KimApi` 另有 `friendRequest` / `friendAccept` / `friendReject` / `friendList` / `friendIncoming` / `searchUsers`（列表为 JSON）。没有 NDK / Xcode 时仍可用 CLI 验证协议。

### Flutter 壳现在有什么

- `path_provider` → `KimPaths`（documents / support / cache / temp）。路径留在 Dart 侧，给以后 SQLite 用；**没有**把 data-dir 传进 FFI。
- 登录后 `KimApi.listen` 在 Rust 里 `recv` 推送（talk / kick / friend / token）。WGateway 连接在 `login.signin` 后拆成读写半边，所以收消息不会卡住 `talk`。Flutter 只订阅这条流，不自己开 WebSocket。
- 登录 / 注册 / 退出 / 改密：Rust `kim_client::AuthClient` 发 uncompressed protobuf，`User-Agent` / `Accept` / `Content-Type` / `Accept-Language` 由客户端设置。reqwest `gzip` 只解压响应（`Accept-Encoding: gzip`）；**不**给请求体加 `Content-Encoding`。Caddy `encode gzip zstd` 压的是响应；Royal/axum 没有 `CompressionLayer`，也不解压请求 gzip。
- `flutter_secure_storage`：JWT 只进 Keychain / Android Keystore（v11 RSA-OAEP+AES-GCM，替代已弃用的 EncryptedSharedPreferences）。`shared_preferences` 存 WGateway URL、Royal HTTP origin、account、dest；token 不进 prefs。
- `connectivity_plus`：离线横幅。不是 Dart socket。
- `permission_handler`：启动时最多问一次通知；相机 / 麦克风 / 相册 **不**在启动时请求。
- `package_info_plus` / `intl`：版本号、时间格式。
- 产品 UI：`go_router` 底部三栏（消息 / 通讯录 / 我）+ 会话页；`flutter_riverpod` **3.4**（`AuthNotifier` + `AsyncNotifier` gateway、`Mutation` 管登录/发消息/好友副作用、`kimRetry` 只重试瞬时网络错误、`ref.mounted` 挡住作废的 async）。`liveEventsProvider` 必须在 `KimApp` 根上 watch，不能放进 IndexedStack tab（3.0 会 pause 离屏 listener）。`flutter_chat_ui` 消息列表和输入栏（消息行是 Discord 式左对齐，不是 iMessage 气泡）；`flex_color_scheme`、`flutter_animate`、`wolt_modal_sheet`、`flutter_slidable`、`toastification`、`skeletonizer`、`lucide_icons_flutter` 负责主题、动效和常见交互。主题是 **Material 3**（跟随系统亮暗）。聊天 / 改密走 `CupertinoPage`（iOS 左缘返回）或 Android 预测性返回。不用 Cupertino 当 app theme，不用 Dart `web_socket_channel`。登录后自动 `connect` + `login.signin`，不再露出 ping / talk 调试按钮。
- 会话列表和消息缓存在 `shared_preferences`（按账号）。通讯录 / 好友申请 / 搜索走 `kim-client` 的 `chat.friend.*` 与 `chat.user.search`。私聊发送前要求已是好友（`NotFriends=109` 时输入栏换成加好友）。
- Android **main** `AndroidManifest` 声明 `INTERNET`（release WSS 以前缺这个会挂）。`usesCleartextTraffic` 仅 debug/profile，给本地 `ws://127.0.0.1:8001`。`allowBackup="false"`。
- iOS `NSAllowsLocalNetworking`；**不**设 `NSAllowsArbitraryLoads`。相机 / 麦 / 相册的隐私文案先不写（还没有 picker，避免未使用 key 被拒）。
- Xcode 编 iOS 时 Run Script 走 `ios/Scripts/run_xcode_backend.sh`，把 `~/.cargo/bin` 加进 PATH。否则 GUI 里找不到 `rustup`，`PhaseScriptExecution` 会以 native assets 失败退出。打开 `ios/Runner.xcworkspace`。插件走 SPM，不要把 `Pods_Runner.framework` 链进 Runner（否则 `ld: framework 'Pods_Runner' not found`）。

## 非目标

改 `sdk/web`、deploy、TGateway、把 JWT 放进 Upgrade URL、在 `kim-ws` 里写登录、Dart 侧 WebSocket / TCP / QUIC、Firebase / FCM、启动时要相机权限。服务端好友图 / 群 / 历史同步仍走后续 FFI，不在本壳伪造。
