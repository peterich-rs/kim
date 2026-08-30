/// Dart shell around `kim-client` via flutter_rust_bridge 2.13.
/// Session / login / talk / Royal HTTP stay in Rust. Do not expand FFI here.
library;

import 'dart:convert';

import 'models/models.dart';
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

/// Long-lived WGateway session. Tests inject a fake; the app uses [KimBridge].
abstract class KimClientPort {
  Future<String> connect(String url, String token, {required String userAgent});

  Future<String> loginWs();

  Future<String> ping();

  Future<String> talk(String dest, String body);

  Future<void> ack(int messageId);

  Stream<KimEvent> events();

  Future<List<KimPerson>> friendList();

  Future<List<KimPerson>> friendIncoming();

  Future<List<KimPerson>> searchUsers(String query);

  Future<void> friendRequest(String dest);

  Future<void> friendAccept(String dest);

  Future<void> friendReject(String dest);

  Future<KimPerson> profile({String dest = ''});

  Future<KimPerson> updateProfile({
    required String nickname,
    required String avatar,
    String bio = '',
  });

  Future<String> disconnect();
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

class KimBridge implements KimAuthPort, KimClientPort {
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
      await _auth(
        origin,
        userAgent,
      ).login(account: account, password: password),
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
      await _auth(
        origin,
        userAgent,
      ).register(account: account, password: password),
    );
  }

  @override
  Future<void> logout({
    required String origin,
    required String userAgent,
    required String token,
  }) async {
    await _ensure();
    await _auth(origin, userAgent).logout(token: token);
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
    await _auth(origin, userAgent).changePassword(
      token: token,
      oldPassword: oldPassword,
      newPassword: newPassword,
    );
  }

  @override
  String httpOriginFromWs(String wsUrl) {
    return rust_auth.httpOriginFromWs(wsUrl: wsUrl);
  }

  @override
  Future<String> connect(
    String url,
    String token, {
    required String userAgent,
  }) async {
    if (token.trim().isEmpty) {
      throw StateError('JWT required (Royal /login). Do not mint in the app.');
    }
    if (!(url.startsWith('ws://') || url.startsWith('wss://'))) {
      throw StateError('url must be ws:// or wss:// (WGateway only)');
    }
    await _ensure();
    lastUrl = url;
    final prev = _api;
    if (prev != null) {
      try {
        await prev.disconnect();
      } catch (_) {}
    }
    _api = KimApi(url: url, token: token, userAgent: userAgent);
    return _api!.connect();
  }

  @override
  Future<String> loginWs() async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return api.login();
  }

  @override
  Future<String> ping() async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return api.ping();
  }

  @override
  Future<String> talk(String dest, String body) async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return api.talkToUser(dest: dest, body: body);
  }

  @override
  Future<void> ack(int messageId) async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    await api.ack(messageId: messageId);
  }

  @override
  Stream<KimEvent> events() {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return api.listen().map(_event);
  }

  KimEvent _event(KimPush push) {
    final kind = switch (push.kind) {
      'talk' => KimEventKind.talk,
      'kick' => KimEventKind.kick,
      'friend' => KimEventKind.friend,
      'group' => KimEventKind.group,
      'token' => KimEventKind.token,
      _ => KimEventKind.closed,
    };
    return KimEvent(
      kind: kind,
      dest: push.dest,
      sender: push.sender,
      body: push.body,
      extra: push.extra,
      messageId: push.messageId.toInt(),
      sendTime: push.sendTime.toInt(),
      token: push.token,
      exp: push.exp.toInt(),
    );
  }

  List<KimPerson> _people(String raw) {
    final decoded = jsonDecode(raw);
    if (decoded is! List) {
      return const [];
    }
    return [
      for (final item in decoded)
        if (item is Map)
          KimPerson(
            account: '${item['account'] ?? ''}',
            nickname: '${item['nickname'] ?? ''}',
            avatar: '${item['avatar'] ?? ''}',
          ),
    ].where((p) => p.account.isNotEmpty).toList();
  }

  @override
  Future<List<KimPerson>> friendList() async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return _people(await api.friendList());
  }

  @override
  Future<List<KimPerson>> friendIncoming() async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return _people(await api.friendIncoming());
  }

  @override
  Future<List<KimPerson>> searchUsers(String query) async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return _people(await api.searchUsers(query: query));
  }

  @override
  Future<void> friendRequest(String dest) async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    await api.friendRequest(dest: dest);
  }

  @override
  Future<void> friendAccept(String dest) async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    await api.friendAccept(dest: dest);
  }

  @override
  Future<void> friendReject(String dest) async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    await api.friendReject(dest: dest);
  }

  KimPerson _person(String raw) {
    final decoded = jsonDecode(raw);
    if (decoded is! Map) {
      throw StateError('bad profile');
    }
    final account = '${decoded['account'] ?? ''}';
    if (account.isEmpty) {
      throw StateError('bad profile');
    }
    return KimPerson(
      account: account,
      nickname: '${decoded['nickname'] ?? ''}',
      avatar: '${decoded['avatar'] ?? ''}',
    );
  }

  @override
  Future<KimPerson> profile({String dest = ''}) async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return _person(await api.profile(dest: dest));
  }

  @override
  Future<KimPerson> updateProfile({
    required String nickname,
    required String avatar,
    String bio = '',
  }) async {
    final api = _api;
    if (api == null) {
      throw StateError('connect first');
    }
    return _person(
      await api.updateProfile(nickname: nickname, avatar: avatar, bio: bio),
    );
  }

  @override
  Future<String> disconnect() async {
    final api = _api;
    if (api == null) {
      return 'not connected';
    }
    final out = await api.disconnect();
    _api = null;
    return out;
  }
}
