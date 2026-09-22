/// Port contracts + DTOs for kim-client FFI.
/// Production adapter: [KimBridge]; tests: FakeKim.
library;

import 'dart:typed_data';

import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/handles.dart' as rust_handles;
import 'package:kim_mobile/src/rust/api/types.dart' as rust_types;

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
  Stream<rust_types.SessionSnapshot> watchSessionSnapshot();

  Stream<rust_types.SessionUpdate> watchSessionEvents();

  Stream<rust_types.TimelineUpdate> watchThread(
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

  Future<void> markConversationRead(String dest, ThreadKind kind);

  Future<void> setConversationVisibility({
    required int generation,
    required bool foreground,
    required String dest,
    required ThreadKind kind,
  });

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

  Stream<rust_types.TokenPersist> watchTokenPersist();

  Future<rust_types.Settings> settingsGet();

  Future<rust_types.Settings> settingsPatch({
    String? wsUrl,
    String? httpOrigin,
    String? env,
  });

  Future<rust_types.Settings> importDeviceSettings({
    required String wsUrl,
    required String httpOrigin,
    String env = 'prod',
    String locale = '',
  });

  Future<void> refreshContacts();

  Stream<rust_types.ContactsSnapshot> watchContacts();

  Stream<rust_types.AgentRunRequest> watchAgentRun();

  Future<void> submitAgentRun(rust_types.AgentRunResult result);

  Future<List<rust_types.AgentProfile>> listAgentProfiles();

  Future<void> upsertAgentProfile(rust_types.AgentProfile row);

  Future<void> deleteAgentProfile(String profileId);

  Future<void> importAgentProfiles(List<rust_types.AgentProfile> rows);

  Future<List<rust_types.ProviderAccount>> listProviderAccounts();

  Future<void> upsertProviderAccount(rust_types.ProviderAccount row);

  Future<void> deleteProviderAccount(String id);

  Future<rust_types.DeviceOverlay?> getDeviceOverlay(String profileId);

  Future<void> upsertDeviceOverlay(rust_types.DeviceOverlay row);

  Future<rust_types.AgentFlags> agentFlags();

  Future<void> setAgentFlags(rust_types.AgentFlags flags);

  Future<void> syncAgentSpecs();

  Future<Uint8List> specJsonToBlob(String bodyJson);

  Future<String> specBlobToJson(List<int> blob);

  Future<rust_types.CommandAck> command(rust_types.UiCommand cmd);

  Future<List<rust_types.MessageView>> searchMessages(
    String query, {
    String? dest,
  });

  Future<rust_types.LocalMedia> mediaFetch(String url);

  Future<rust_types.LocalMedia> mediaUpload({
    required String path,
    required String mime,
    int width = 0,
    int height = 0,
    int byteSize = 0,
  });

  Future<rust_types.Metrics> metricsSnapshot();

  /// One-shot handoff into the desktop secret vault.
  Future<void> cacheAgentSecret({
    required String keyRef,
    required String secret,
  });

  Future<void> respondAgentPermission({
    required String callId,
    required bool allow,
  });

  Stream<rust_handles.AgentPermissionEvent> watchAgentPermission();

  Stream<rust_handles.AgentUiStatus> watchAgentUi();
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
