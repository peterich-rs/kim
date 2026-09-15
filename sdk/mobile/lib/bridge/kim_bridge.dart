/// Dart shell around `kim-client` via flutter_rust_bridge 2.13.
/// Session / login / talk / Royal HTTP stay in Rust. Do not expand FFI here.
library;

import 'dart:async';
import 'dart:io' show Directory, File, Platform;
import 'dart:typed_data';

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;

import 'package:kim_mobile/core/format.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/core/media.dart';
import 'package:kim_mobile/core/ota_info.dart';
import 'package:kim_mobile/core/image_extra.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/auth.dart' as rust_auth;
import 'package:kim_mobile/src/rust/api/client.dart' as rust;
import 'package:kim_mobile/src/rust/api/simple.dart' as rust_simple;
import 'package:kim_mobile/src/rust/api/types.dart' as rust_types;
import 'package:kim_mobile/src/rust/frb_generated.dart';

class KimCommandReceipt {
  const KimCommandReceipt({
    required this.requestId,
    required this.clientId,
    required this.dest,
    required this.acceptedAt,
    required this.sendStatus,
  });

  final String requestId;
  final String clientId;
  final String dest;
  final int acceptedAt;
  final String sendStatus;
}

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
  Stream<rust_types.SessionSnapshotDto> watchSessionSnapshot();

  Stream<rust_types.SessionUpdateDto> watchSessionEvents();

  Stream<rust_types.TimelineUpdateDto> watchThread(
    String dest, {
    int limit = 50,
  });

  Future<void> startSession(
    String url,
    String token, {
    required String userAgent,
  });

  Future<void> stopSession();

  Future<void> notifyRadioUp();

  Future<void> notifyForeground();

  Future<KimCommandReceipt> enqueueMessage({
    required String dest,
    required ThreadKind kind,
    required KimOutgoingContent content,
    required String clientId,
    String localPath = '',
    String mime = '',
    int width = 0,
    int height = 0,
    int byteSize = 0,
  });

  Future<void> cancelSend(String clientId);

  Future<KimCommandReceipt> retrySend(String clientId);

  Future<void> deleteThread(String dest);

  Future<void> loadOlder({required String dest});

  Future<void> markRead(String dest, ThreadKind kind, int messageId);

  Future<List<KimPerson>> friendList();

  Future<List<KimPerson>> friendIncoming();

  Future<List<KimPerson>> searchUsers(String query);

  Future<void> friendRequest(String dest);

  Future<void> friendAccept(String dest);

  Future<void> friendReject(String dest);

  Future<void> friendRemove(String dest);

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
    String model = '',
    String thinkingEffort = '',
    int? contextTokens,
    String visibility = '',
  });

  Future<void> botDelete(String dest);

  Future<KimPerson> botUpdate({
    required String dest,
    required String nickname,
    String avatar = '',
    String bio = '',
    String model = '',
    String thinkingEffort = '',
    int? contextTokens,
    String visibility = '',
  });

  Future<KimTalkResult> botReply({
    required String dest,
    required String body,
    required int inReplyTo,
    required String clientId,
  });

  Future<List<KimBotPendingItem>> botPending(String dest, {int limit = 20});

  /// Owner-sent bot typing for a registered 1:1 (S-KD 26).
  Future<void> botTyping(String dest, {int kind = 0, bool active = true});

  Stream<rust_types.TokenPersistDto> watchTokenPersist();

  Future<rust_types.SettingsDto> settingsGet();

  Future<rust_types.SettingsDto> settingsPatch({
    String? wsUrl,
    String? httpOrigin,
    String? env,
  });

  Future<rust_types.SettingsDto> importDeviceSettings({
    required String wsUrl,
    required String httpOrigin,
    String env = 'prod',
    String locale = '',
  });

  Future<void> refreshContacts();

  Stream<rust_types.ContactsSnapshotDto> watchContacts();

  Stream<rust_types.AgentRunRequestDto> watchAgentRun();

  Future<void> submitAgentRun(rust_types.AgentRunResultDto result);

  Future<List<rust_types.AgentProfileDto>> listAgentProfiles();

  Future<void> upsertAgentProfile(rust_types.AgentProfileDto row);

  Future<void> deleteAgentProfile(String profileId);

  Future<void> importAgentProfiles(List<rust_types.AgentProfileDto> rows);

  Future<List<rust_types.ProviderAccountDto>> listProviderAccounts();

  Future<void> upsertProviderAccount(rust_types.ProviderAccountDto row);

  Future<void> deleteProviderAccount(String id);

  Future<rust_types.DeviceOverlayDto?> getDeviceOverlay(String profileId);

  Future<void> upsertDeviceOverlay(rust_types.DeviceOverlayDto row);

  Future<String> agentFlags();

  Future<void> setAgentFlags(String flagsJson);

  Future<void> syncAgentSpecs();

  Future<Uint8List> specJsonToBlob(String bodyJson);

  Future<String> specBlobToJson(List<int> blob);

  Future<rust_types.CommandAckDto> command(rust_types.UiCommandDto cmd);

  Future<List<rust_types.MessageViewDto>> searchMessages(
    String query, {
    String? dest,
  });

  Future<rust_types.LocalMediaDto> mediaFetch(String url);

  Future<rust_types.LocalMediaDto> mediaUpload({
    required String path,
    required String mime,
    int width = 0,
    int height = 0,
    int byteSize = 0,
  });

  Future<rust_types.MetricsDto> metricsSnapshot();
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

class KimBridge implements KimAuthPort, KimClientPort, KimMediaPort {
  static const flutterPin = '3.47.2';
  static const ffiReady = true;

  static bool _inited = false;
  rust.KimUiHandle? _api;
  String? _account;

  /// Last WGateway URL passed to [startSession]. Device URL lives in Rust.
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

  rust.KimUiHandle _require() {
    final api = _api;
    if (api == null) {
      throw StateError('attachStore first');
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
    final api = _api ?? rust.KimUiHandle.create();
    _api = api;
    if (_account != null && _account!.isNotEmpty) {
      try {
        await api.notifyRadioUp();
      } catch (e, st) {
        KimLogger.warn('notifyRadioUp', e, st);
      }
      return;
    }
    await api.startSession(
      url: url,
      token: token,
      userAgent: userAgent,
      account: '',
    );
    _account = 'session';
  }

  @override
  Stream<rust_types.SessionSnapshotDto> watchSessionSnapshot() {
    return _require().watchSessionSnapshot();
  }

  @override
  Stream<rust_types.SessionUpdateDto> watchSessionEvents() {
    return _require().watchSession();
  }

  @override
  Stream<rust_types.TimelineUpdateDto> watchThread(
    String dest, {
    int limit = 50,
  }) {
    return _require().watchTimeline(dest: dest, limit: limit);
  }

  @override
  Future<void> loadOlder({required String dest}) {
    return _require().loadOlder(dest: dest);
  }

  @override
  Future<void> stopSession() async {
    _account = null;
    final api = _api;
    if (api == null) {
      return;
    }
    try {
      await api.stop();
    } catch (e, st) {
      KimLogger.warn('stopSession', e, st);
    }
  }

  @override
  Future<void> notifyRadioUp() async {
    await _require().notifyRadioUp();
  }

  @override
  Future<void> notifyForeground() async {
    await _require().notifyForeground();
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
  Future<void> markRead(String dest, ThreadKind kind, int messageId) async {
    await _require().markThreadRead(
      dest: dest,
      kind: kind == ThreadKind.group ? 1 : 0,
      messageId: messageId,
    );
  }

  @override
  Future<KimCommandReceipt> enqueueMessage({
    required String dest,
    required ThreadKind kind,
    required KimOutgoingContent content,
    required String clientId,
    String localPath = '',
    String mime = '',
    int width = 0,
    int height = 0,
    int byteSize = 0,
  }) async {
    final receipt = await _require().enqueueMessage(
      dest: dest,
      kind: kind == ThreadKind.group ? 1 : 0,
      content: _wire(content),
      clientId: clientId,
      localPath: localPath,
      mime: mime,
      width: width,
      height: height,
      byteSize: byteSize,
    );
    return KimCommandReceipt(
      requestId: receipt.requestId,
      clientId: receipt.clientId,
      dest: receipt.dest,
      acceptedAt: receipt.acceptedAt.toInt(),
      sendStatus: _sendStatusLabel(receipt.sendStatus),
    );
  }

  @override
  Future<void> cancelSend(String clientId) async {
    await _require().cancelSend(clientId: clientId);
  }

  @override
  Future<KimCommandReceipt> retrySend(String clientId) async {
    final receipt = await _require().retrySend(clientId: clientId);
    return KimCommandReceipt(
      requestId: receipt.requestId,
      clientId: receipt.clientId,
      dest: receipt.dest,
      acceptedAt: receipt.acceptedAt.toInt(),
      sendStatus: _sendStatusLabel(receipt.sendStatus),
    );
  }

  @override
  Future<void> deleteThread(String dest) async {
    await _require().deleteThread(dest: dest);
  }

  KimPerson _fromPerson(rust_types.PersonDto p) {
    return KimPerson(
      account: p.account,
      nickname: p.nickname,
      avatar: p.avatar,
      bio: p.bio,
      kind: _profileKind(p.kind),
    );
  }

  KimPerson _fromProfile(rust_types.ProfileDto p) {
    return KimPerson(
      account: p.account,
      nickname: p.nickname,
      avatar: p.avatar,
      bio: p.bio,
      kind: _profileKind(p.kind),
    );
  }

  String _sendStatusLabel(rust_types.SendStatusDto status) {
    return switch (status) {
      rust_types.SendStatusDto.pending => 'pending',
      rust_types.SendStatusDto.uploading => 'uploading',
      rust_types.SendStatusDto.sending => 'sending',
      rust_types.SendStatusDto.sent => 'sent',
      rust_types.SendStatusDto.failed => 'failed',
      rust_types.SendStatusDto.cancelled => 'cancelled',
    };
  }

  @override
  Future<List<KimPerson>> friendList() async {
    return [for (final p in await _require().friendList()) _fromPerson(p)];
  }

  @override
  Future<List<KimPerson>> friendIncoming() async {
    return [for (final p in await _require().friendIncoming()) _fromPerson(p)];
  }

  @override
  Future<List<KimPerson>> searchUsers(String query) async {
    return [
      for (final p in await _require().searchUsers(query: query))
        _fromPerson(p),
    ];
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

  @override
  Future<void> friendRemove(String dest) async {
    await _require().friendRemove(dest: dest);
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
    return _fromProfile(await _require().profile(dest: dest));
  }

  @override
  Future<KimPerson> updateProfile({
    required String nickname,
    required String avatar,
    String bio = '',
  }) async {
    return _fromProfile(
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
    final rows = await _require().roomEnter(dest: dest, kind: kind);
    return [
      for (final row in rows)
        {
          'account': row.account,
          'status': row.status,
          'lastSeen': row.lastSeen.toInt(),
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
    String model = '',
    String thinkingEffort = '',
    int? contextTokens,
    String visibility = '',
  }) async {
    return _fromPerson(
      await _require().botCreate(
        clientProfileId: clientProfileId,
        nickname: nickname,
        avatar: avatar,
        bio: bio,
        model: model,
        thinkingEffort: thinkingEffort,
        contextTokens: contextTokens ?? 0,
        visibility: visibility,
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
    String model = '',
    String thinkingEffort = '',
    int? contextTokens,
    String visibility = '',
  }) async {
    return _fromPerson(
      await _require().botUpdate(
        dest: dest,
        nickname: nickname,
        avatar: avatar,
        bio: bio,
        model: model,
        thinkingEffort: thinkingEffort,
        contextTokens: contextTokens ?? 0,
        visibility: visibility,
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
  Future<void> botTyping(
    String dest, {
    int kind = 0,
    bool active = true,
  }) async {
    await _require().botTyping(dest: dest, kind: kind, active: active);
  }

  @override
  Stream<rust_types.TokenPersistDto> watchTokenPersist() {
    return _require().watchTokenPersist();
  }

  @override
  Future<rust_types.SettingsDto> settingsGet() {
    return _require().settingsGet();
  }

  @override
  Future<rust_types.SettingsDto> settingsPatch({
    String? wsUrl,
    String? httpOrigin,
    String? env,
  }) {
    return _require().settingsPatch(
      wsUrl: wsUrl,
      httpOrigin: httpOrigin,
      env: env,
    );
  }

  @override
  Future<rust_types.SettingsDto> importDeviceSettings({
    required String wsUrl,
    required String httpOrigin,
    String env = 'prod',
    String locale = '',
  }) {
    return _require().importDeviceSettings(
      wsUrl: wsUrl,
      httpOrigin: httpOrigin,
      env: env,
      locale: locale,
    );
  }

  @override
  Future<void> refreshContacts() {
    return _require().refreshContacts();
  }

  @override
  Stream<rust_types.ContactsSnapshotDto> watchContacts() {
    return _require().watchContacts();
  }

  @override
  Stream<rust_types.AgentRunRequestDto> watchAgentRun() {
    return _require().watchAgentRun();
  }

  @override
  Future<void> submitAgentRun(rust_types.AgentRunResultDto result) {
    return _require().submitAgentRun(result: result);
  }

  @override
  Future<List<rust_types.AgentProfileDto>> listAgentProfiles() {
    return _require().listAgentProfiles();
  }

  @override
  Future<void> upsertAgentProfile(rust_types.AgentProfileDto row) {
    return _require().upsertAgentProfile(row: row);
  }

  @override
  Future<void> deleteAgentProfile(String profileId) {
    return _require().deleteAgentProfile(profileId: profileId);
  }

  @override
  Future<void> importAgentProfiles(List<rust_types.AgentProfileDto> rows) {
    return _require().importAgentProfiles(rows: rows);
  }

  @override
  Future<List<rust_types.ProviderAccountDto>> listProviderAccounts() {
    return _require().listProviderAccounts();
  }

  @override
  Future<void> upsertProviderAccount(rust_types.ProviderAccountDto row) {
    return _require().upsertProviderAccount(row: row);
  }

  @override
  Future<void> deleteProviderAccount(String id) {
    return _require().deleteProviderAccount(id: id);
  }

  @override
  Future<rust_types.DeviceOverlayDto?> getDeviceOverlay(String profileId) {
    return _require().getDeviceOverlay(profileId: profileId);
  }

  @override
  Future<void> upsertDeviceOverlay(rust_types.DeviceOverlayDto row) {
    return _require().upsertDeviceOverlay(row: row);
  }

  @override
  Future<String> agentFlags() {
    return _require().agentFlags();
  }

  @override
  Future<void> setAgentFlags(String flagsJson) {
    return _require().setAgentFlags(flagsJson: flagsJson);
  }

  @override
  Future<void> syncAgentSpecs() {
    return _require().syncAgentSpecs();
  }

  @override
  Future<Uint8List> specJsonToBlob(String bodyJson) {
    return rust_simple.specJsonToBlob(bodyJson: bodyJson);
  }

  @override
  Future<String> specBlobToJson(List<int> blob) {
    return rust_simple.specBlobToJson(blob: blob);
  }

  @override
  Future<rust_types.CommandAckDto> command(rust_types.UiCommandDto cmd) {
    return _require().command(cmd: cmd);
  }

  @override
  Future<List<rust_types.MessageViewDto>> searchMessages(
    String query, {
    String? dest,
  }) {
    return _require().searchMessages(query: query, dest: dest);
  }

  @override
  Future<rust_types.LocalMediaDto> mediaFetch(String url) {
    return _require().mediaFetch(url: url);
  }

  @override
  Future<rust_types.LocalMediaDto> mediaUpload({
    required String path,
    required String mime,
    int width = 0,
    int height = 0,
    int byteSize = 0,
  }) {
    return _require().mediaUpload(
      path: path,
      mime: mime,
      width: width,
      height: height,
      byteSize: byteSize,
    );
  }

  @override
  @override
  Future<rust_types.MetricsDto> metricsSnapshot() async {
    return _require().metricsSnapshot();
  }

  @override
  Future<UploadedObject> uploadImage({
    required String token,
    required List<int> bytes,
    required String contentType,
  }) async {
    final _ = token;
    final file = File(
      '${Directory.systemTemp.path}/kim-up-${DateTime.now().microsecondsSinceEpoch}',
    );
    await file.writeAsBytes(bytes, flush: true);
    try {
      final dto = await mediaUpload(
        path: file.path,
        mime: contentType,
        byteSize: bytes.length,
      );
      return UploadedObject(
        key: '',
        url: dto.localPath,
        contentType: contentType,
        bytes: bytes.length,
      );
    } finally {
      if (file.existsSync()) {
        await file.delete();
      }
    }
  }

  Future<void> attachStore(String dbPath) async {
    await _ensure();
    _api ??= rust.KimUiHandle.create();
    await _api!.attachStore(dbPath: dbPath);
  }
}
