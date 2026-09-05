/// Dart shell around `kim-client` via flutter_rust_bridge 2.13.
/// Session / login / talk / Royal HTTP stay in Rust. Do not expand FFI here.
library;

import 'dart:convert';

import 'core/format.dart';
import 'core/image_extra.dart';
import 'models/models.dart';
import 'src/rust/api/auth.dart' as rust_auth;
import 'src/rust/api/client.dart' as rust;
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
  Stream<KimEvent> sessionEvents();

  KimLinkState linkState();

  Future<void> startSession(
    String url,
    String token, {
    required String userAgent,
  });

  Future<void> stopSession();

  Future<void> syncConfirm(int cursor);

  Future<void> notifyRadioUp();

  Future<KimTalkResult> sendMessage(
    String dest,
    ThreadKind kind,
    KimOutgoingContent content, {
    required String clientId,
  });

  Future<List<KimHistoryMsg>> history(
    String dest,
    ThreadKind kind, {
    int beforeId = 0,
    int limit = 50,
  });

  Future<List<KimThread>> inboxList({int limit = 200});

  Future<void> ack(int messageId);

  Future<void> markRead(String dest, ThreadKind kind, int messageId);

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
  rust.KimApi? _api;
  Stream<KimEvent>? _events;

  /// Last WGateway URL passed to [startSession]. Not a second source of truth —
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

  rust.KimApi _require() {
    final api = _api;
    if (api == null) {
      throw StateError('startSession first');
    }
    return api;
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
  Future<void> startSession(
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
        await prev.stop();
      } catch (_) {}
    }
    final api = rust.KimApi.start(url: url, token: token, userAgent: userAgent);
    _api = api;
    _events = api.sessionEvents().map(_event);
  }

  @override
  Future<void> stopSession() async {
    final api = _api;
    _api = null;
    _events = null;
    if (api == null) {
      return;
    }
    try {
      await api.stop();
    } catch (_) {}
  }

  @override
  KimLinkState linkState() {
    final api = _api;
    if (api == null) {
      return const KimLinkState();
    }
    return KimLinkState(status: KimLinkState.statusFromLabel(api.linkState()));
  }

  @override
  Stream<KimEvent> sessionEvents() {
    return _events ?? const Stream.empty();
  }

  @override
  Future<void> syncConfirm(int cursor) async {
    await _require().syncConfirm(cursor: cursor);
  }

  @override
  Future<void> notifyRadioUp() async {
    await _require().notifyRadioUp();
  }

  @override
  Future<KimTalkResult> sendMessage(
    String dest,
    ThreadKind kind,
    KimOutgoingContent content, {
    required String clientId,
  }) async {
    final result = await _require().sendMessage(
      dest: dest,
      kind: kind == ThreadKind.group ? 1 : 0,
      content: _wire(content),
      clientId: clientId,
    );
    return KimTalkResult(
      messageId: result.messageId.toInt(),
      sendTime: sendTimeMs(result.sendTime.toInt()),
    );
  }

  rust.KimOutgoingContent _wire(KimOutgoingContent content) {
    return switch (content) {
      KimTextContent(:final text) => rust.KimOutgoingContent(
        kind: 1,
        body: text,
        extra: '',
      ),
      KimImageContent(:final url, :final width, :final height) =>
        rust.KimOutgoingContent(
          kind: 2,
          body: url,
          extra: encodeImageExtra(width: width, height: height),
        ),
      KimVideoContent(:final url) => rust.KimOutgoingContent(
        kind: 4,
        body: url,
        extra: '',
      ),
    };
  }

  @override
  Future<List<KimHistoryMsg>> history(
    String dest,
    ThreadKind kind, {
    int beforeId = 0,
    int limit = 50,
  }) async {
    final items = await _require().history(
      dest: dest,
      kind: kind == ThreadKind.group ? 1 : 0,
      beforeId: beforeId,
      limit: limit,
    );
    return [
      for (final item in items)
        KimHistoryMsg(
          messageId: item.messageId.toInt(),
          msgType: item.msgType,
          body: item.body,
          extra: item.extra,
          sender: item.sender,
          sendTime: item.sendTime.toInt(),
          direction: item.direction,
        ),
    ];
  }

  @override
  Future<List<KimThread>> inboxList({int limit = 200}) async {
    final items = await _require().inbox(limit: limit);
    return [
      for (final item in items)
        KimThread(
          id: item.dest,
          kind: item.kind == 1 ? ThreadKind.group : ThreadKind.user,
          title: item.title.isEmpty ? item.dest : item.title,
          lastBody: item.lastBody,
          lastAt: sendTimeMs(item.lastSendTime.toInt()),
          unread: item.unread,
          avatar: item.avatar,
        ),
    ];
  }

  @override
  Future<void> ack(int messageId) async {
    await _require().ack(messageId: messageId);
  }

  @override
  Future<void> markRead(String dest, ThreadKind kind, int messageId) async {
    await _require().markRead(
      dest: dest,
      kind: kind == ThreadKind.group ? 1 : 0,
      messageId: messageId,
    );
  }

  KimEvent _event(rust.KimSessionEvent push) {
    final kind = switch (push.kind) {
      'talk' => KimEventKind.talk,
      'kick' => KimEventKind.kick,
      'friend' => KimEventKind.friend,
      'friend_accepted' => KimEventKind.friendAccepted,
      'group' => KimEventKind.group,
      'token' => KimEventKind.token,
      'link' => KimEventKind.link,
      'inbox' => KimEventKind.inbox,
      'sync_progress' => KimEventKind.syncProgress,
      'sync_done' => KimEventKind.syncDone,
      'sync_failed' => KimEventKind.syncFailed,
      'auth' => KimEventKind.authExpired,
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
      state: push.state,
      attempt: push.attempt,
      inbox: [
        for (final item in push.items)
          KimThread(
            id: item.dest,
            kind: item.kind == 1 ? ThreadKind.group : ThreadKind.user,
            title: item.title.isEmpty ? item.dest : item.title,
            lastBody: item.lastBody,
            lastAt: sendTimeMs(item.lastSendTime.toInt()),
            unread: item.unread,
            avatar: item.avatar,
          ),
      ],
      pulled: push.pulled.toInt(),
      pagePending: push.pagePending,
      error: push.error,
      msgType: push.msgType,
      nickname: push.nickname,
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
    return _people(await _require().friendList());
  }

  @override
  Future<List<KimPerson>> friendIncoming() async {
    return _people(await _require().friendIncoming());
  }

  @override
  Future<List<KimPerson>> searchUsers(String query) async {
    return _people(await _require().searchUsers(query: query));
  }

  @override
  Future<void> friendRequest(String dest) async {
    await _require().friendRequest(dest: dest);
  }

  @override
  Future<void> friendAccept(String dest) async {
    await _require().friendAccept(dest: dest);
  }

  @override
  Future<void> friendReject(String dest) async {
    await _require().friendReject(dest: dest);
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
    return _person(await _require().profile(dest: dest));
  }

  @override
  Future<KimPerson> updateProfile({
    required String nickname,
    required String avatar,
    String bio = '',
  }) async {
    return _person(
      await _require().updateProfile(
        nickname: nickname,
        avatar: avatar,
        bio: bio,
      ),
    );
  }
}
