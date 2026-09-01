# Mobile client（已落地）

对照 Web SDK（[web-sdk.md](web-sdk.md)）：桌面仍是 `sdk/web`。App 长连接本里程碑走 **WGateway WSS/WS**，不是 TGateway TCP / 自定义 TLS 端口 / QUIC。

`kim-client` 是 TDLib 形：session / login / talk / ack 在 Rust。Flutter 只是壳。

`kim-client` 有 `inbox_list` / `history` / `offline_index` / `offline_content`、统一 `send_message(dest, kind, content, client_id)`，以及 `SessionSupervisor`（重连退避 + `SyncEngine` 分页补拉，persist-then-ack）。Flutter FFI `KimApi` 持 supervisor：`start/stop/link_state/session_events/sync_confirm/notify_radio_up/send_message/history/inbox`。Dart `outbox` 入队即落库并用稳定 `client_id` 上行。G-14 仍开：Web `isRetryable` 未对齐；聊天页仍用 flutter_chat_ui，图片/视频发送未全部走 outbox 的失败重试 UI。

## Crate

`crates/kim-client`。业务只碰 [`kim_core::Conn`]。本 PR 的 Conn 实现是 `kim_ws::connect_ws`（`ws://` 明文 Upgrade，`wss://` 先 TLS）。

```rust
let mut cli = KimClient::new(ClientConfig::local(token)); // ws://127.0.0.1:8001/
// 或 ClientConfig::production(token)  →  wss://kim.ainexc.com/
cli.connect().await?;          // HTTP Upgrade；token 不进 URL
let session = cli.login().await?; // 第一帧 JWT login.signin
cli.ping().await?;
cli.send_message("bob", INBOX_KIND_USER, OutgoingContent::Text("hello".into()), client_id).await?;
cli.inbox_list(200).await?;
cli.history("bob", INBOX_KIND_USER, 0, 50).await?;
cli.ack_batch(&[message_id]).await?;
cli.recv().await?;             // Push / Kickout
cli.disconnect().await?;
// 或 SessionSupervisor::start(config)：connect → login → sync → recv，断线退避重连
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

FFI：`sdk/mobile/rust`（`kim_client_ffi`）用 **flutter_rust_bridge 2.13 Native Assets** 调 `kim-client`。`KimBridge.ffiReady == true`。账号 HTTP 走 `KimAuth`（`register/login/logout/change_password`），长连接走 `KimApi.start`（SessionSupervisor）/ `sessionEvents` / `sendMessage` / `syncConfirm` / `stop`。编译走 `sdk/mobile/hook/build.dart`（`flutter_rust_bridge_hooks` / native_toolchain_rust），不是 `rust_builder` + cargokit / ffiPlugin。`rust/rust-toolchain.toml` 钉具体 toolchain。本仓库 workspace **不**收这个 crate（`unsafe` 生成代码），避免 `unsafe_code = deny`。

`KimApi` 另有 `friendRequest` / `friendAccept` / `friendReject` / `friendList` / `friendIncoming` / `searchUsers`（列表为 JSON）。没有 NDK / Xcode 时仍可用 CLI 验证协议。

### Flutter 壳现在有什么

- `path_provider` → `KimPaths`（documents / support / cache / temp）。`support/kim-cache.db` 是会话 + 消息 SQLite（`package:sqlite3` 3.x 自带 native lib）。第一次打开会把旧的 `shared_preferences` JSON 导进去。**没有**把 data-dir 传进 FFI。
- 登录后 `KimApi.start` 拉起 `SessionSupervisor`（connect → login → sync → recv，断线退避）。Dart `linkProvider` 订阅 `sessionEvents`：Inbox 合并线程、Talk 落 SQLite 后 `sync_confirm`，不再 8s ping。Flutter 不自己开 WebSocket。
- 登录 / 注册 / 退出 / 改密：Rust `kim_client::AuthClient` 发 uncompressed protobuf，`User-Agent` / `Accept` / `Content-Type` / `Accept-Language` 由客户端设置。reqwest `gzip` 只解压响应（`Accept-Encoding: gzip`）；**不**给请求体加 `Content-Encoding`。Caddy `encode gzip zstd` 压的是响应；Royal/axum 没有 `CompressionLayer`，也不解压请求 gzip。
- `flutter_secure_storage`：JWT 只进 Keychain / Android Keystore（v11 RSA-OAEP+AES-GCM，替代已弃用的 EncryptedSharedPreferences）。`shared_preferences` 存 WGateway URL、Royal HTTP origin、account、dest；token 不进 prefs。
- `connectivity_plus`：离线横幅。不是 Dart socket。
- `permission_handler`：通知权限在登录成功后问一次，不挡 `runApp` splash；相机 / 麦克风 / 相册 **不**在启动时请求。拍照 / 选相册走自研插件 `sdk/mobile/plugins/kim_media_picker`（Android CameraX + MediaStore，iOS AVFoundation + PhotoKit）。相册 API：`pickSingle` / `pickMultiple`。拍摄 API：`takePhoto` / `takeVideo` / `capture`（默认 mixed：点按拍照、长按录像，录像模式点击开停）。权限在打开拍摄 / 相册时由原生页申请。头像：点「我」页头像 → 拍照或相册 → `POST upload.kim.ainexc.com/v1/objects` → `chat.user.update` 写 avatar URL，会话 / 通讯录 / 聊天都读这个 URL。
- `package_info_plus` / `intl`：版本号、时间格式。
- 产品 UI：`go_router` 底部三栏（消息 / 通讯录 / 我）+ 会话页；`flutter_riverpod` **3.4**（`AuthNotifier` + `linkProvider` 镜像 supervisor、`threadsProvider` / `threadMessagesProvider.family` / `outboxProvider`、`Mutation` 管登录/发消息/好友副作用、`kimRetry` 只重试瞬时网络错误、`ref.mounted` 挡住作废的 async）。`linkProvider` 与 `outboxProvider` 必须在 `KimApp` 根上 watch，不能放进 IndexedStack tab（3.0 会 pause 离屏 listener）。`flutter_chat_ui` 仍驱动聊天页（Phase 6 才换成自研 ChatList），数据来自 `threadMessagesProvider`；`flex_color_scheme`、`flutter_animate`、`wolt_modal_sheet`、`flutter_slidable`、`toastification`、`skeletonizer`、`lucide_icons_flutter` 负责主题、动效和常见交互。主题是 **Material 3**（跟随系统亮暗）。聊天 / 改密走宽左缘跟手返回（约 80px，不靠 iOS 20px / Android 系统预测性返回）。不用 Cupertino 当 app theme，不用 Dart `web_socket_channel`。`main()` 先 `runApp` splash，bootstrap（路径/SQLite/FFI）完成后再进 `KimApp`。登录后 supervisor 自动连上。不再露出 ping / talk 调试按钮。
- 会话列表和消息缓存在 SQLite（按账号，增量 `upsert` + `loadMessagesPage`）。文本发送走 Dart outbox（sending 落库 → `sendMessage` 稳定 `client_id` → sent/failed，上线重放）。图片：`uploadImage` → `sendMessage` image content（`type=2`，`extra={"w","h"}`）。列表缩略图走 `cached_network_image`（内存按显示宽解码 + 磁盘缓存，与预览共用 `CachedNetworkImageProvider`）。点击全屏：`photo_view`（pinch / 双击缩放、拖动）+ `dismissible_page`（纵向滑关闭、透明路由 Hero）。通讯录 / 好友申请 / 搜索走 `kim-client` 的 `chat.friend.*` 与 `chat.user.search`。私聊发送前要求已是好友（`NotFriends=109` 时输入栏换成加好友）。
- Android **main** `AndroidManifest` 声明 `INTERNET`（release WSS 以前缺这个会挂）。`usesCleartextTraffic` 仅 debug/profile，给本地 `ws://127.0.0.1:8001`。`allowBackup="false"`。
- iOS `NSAllowsLocalNetworking`；**不**设 `NSAllowsArbitraryLoads`。`NSCameraUsageDescription` / `NSPhotoLibraryUsageDescription` 给拍摄和相册。
- Xcode 编 iOS 时 Run Script 走 `ios/Scripts/run_xcode_backend.sh`，把 `~/.cargo/bin` 加进 PATH。否则 GUI 里找不到 `rustup`，`PhaseScriptExecution` 会以 native assets 失败退出。打开 `ios/Runner.xcworkspace`（不要开 `Runner.xcodeproj`）。`Pods/` 不进 git，Xcode 编译前要 `cd ios && pod install`，否则 `[CP] Check Pods Manifest.lock` 会报 sandbox 不同步。插件走 SPM。Podfile **不要** `use_frameworks!`，链的是 `libPods-Runner.a`；链 `Pods_Runner.framework` 会 `ld: framework 'Pods_Runner' not found`。

## 非目标

改 `sdk/web`、deploy、TGateway、把 JWT 放进 Upgrade URL、在 `kim-ws` 里写登录、Dart 侧 WebSocket / TCP / QUIC、Firebase / FCM、启动时要相机权限。服务端好友图 / 群 / 历史同步仍走后续 FFI，不在本壳伪造。
