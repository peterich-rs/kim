/// Shared FFI host state for auth / client / media adapters.
library;

import 'dart:io' show Platform;

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;

import 'package:kim_mobile/core/ota_info.dart';
import 'package:kim_mobile/src/rust/api/auth.dart' as rust_auth;
import 'package:kim_mobile/src/rust/api/client.dart' as rust;
import 'package:kim_mobile/src/rust/frb_generated.dart';

class KimBridgeBase {
  static const ffiReady = true;

  static bool inited = false;
  rust.KimUiHandle? api;
  String? account;

  /// Last WGateway URL passed to [KimClientPort.startSession].
  String? lastUrl;

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

  rust_auth.KimAuth authClient(String origin, String userAgent) {
    return rust_auth.KimAuth(baseUrl: origin, userAgent: userAgent);
  }

  rust.KimUiHandle requireApi() {
    final handle = api;
    if (handle == null) {
      throw StateError('attachStore first');
    }
    return handle;
  }

  Future<void> attachStore(String dbPath) async {
    await ensure();
    api ??= rust.KimUiHandle.create();
    await api!.attachStore(dbPath: dbPath);
  }
}
