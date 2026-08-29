# KIM mobile shell

Flutter **3.47.2** UI around `crates/kim-client`. See [docs/mobile-client.md](../../docs/mobile-client.md).

```bash
export PATH=/workspace/flutter-3.47.2/bin:$PATH
flutter pub get
flutter run
```

Paste a Royal JWT (stored in Keychain / Android Keystore via `flutter_secure_storage`, never committed). Last WGateway URL and dest account live in `shared_preferences`. Default URL `wss://kim.ainexc.com/` or `ws://127.0.0.1:8001/`.

App support dir (future SQLite / cache) comes from `path_provider` (`KimPaths`). Session / login / talk stay in Rust `KimApi`; Flutter does not open Dart sockets, Dio, or a Dart WebSocket.

Theme is **Material 3** (system light/dark) plus the Flutter team `animations` package (`FadeThrough` page transitions). Cupertino is not the app theme.

Android: `INTERNET` is in the **main** manifest (release WSS). Cleartext `ws://` is debug/profile only. iOS: `NSAllowsLocalNetworking` for simulator/LAN `ws://`; no `NSAllowsArbitraryLoads`.

FFI is flutter_rust_bridge **2.13 Native Assets** (`hook/build.dart` → `rust/` `kim_client_ffi` → `KimApi`). Flutter invokes the hook via `flutter_rust_bridge_hooks` (wrapping native_toolchain_rust); there is no `rust_builder` / Cargokit / ffiPlugin. Without a device toolchain, use:

```bash
cargo run -p kim-client-demo -- alice
```
