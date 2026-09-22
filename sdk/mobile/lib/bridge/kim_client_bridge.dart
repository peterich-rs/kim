/// Client port adapter over [KimBridgeBase].
library;

import 'dart:typed_data';

import 'package:kim_mobile/bridge/kim_bridge_base.dart';
import 'package:kim_mobile/bridge/kim_ports.dart';
import 'package:kim_mobile/core/format.dart';
import 'package:kim_mobile/core/image_extra.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/client.dart' as rust;
import 'package:kim_mobile/src/rust/api/handles.dart' as rust_handles;
import 'package:kim_mobile/src/rust/api/simple.dart' as rust_simple;
import 'package:kim_mobile/src/rust/api/types.dart' as rust_types;

mixin KimClientBridge on KimBridgeBase implements KimClientPort {
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
    await ensure();
    lastUrl = url;
    final handle = api ?? rust.KimUiHandle.create();
    api = handle;
    if (account != null && account!.isNotEmpty) {
      try {
        await handle.notifyRadioUp();
      } catch (e, st) {
        KimLogger.warn('notifyRadioUp', e, st);
      }
      return;
    }
    await handle.startSession(
      url: url,
      token: token,
      userAgent: userAgent,
      account: '',
    );
    account = 'session';
  }

  @override
  Stream<rust_types.SessionSnapshot> watchSessionSnapshot() {
    return requireApi().watchSessionSnapshot();
  }

  @override
  Stream<rust_types.SessionUpdate> watchSessionEvents() {
    return requireApi().watchSession();
  }

  @override
  Stream<rust_types.TimelineUpdate> watchThread(
    String dest, {
    int limit = 50,
  }) {
    return requireApi().watchTimeline(dest: dest, limit: limit);
  }

  @override
  Future<void> loadOlder({required String dest}) {
    return requireApi().loadOlder(dest: dest);
  }

  @override
  Future<void> stopSession() async {
    account = null;
    final handle = api;
    if (handle == null) {
      return;
    }
    try {
      await handle.stop();
    } catch (e, st) {
      KimLogger.warn('stopSession', e, st);
    }
  }

  @override
  Future<void> notifyRadioUp() async {
    await requireApi().notifyRadioUp();
  }

  @override
  Future<void> notifyForeground() async {
    await requireApi().notifyForeground();
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
    if (messageId <= 0) {
      await markConversationRead(dest, kind);
      return;
    }
    await requireApi().markThreadRead(
      dest: dest,
      kind: kind == ThreadKind.group ? 1 : 0,
      messageId: messageId,
    );
  }

  @override
  Future<void> markConversationRead(String dest, ThreadKind kind) async {
    await requireApi().markConversationRead(
      dest: dest,
      kind: kind == ThreadKind.group ? 1 : 0,
    );
  }

  @override
  Future<void> setConversationVisibility({
    required int generation,
    required bool foreground,
    required String dest,
    required ThreadKind kind,
  }) async {
    await requireApi().setConversationVisibility(
      generation: BigInt.from(generation),
      foreground: foreground,
      dest: dest,
      kind: kind == ThreadKind.group ? 1 : 0,
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
    final receipt = await requireApi().enqueueMessage(
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
    await requireApi().cancelSend(clientId: clientId);
  }

  @override
  Future<KimCommandReceipt> retrySend(String clientId) async {
    final receipt = await requireApi().retrySend(clientId: clientId);
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
    await requireApi().deleteThread(dest: dest);
  }

  KimPerson _fromPerson(rust_types.Person p) {
    return KimPerson(
      account: p.account,
      nickname: p.nickname,
      avatar: p.avatar,
      bio: p.bio,
      kind: _profileKind(p.kind),
    );
  }

  KimPerson _fromProfile(rust_types.Profile p) {
    return KimPerson(
      account: p.account,
      nickname: p.nickname,
      avatar: p.avatar,
      bio: p.bio,
      kind: _profileKind(p.kind),
    );
  }

  String _sendStatusLabel(rust_types.SendStatus status) {
    return switch (status) {
      rust_types.SendStatus.pending => 'pending',
      rust_types.SendStatus.uploading => 'uploading',
      rust_types.SendStatus.sending => 'sending',
      rust_types.SendStatus.sent => 'sent',
      rust_types.SendStatus.failed => 'failed',
      rust_types.SendStatus.cancelled => 'cancelled',
    };
  }

  @override
  Future<List<KimPerson>> friendList() async {
    return [for (final p in await requireApi().friendList()) _fromPerson(p)];
  }

  @override
  Future<List<KimPerson>> friendIncoming() async {
    return [for (final p in await requireApi().friendIncoming()) _fromPerson(p)];
  }

  @override
  Future<List<KimPerson>> searchUsers(String query) async {
    return [
      for (final p in await requireApi().searchUsers(query: query))
        _fromPerson(p),
    ];
  }

  @override
  Future<void> friendRequest(String dest) async {
    await requireApi().friendRequest(dest: dest);
  }

  @override
  Future<void> friendAccept(String dest) async {
    await requireApi().friendAccept(dest: dest);
  }

  @override
  Future<void> friendReject(String dest) async {
    await requireApi().friendReject(dest: dest);
  }

  @override
  Future<void> friendRemove(String dest) async {
    await requireApi().friendRemove(dest: dest);
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
    return _fromProfile(await requireApi().profile(dest: dest));
  }

  @override
  Future<KimPerson> updateProfile({
    required String nickname,
    required String avatar,
    String bio = '',
  }) async {
    return _fromProfile(
      await requireApi().updateProfile(
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
    final rows = await requireApi().roomEnter(dest: dest, kind: kind);
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
    await requireApi().sendTyping(dest: dest, kind: kind, active: active);
  }

  @override
  Future<void> roomLeave(String dest, {int kind = 0}) async {
    await requireApi().roomLeave(dest: dest, kind: kind);
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
      await requireApi().botCreate(
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
    await requireApi().botDelete(dest: dest);
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
      await requireApi().botUpdate(
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
    final result = await requireApi().botReply(
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
    final items = await requireApi().botPending(dest: dest, limit: limit);
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
    await requireApi().botTyping(dest: dest, kind: kind, active: active);
  }

  @override
  Stream<rust_types.TokenPersist> watchTokenPersist() {
    return requireApi().watchTokenPersist();
  }

  @override
  Future<rust_types.Settings> settingsGet() {
    return requireApi().settingsGet();
  }

  @override
  Future<rust_types.Settings> settingsPatch({
    String? wsUrl,
    String? httpOrigin,
    String? env,
  }) {
    return requireApi().settingsPatch(
      wsUrl: wsUrl,
      httpOrigin: httpOrigin,
      env: env,
    );
  }

  @override
  Future<rust_types.Settings> importDeviceSettings({
    required String wsUrl,
    required String httpOrigin,
    String env = 'prod',
    String locale = '',
  }) {
    return requireApi().importDeviceSettings(
      wsUrl: wsUrl,
      httpOrigin: httpOrigin,
      env: env,
      locale: locale,
    );
  }

  @override
  Future<void> refreshContacts() {
    return requireApi().refreshContacts();
  }

  @override
  Stream<rust_types.ContactsSnapshot> watchContacts() {
    return requireApi().watchContacts();
  }

  @override
  Stream<rust_types.AgentRunRequest> watchAgentRun() {
    return requireApi().watchAgentRun();
  }

  @override
  Future<void> cacheAgentSecret({
    required String keyRef,
    required String secret,
  }) {
    return requireApi().cacheAgentSecret(keyRef: keyRef, secret: secret);
  }

  @override
  Future<void> respondAgentPermission({
    required String callId,
    required bool allow,
  }) {
    return requireApi().respondAgentPermission(callId: callId, allow: allow);
  }

  @override
  Stream<rust_handles.AgentPermissionEvent> watchAgentPermission() {
    return requireApi().watchAgentPermission();
  }

  @override
  Stream<rust_handles.AgentUiStatus> watchAgentUi() {
    return requireApi().watchAgentUi();
  }

  @override
  Future<void> submitAgentRun(rust_types.AgentRunResult result) {
    return requireApi().submitAgentRun(result: result);
  }

  @override
  Future<List<rust_types.AgentProfile>> listAgentProfiles() {
    return requireApi().listAgentProfiles();
  }

  @override
  Future<void> upsertAgentProfile(rust_types.AgentProfile row) {
    return requireApi().upsertAgentProfile(row: row);
  }

  @override
  Future<void> deleteAgentProfile(String profileId) {
    return requireApi().deleteAgentProfile(profileId: profileId);
  }

  @override
  Future<void> importAgentProfiles(List<rust_types.AgentProfile> rows) {
    return requireApi().importAgentProfiles(rows: rows);
  }

  @override
  Future<List<rust_types.ProviderAccount>> listProviderAccounts() {
    return requireApi().listProviderAccounts();
  }

  @override
  Future<void> upsertProviderAccount(rust_types.ProviderAccount row) {
    return requireApi().upsertProviderAccount(row: row);
  }

  @override
  Future<void> deleteProviderAccount(String id) {
    return requireApi().deleteProviderAccount(id: id);
  }

  @override
  Future<rust_types.DeviceOverlay?> getDeviceOverlay(String profileId) {
    return requireApi().getDeviceOverlay(profileId: profileId);
  }

  @override
  Future<void> upsertDeviceOverlay(rust_types.DeviceOverlay row) {
    return requireApi().upsertDeviceOverlay(row: row);
  }

  @override
  Future<rust_types.AgentFlags> agentFlags() {
    return requireApi().agentFlags();
  }

  @override
  Future<void> setAgentFlags(rust_types.AgentFlags flags) {
    return requireApi().setAgentFlags(flags: flags);
  }

  @override
  Future<void> syncAgentSpecs() {
    return requireApi().syncAgentSpecs();
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
  Future<rust_types.CommandAck> command(rust_types.UiCommand cmd) {
    return requireApi().command(cmd: cmd);
  }

  @override
  Future<List<rust_types.MessageView>> searchMessages(
    String query, {
    String? dest,
  }) {
    return requireApi().searchMessages(query: query, dest: dest);
  }

  @override
  Future<rust_types.LocalMedia> mediaFetch(String url) {
    return requireApi().mediaFetch(url: url);
  }

  @override
  Future<rust_types.LocalMedia> mediaUpload({
    required String path,
    required String mime,
    int width = 0,
    int height = 0,
    int byteSize = 0,
  }) {
    return requireApi().mediaUpload(
      path: path,
      mime: mime,
      width: width,
      height: height,
      byteSize: byteSize,
    );
  }

  @override
  Future<rust_types.Metrics> metricsSnapshot() async {
    return requireApi().metricsSnapshot();
  }

}
