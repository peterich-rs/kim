/// Client port adapter over [KimBridgeBase].
library;

import 'dart:typed_data';

import 'package:kim_mobile/bridge/kim_bridge_base.dart';
import 'package:kim_mobile/bridge/kim_ports.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/client.dart' as rust;
import 'package:kim_mobile/src/rust/api/handles.dart' as rust_handles;
import 'package:kim_mobile/src/rust/api/types.dart' as rust_types;

mixin KimClientBridge on KimBridgeBase implements KimClientPort {
  @override
  Future<void> startSession() async {
    await ensure();
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
    await handle.startSession();
    account = 'session';
  }

  @override
  Future<bool> hasStoredToken() {
    return requireApi().hasStoredToken();
  }

  @override
  Future<String> storedAccount() {
    return requireApi().storedAccount();
  }

  @override
  Future<void> storeAuth({required String token, required String account}) {
    return requireApi().storeAuth(token: token, account: account);
  }

  @override
  Future<void> clearAuth() {
    return requireApi().clearAuth();
  }

  @override
  Future<void> authLogout() {
    return requireApi().authLogout();
  }

  @override
  Future<void> authChangePassword({
    required String oldPassword,
    required String newPassword,
  }) {
    return requireApi().authChangePassword(
      oldPassword: oldPassword,
      newPassword: newPassword,
    );
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
  Stream<rust_types.TimelineUpdate> watchThread(String dest, {int limit = 50}) {
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

  rust_types.ThreadKind _kind(ThreadKind kind) {
    return switch (kind) {
      ThreadKind.group => rust_types.ThreadKind.group,
      ThreadKind.user => rust_types.ThreadKind.user,
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
      kind: _kind(kind),
      messageId: messageId,
    );
  }

  @override
  Future<void> markConversationRead(String dest, ThreadKind kind) async {
    await requireApi().markConversationRead(dest: dest, kind: _kind(kind));
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
      kind: _kind(kind),
    );
  }

  @override
  Future<KimCommandReceipt> enqueueMessage({
    required String dest,
    required ThreadKind kind,
    required rust_types.OutgoingContent content,
    required String clientId,
  }) async {
    final receipt = await requireApi().enqueueMessage(
      dest: dest,
      kind: _kind(kind),
      content: content,
      clientId: clientId,
    );
    return KimCommandReceipt(
      requestId: receipt.requestId,
      clientId: receipt.clientId,
      dest: receipt.dest,
      acceptedAt: receipt.acceptedAt.toInt(),
      sendStatus: receipt.sendStatus,
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
      sendStatus: receipt.sendStatus,
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

  int _profileKind(rust_types.ProfileKind kind) {
    return switch (kind) {
      rust_types.ProfileKind.bot => ProfileKind.bot,
      rust_types.ProfileKind.user => ProfileKind.user,
    };
  }

  @override
  Future<List<KimPerson>> friendList() async {
    return [for (final p in await requireApi().friendList()) _fromPerson(p)];
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
  Future<List<rust_types.RoomMember>> roomEnter(
    String dest,
    ThreadKind kind,
  ) async {
    return requireApi().roomEnter(dest: dest, kind: _kind(kind));
  }

  @override
  Future<void> sendTyping(
    String dest,
    ThreadKind kind, {
    bool active = true,
  }) async {
    await requireApi().sendTyping(
      dest: dest,
      kind: _kind(kind),
      active: active,
    );
  }

  @override
  Future<void> roomLeave(String dest, ThreadKind kind) async {
    await requireApi().roomLeave(dest: dest, kind: _kind(kind));
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
  Future<rust_types.Settings> settingsGet() {
    return requireApi().settingsGet();
  }

  @override
  Future<rust_types.Settings> settingsPatch({String? wsUrl, String? env}) {
    return requireApi().settingsPatch(wsUrl: wsUrl, env: env);
  }

  @override
  Future<rust_types.Settings> settingsPreset(rust_types.SettingsPreset preset) {
    return requireApi().settingsPreset(preset: preset);
  }

  @override
  Future<bool> settingsImported() {
    return requireApi().settingsImported();
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
  Future<void> storeAgentSecret({
    required String keyRef,
    required String secret,
  }) {
    return requireApi().storeAgentSecret(keyRef: keyRef, secret: secret);
  }

  @override
  Future<List<String>> fetchModels({
    required String vendorId,
    required String baseUrl,
    required String keyRef,
  }) {
    return requireApi().fetchModels(
      vendorId: vendorId,
      baseUrl: baseUrl,
      keyRef: keyRef,
    );
  }

  @override
  Future<rust_handles.CapabilityPreview> previewProfile(String profileId) {
    return requireApi().previewProfile(profileId: profileId);
  }

  @override
  Future<void> workspaceGrantRegister({
    required String profileId,
    required String path,
  }) {
    return requireApi().workspaceGrantRegister(
      profileId: profileId,
      path: path,
    );
  }

  @override
  Future<String?> workspaceGrant(String profileId) {
    return requireApi().workspaceGrant(profileId: profileId);
  }

  @override
  Future<String> ensureAgentSandbox(String profileId) {
    return requireApi().ensureAgentSandbox(profileId: profileId);
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
  Future<List<rust_types.AgentProfile>> listAgentProfiles() {
    return requireApi().listAgentProfiles();
  }

  @override
  Future<void> upsertAgentProfile(String documentJson) {
    return requireApi().upsertAgentProfile(documentJson: documentJson);
  }

  @override
  Future<void> deleteAgentProfile(String profileId) {
    return requireApi().deleteAgentProfile(profileId: profileId);
  }

  @override
  Future<void> importAgentProfiles(List<String> documents) {
    return requireApi().importAgentProfiles(documents: documents);
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
  Future<rust_types.LocalMedia> mediaUploadBytes({
    required Uint8List bytes,
    required String mime,
    int width = 0,
    int height = 0,
  }) {
    return requireApi().mediaUploadBytes(
      bytes: bytes,
      mime: mime,
      width: width,
      height: height,
    );
  }

  @override
  Future<rust_types.Metrics> metricsSnapshot() async {
    return requireApi().metricsSnapshot();
  }
}
