# KIM mobile shell

Flutter **3.47.2** UI around `crates/kim-client`. See [docs/mobile-client.md](../../docs/mobile-client.md).

```bash
export PATH=/workspace/flutter-3.47.2/bin:$PATH
flutter pub get
cd ios && pod install && cd ..   # writes Pods/Manifest.lock (gitignored)
flutter run
```

Open `ios/Runner.xcworkspace` in Xcode, not `Runner.xcodeproj`. `Pods/` is gitignored, so a fresh tree needs `pod install` before the `[CP] Check Pods Manifest.lock` phase.

CI (same job as `.github/workflows/ci.yml` `sdk-mobile`):

```bash
flutter pub get --enforce-lockfile
dart format --output=none --set-exit-if-changed lib test hook
flutter analyze --fatal-infos --fatal-warnings
cargo fmt --manifest-path rust/Cargo.toml -- --check
flutter test
```

Login / register / logout / change-password are Rust `AuthClient` calls to Royal `/api/v1/auth/*` (protobuf, `User-Agent` set by the app). JWT stays in Keychain / Android Keystore via `flutter_secure_storage`, never committed. WGateway URL, Royal HTTP origin, account, and dest live in `shared_preferences`. Defaults: `wss://kim.ainexc.com/` + `https://kim.ainexc.com`, or local `ws://127.0.0.1:8001/` + `http://127.0.0.1:8080`.

App support dir (future SQLite / cache) comes from `path_provider` (`KimPaths`). Account HTTP, session, WS login, and talk stay in Rust; Flutter does not open Dart sockets or a Dart WebSocket.

The shell is a product IM UI: 消息 / 通讯录 / 我 plus a chat thread. Theme is **Material 3** (system light/dark) via `flex_color_scheme`, with `go_router` + `flutter_riverpod` 3.4 (`AuthNotifier`, `GatewayNotifier`, mutations, `kimRetry`) for navigation/state, an in-house reverse `ChatList` (Telegram tokens × Discord grouping, own teal bubbles) plus a capsule composer (`发送` / `+` → 相册 / 拍摄). Camera and album are the in-repo plugin `plugins/kim_media_picker` (Android CameraX + MediaStore, iOS AVFoundation + PhotoKit), not `image_picker`. `flutter_animate` / `wolt_modal_sheet` / `flutter_slidable` / `toastification` / `skeletonizer` / `lucide_icons_flutter` cover motion and chrome. Chat and password use a wide left-edge swipe-back (80px, follow-through) on every platform. Cupertino is not the app theme. Login auto-connects the WGateway session; there is no connect/ping debug bar.

Android: packaging is **arm64-v8a only** (`ndk.abiFilters` in `android/app/build.gradle.kts`) so release APKs do not ship armeabi-v7a / x86 / x86_64 copies of Flutter engine, Rust FFI, and sqlite3. `INTERNET` is in the **main** manifest (release WSS). Cleartext `ws://` is debug/profile only. iOS: `NSAllowsLocalNetworking` for simulator/LAN `ws://`; no `NSAllowsArbitraryLoads`.

## Android Logic SO OTA

App-store APK is primary. A small Android-only hotfix channel can swap `libapp.so` + `libkim_client_ffi.so` (arm64-v8a) after SHA-256 + Ed25519 verify. **Catalog = GitHub Releases** (`logic-ota-v{x.y.z}` tags + assets); no Royal check API. **Patch-only dynamic semver:** OTA stays within the same `x.y` from `pubspec.yaml`; `host_line` is derived `x.y` (not a separate knob). Never OTAs Flutter engine, plugins, Java/Kotlin, resources, or sqlite3 (phase 1). Platform / Manifest / permission changes bump **minor or major** and ship a full APK. Both SOs always ship together.

Design: [docs/mobile-android-so-ota.md](../../docs/mobile-android-so-ota.md). Pack/verify/publish: [ota/README.md](ota/README.md).


FFI is flutter_rust_bridge **2.13 Native Assets** (`hook/build.dart` → `rust/` `kim_client_ffi` → `KimApi`). Flutter invokes the hook via `flutter_rust_bridge_hooks` (wrapping native_toolchain_rust); there is no `rust_builder` / Cargokit / ffiPlugin. Xcode does not inherit `~/.zshrc`, so `ios/Scripts/run_xcode_backend.sh` prepends `~/.cargo/bin` (see `ios/.xcode.env`). Without a device toolchain, use:

```bash
cargo run -p kim-client-demo -- alice
```
