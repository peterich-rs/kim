/// Dart shell around `kim-client` via flutter_rust_bridge 2.12.
library;

import 'src/rust/api/client.dart';
import 'src/rust/frb_generated.dart';

class KimBridge {
  static const flutterPin = '3.47.2';
  static const ffiReady = true;

  static bool _inited = false;
  KimApi? _api;

  String get ffiStatus => 'FFI: kim-client via flutter_rust_bridge 2.12';

  Future<void> _ensure() async {
    if (_inited) {
      return;
    }
    await RustLib.init();
    _inited = true;
  }

  Future<String> connect(String url, String token) async {
    if (token.trim().isEmpty) {
      throw StateError('JWT required (Royal /login). Do not mint in the app.');
    }
    if (!(url.startsWith('ws://') || url.startsWith('wss://'))) {
      throw StateError('url must be ws:// or wss:// (WGateway only)');
    }
    await _ensure();
    _api = KimApi(url: url, token: token);
    return _api!.connect();
  }

  Future<String> login() async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return api.login();
  }

  Future<String> ping() async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return api.ping();
  }

  Future<String> talk(String dest, String body) async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return api.talkToUser(dest: dest, body: body);
  }

  Future<String> disconnect() async {
    final api = _api;
    if (api == null) {
      return 'not connected';
    }
    final out = api.disconnect();
    _api = null;
    return out;
  }
}
