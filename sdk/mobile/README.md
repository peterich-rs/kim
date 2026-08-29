# KIM mobile shell

Flutter **3.47.2** UI around `crates/kim-client`. See [docs/mobile-client.md](../../docs/mobile-client.md).

```bash
export PATH=/workspace/flutter-3.47.2/bin:$PATH
flutter pub get
flutter run
```

Paste a Royal JWT. Default URL `wss://kim.ainexc.com/` or `ws://127.0.0.1:8001/`.

FFI is flutter_rust_bridge 2.12 (`rust/` → `KimApi`). Without a device toolchain, use:

```bash
cargo run -p kim-client-demo -- alice
```
