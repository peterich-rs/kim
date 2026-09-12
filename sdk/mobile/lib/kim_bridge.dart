/// Dart shell around `kim-client` via flutter_rust_bridge 2.13.
/// Session / login / talk / Royal HTTP stay in Rust. Do not expand FFI here.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io' show Platform;

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;

import 'core/format.dart';
import 'data/conversation_store.dart';
import 'core/ota_info.dart';
import 'core/image_extra.dart';
import 'core/jwt.dart';
import 'models/models.dart';
import 'src/rust/api/auth.dart' as rust_auth;
import 'src/rust/api/client.dart' as rust;
import 'src/rust/api/types.dart' as rust_types;
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

  Future<void> notifyForeground();

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

  /// Room interest enter; returns snapshot entries `{account,status,lastSeen}`.
  Future<List<Map<String, dynamic>>> roomEnter(String dest, {int kind = 0});

  Future<void> roomLeave(String dest, {int kind = 0});

  /// Fire-and-forget typing indicator for a DM thread.
  Future<void> sendTyping(String dest, {int kind = 0, bool active = true});

  Future<KimPerson> botCreate({
    required String clientProfileId,
    required String nickname,
    String avatar = '',
    String bio = '',
  });

  Future<void> botDelete(String dest);

  Future<KimPerson> botUpdate({
    required String dest,
    required String nickname,
    String avatar = '',
    String bio = '',
  });

  Future<KimTalkResult> botReply({
    required String dest,
    required String body,
    required int inReplyTo,
    required String clientId,
  });

  Future<List<KimBotPendingItem>> botPending(String dest, {int limit = 20});

  Future<void> attachStore(String dbPath);

  bool get rustStoreAttached;

  Future<void> persistTalks(
    Iterable<KimChatMsg> msgs, {
    required UnreadPolicy policy,
  });

  Future<void> persistInboxThreads(List<KimThread> threads);
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
  rust.KimSdkHandle? _api;
  Stream<KimEvent>? _events;
  String? _account;

  /// Last WGateway URL passed to [startSession]. Not a second source of truth —
  /// [SettingsStore] persists it.
  String? lastUrl;

  String get ffiStatus => 'FFI: kim-client via flutter_rust_bridge 2.13';

  Future<void> _ensure() async {
    if (_inited) {
      return;
    }
    // Android logic SO OTA: if KimApplication already System.load'd the OTA
    // libkim_client_ffi.so, open that absolute path so FRB does not pick the
    // APK/native-assets copy (stem: kim_client_ffi).
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
    _inited = true;
    if (!kIsWeb && Platform.isAndroid) {
      await OtaBridge.markHealthy();
    }
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

  rust.KimSdkHandle _require() {
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
    final account = JwtPeek.account(token) ?? '';
    _api ??= rust.KimSdkHandle.create();
    if (_account == account && account.isNotEmpty) {
      try {
        await _api!.notifyRadioUp();
      } catch (_) {}
      return;
    }
    await _api!.startSession(
      url: url,
      token: token,
      userAgent: userAgent,
      account: account,
    );
    _account = account;
    final fat = _api!
        .sessionEvents()
        .map(_event)
        .where((event) => event != null)
        .map((event) => event!);
    final watch = _api!.watchSession().map(_fromWatch);
    _events = _mergeEvents(fat, watch);
  }

  Stream<KimEvent> _mergeEvents(Stream<KimEvent> a, Stream<KimEvent> b) {
    late StreamController<KimEvent> controller;
    StreamSubscription<KimEvent>? sa;
    StreamSubscription<KimEvent>? sb;
    controller = StreamController<KimEvent>.broadcast(
      onListen: () {
        sa = a.listen(controller.add, onError: controller.addError);
        sb = b.listen(controller.add, onError: controller.addError);
      },
      onCancel: () {
        unawaited(sa?.cancel());
        unawaited(sb?.cancel());
      },
    );
    return controller.stream;
  }

  KimEvent _fromWatch(rust_types.SessionUpdateDto dto) {
    return switch (dto.kind) {
      'kickout' => KimEvent(kind: KimEventKind.kick, dest: dto.channelId),
      'auth_expired' => KimEvent(
        kind: KimEventKind.authExpired,
        error: dto.reason,
      ),
      'token' => KimEvent(
        kind: KimEventKind.token,
        token: dto.token,
        exp: dto.exp.toInt(),
      ),
      'friend' => KimEvent(
        kind: KimEventKind.friend,
        dest: dto.from,
        sender: dto.from,
        nickname: dto.nickname,
      ),
      'friend_accepted' => KimEvent(
        kind: KimEventKind.friendAccepted,
        dest: dto.from,
        sender: dto.from,
        nickname: dto.nickname,
      ),
      'link' => KimEvent(kind: KimEventKind.link, error: dto.lastError ?? ''),
      _ => const KimEvent(kind: KimEventKind.closed),
    };
  }

  @override
  Future<void> stopSession() async {
    final api = _api;
    _api = null;
    _events = null;
    _account = null;
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
  Future<void> notifyForeground() async {
    await _require().notifyForeground();
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
          lastBody: previewSnippet(item.lastBody),
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

  KimEvent? _event(rust.KimSessionEvent push) {
    final kind = switch (push.kind) {
      'talk' => KimEventKind.talk,
      'kick' => KimEventKind.kick,
      'friend' => KimEventKind.friend,
      'friend_accepted' => KimEventKind.friendAccepted,
      'profile_updated' => KimEventKind.profileUpdated,
      'presence' => KimEventKind.presence,
      'typing' => KimEventKind.typing,
      'receipt_read' => KimEventKind.receiptRead,
      'group' => KimEventKind.group,
      'token' => KimEventKind.token,
      'link' => KimEventKind.link,
      'inbox' => KimEventKind.inbox,
      'sync_progress' => KimEventKind.syncProgress,
      'sync_done' => KimEventKind.syncDone,
      'sync_failed' => KimEventKind.syncFailed,
      'sync_page' => KimEventKind.syncPage,
      'auth' => KimEventKind.authExpired,
      'closed' => KimEventKind.closed,
      _ => null,
    };
    if (kind == null) {
      return null;
    }
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
            lastBody: previewSnippet(item.lastBody),
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
      talks: kind == KimEventKind.syncPage
          ? _talksFromJson(push.body)
          : const [],
      pageId: kind == KimEventKind.syncPage ? push.messageId.toInt() : 0,
    );
  }

  List<KimEvent> _talksFromJson(String raw) {
    if (raw.isEmpty) {
      return const [];
    }
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! List) {
        return const [];
      }
      return [
        for (final row in decoded)
          if (row is Map)
            KimEvent(
              kind: KimEventKind.talk,
              dest: '${row['dest'] ?? ''}',
              sender: '${row['sender'] ?? ''}',
              body: '${row['body'] ?? ''}',
              extra: '${row['extra'] ?? ''}',
              messageId: (row['message_id'] as num?)?.toInt() ?? 0,
              sendTime: (row['send_time'] as num?)?.toInt() ?? 0,
              msgType: (row['msg_type'] as num?)?.toInt() ?? 0,
            ),
      ];
    } catch (_) {
      return const [];
    }
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
            kind: _profileKind(item['kind']),
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
      kind: _profileKind(decoded['kind']),
    );
  }

  int _profileKind(Object? raw) {
    if (raw == 'bot' || raw == '2') {
      return ProfileKind.bot;
    }
    final n = raw is int
        ? raw
        : raw is num
        ? raw.toInt()
        : ProfileKind.user;
    return n == ProfileKind.bot ? ProfileKind.bot : ProfileKind.user;
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

  @override
  Future<List<Map<String, dynamic>>> roomEnter(
    String dest, {
    int kind = 0,
  }) async {
    final raw = await _require().roomEnter(dest: dest, kind: kind);
    final decoded = jsonDecode(raw);
    if (decoded is! List) {
      return const [];
    }
    return [
      for (final row in decoded)
        if (row is Map)
          {
            'account': '${row['account'] ?? ''}',
            'status': row['status'] is int
                ? row['status'] as int
                : int.tryParse('${row['status']}') ?? 0,
            'lastSeen': row['last_seen'] is int
                ? row['last_seen'] as int
                : int.tryParse('${row['last_seen'] ?? row['lastSeen'] ?? 0}') ??
                      0,
          },
    ];
  }

  @override
  Future<void> sendTyping(
    String dest, {
    int kind = 0,
    bool active = true,
  }) async {
    await _require().sendTyping(dest: dest, kind: kind, active: active);
  }

  @override
  Future<void> roomLeave(String dest, {int kind = 0}) async {
    await _require().roomLeave(dest: dest, kind: kind);
  }

  @override
  Future<KimPerson> botCreate({
    required String clientProfileId,
    required String nickname,
    String avatar = '',
    String bio = '',
  }) async {
    return _person(
      await _require().botCreate(
        clientProfileId: clientProfileId,
        nickname: nickname,
        avatar: avatar,
        bio: bio,
      ),
    );
  }

  @override
  Future<void> botDelete(String dest) async {
    await _require().botDelete(dest: dest);
  }

  @override
  Future<KimPerson> botUpdate({
    required String dest,
    required String nickname,
    String avatar = '',
    String bio = '',
  }) async {
    return _person(
      await _require().botUpdate(
        dest: dest,
        nickname: nickname,
        avatar: avatar,
        bio: bio,
      ),
    );
  }

  @override
  Future<KimTalkResult> botReply({
    required String dest,
    required String body,
    required int inReplyTo,
    required String clientId,
  }) async {
    final result = await _require().botReply(
      dest: dest,
      body: body,
      inReplyTo: inReplyTo,
      clientId: clientId,
    );
    return KimTalkResult(
      messageId: result.messageId.toInt(),
      sendTime: sendTimeMs(result.sendTime.toInt()),
    );
  }

  @override
  Future<List<KimBotPendingItem>> botPending(
    String dest, {
    int limit = 20,
  }) async {
    final items = await _require().botPending(dest: dest, limit: limit);
    return [
      for (final item in items)
        KimBotPendingItem(
          messageId: item.messageId.toInt(),
          body: item.body,
          sendTime: item.sendTime.toInt(),
        ),
    ];
  }

  @override
  Future<void> attachStore(String dbPath) async {
    await _ensure();
    _api ??= rust.KimSdkHandle.create();
    await _api!.attachStore(dbPath: dbPath);
  }

  @override
  bool get rustStoreAttached => _api?.storeAttached() ?? false;

  @override
  Future<void> persistTalks(
    Iterable<KimChatMsg> msgs, {
    required UnreadPolicy policy,
  }) async {
    await _require().persistTalks(
      talks: [
        for (final m in msgs)
          rust.KimIncomingTalk(
            dest: m.dest,
            sender: m.sender,
            body: m.body,
            extra: '',
            messageId: m.messageId,
            sendTime: m.at,
            msgType: switch (m.kind) {
              KimMsgKind.image => 2,
              KimMsgKind.video => 4,
              _ => 1,
            },
          ),
      ],
      policy: policy == UnreadPolicy.ifInserted ? 'ifInserted' : 'keep',
    );
  }

  @override
  Future<void> persistInboxThreads(List<KimThread> threads) async {
    await _require().persistInbox(
      items: [
        for (final t in threads)
          rust.KimInboxItem(
            dest: t.id,
            kind: t.kind == ThreadKind.group ? 1 : 0,
            title: t.title,
            avatar: t.avatar,
            lastBody: t.lastBody,
            lastSender: '',
            lastMessageId: 0,
            lastSendTime: t.lastAt,
            unread: t.unread,
          ),
      ],
    );
  }
}
