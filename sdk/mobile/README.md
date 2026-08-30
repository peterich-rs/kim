# KIM mobile shell

Flutter **3.47.2** UI around `crates/kim-client`. See [docs/mobile-client.md](../../docs/mobile-client.md).

```bash
export PATH=/workspace/flutter-3.47.2/bin:$PATH
flutter pub get
flutter run
```

Login / register / logout / change-password are Rust `AuthClient` calls to Royal `/api/v1/auth/*` (protobuf, `User-Agent` set by the app). JWT stays in Keychain / Android Keystore via `flutter_secure_storage`, never committed. WGateway URL, Royal HTTP origin, account, and dest live in `shared_preferences`. Defaults: `wss://kim.ainexc.com/` + `https://kim.ainexc.com`, or local `ws://127.0.0.1:8001/` + `http://127.0.0.1:8080`.

App support dir (future SQLite / cache) comes from `path_provider` (`KimPaths`). Account HTTP, session, WS login, and talk stay in Rust; Flutter does not open Dart sockets, Dio, or a Dart WebSocket.

Theme is **Material 3** (system light/dark) plus the Flutter team `animations` package (`FadeThrough` page transitions). Cupertino is not the app theme.

Android: `INTERNET` is in the **main** manifest (release WSS). Cleartext `ws://` is debug/profile only. iOS: `NSAllowsLocalNetworking` for simulator/LAN `ws://`; no `NSAllowsArbitraryLoads`.

FFI is flutter_rust_bridge **2.13 Native Assets** (`hook/build.dart` → `rust/` `kim_client_ffi` → `KimApi`). Flutter invokes the hook via `flutter_rust_bridge_hooks` (wrapping native_toolchain_rust); there is no `rust_builder` / Cargokit / ffiPlugin. Xcode does not inherit `~/.zshrc`, so `ios/Scripts/run_xcode_backend.sh` prepends `~/.cargo/bin` (see `ios/.xcode.env`). Without a device toolchain, use:

```bash
cargo run -p kim-client-demo -- alice
```
