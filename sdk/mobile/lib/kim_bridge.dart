/// Dart shell around `kim-client` via flutter_rust_bridge 2.13.
/// Session / login / talk / Royal HTTP stay in Rust. Do not expand FFI here.
library;

import 'src/rust/api/auth.dart' as rust_auth;
import 'src/rust/api/client.dart';
import 'src/rust/frb_generated.dart';

class KimAuthSession {
  const KimAuthSession({
    required this.token,
    required this.exp,
    required this.account,
  });

  final String token;
  final int exp;
  final String account;
}

/// Royal account HTTP. Tests inject a fake; the app uses [KimBridge].
abstract class KimAuthPort {
  Future<KimAuthSession> login({
    required String origin,
    required String userAgent,
    required String account,
    required String password,
  });

  Future<KimAuthSession> register({
    required String origin,
    required String userAgent,
    required String account,
    required String password,
  });

  Future<void> logout({
    required String origin,
    required String userAgent,
    required String token,
  });

  Future<void> changePassword({
    required String origin,
    required String userAgent,
    required String token,
    required String oldPassword,
    required String newPassword,
  });

  String httpOriginFromWs(String wsUrl);
}

class KimBridge implements KimAuthPort {
  static const flutterPin = '3.47.2';
  static const ffiReady = true;

  static bool _inited = false;
  KimApi? _api;

  /// Last WGateway URL passed to [connect]. Not a second source of truth —
  /// [SettingsStore] persists it.
  String? lastUrl;

  String get ffiStatus => 'FFI: kim-client via flutter_rust_bridge 2.13';

  Future<void> _ensure() async {
    if (_inited) {
      return;
    }
    await RustLib.init();
    _inited = true;
  }

  rust_auth.KimAuth _auth(String origin, String userAgent) {
    return rust_auth.KimAuth(baseUrl: origin, userAgent: userAgent);
  }

  KimAuthSession _session(rust_auth.AuthSession s) {
    return KimAuthSession(
      token: s.token,
      exp: s.exp.toInt(),
      account: s.account,
    );
  }

  @override
  Future<KimAuthSession> login({
    required String origin,
    required String userAgent,
    required String account,
    required String password,
  }) async {
    await _ensure();
    return _session(
      _auth(origin, userAgent).login(account: account, password: password),
    );
  }

  @override
  Future<KimAuthSession> register({
    required String origin,
    required String userAgent,
    required String account,
    required String password,
  }) async {
    await _ensure();
    return _session(
      _auth(origin, userAgent).register(account: account, password: password),
    );
  }

  @override
  Future<void> logout({
    required String origin,
    required String userAgent,
    required String token,
  }) async {
    await _ensure();
    _auth(origin, userAgent).logout(token: token);
  }

  @override
  Future<void> changePassword({
    required String origin,
    required String userAgent,
    required String token,
    required String oldPassword,
    required String newPassword,
  }) async {
    await _ensure();
    _auth(origin, userAgent).changePassword(
      token: token,
      oldPassword: oldPassword,
      newPassword: newPassword,
    );
  }

  @override
  String httpOriginFromWs(String wsUrl) {
    return rust_auth.httpOriginFromWs(wsUrl: wsUrl);
  }

  Future<String> connect(String url, String token, {required String userAgent}) async {
    if (token.trim().isEmpty) {
      throw StateError('JWT required (Royal /login). Do not mint in the app.');
    }
    if (!(url.startsWith('ws://') || url.startsWith('wss://'))) {
      throw StateError('url must be ws:// or wss:// (WGateway only)');
    }
    await _ensure();
    lastUrl = url;
    _api = KimApi(url: url, token: token, userAgent: userAgent);
    return _api!.connect();
  }

  Future<String> loginWs() async {
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

