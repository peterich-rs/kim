/// Shared FFI host state for auth / client / media adapters.
library;

import 'dart:io' show Platform;

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;

import 'package:kim_mobile/core/ota_info.dart';
import 'package:kim_mobile/src/rust/api/client.dart' as rust;
import 'package:kim_mobile/src/rust/frb_generated.dart';

class KimBridgeBase {
  static const ffiReady = true;

  static bool inited = false;
  rust.KimUiHandle? api;
  String? account;

  String get ffiStatus => 'FFI: kim-client via flutter_rust_bridge 2.13';

  Future<void> ensure() async {
    if (inited) {
      return;
    }
    if (!kIsWeb && Platform.isAndroid) {
      final ota = await OtaBridge.status();
      final path = ota.ffiPath;
      if (ota.ffiLoadedFromOta && path != null && path.isNotEmpty) {
        await RustLib.init(externalLibrary: ExternalLibrary.open(path));
      } else {
        await RustLib.init();
      }
    } else {
      await RustLib.init();
    }
    inited = true;
    if (!kIsWeb && Platform.isAndroid) {
      await OtaBridge.markHealthy();
    }
  }

  rust.KimUiHandle requireApi() {
    final handle = api;
    if (handle == null) {
      throw StateError('attachStore first');
    }
    return handle;
  }

  /// Rust derives the store path from the platform bootstrap.
  Future<void> attachStore() async {
    await ensure();
    api ??= rust.KimUiHandle.create();
    await api!.attachStore();
  }
}
