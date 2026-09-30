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
  final rust_types.SendStatus sendStatus;
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

  Stream<rust_types.TimelineUpdate> watchThread(String dest, {int limit = 50});

  /// Intent only: Rust reads URL (settings table), token (secure store),
  /// account (JWT), UA (bootstrap) on its own.
  Future<void> startSession();

  Future<void> stopSession();

  Future<bool> hasStoredToken();

  Future<String> storedAccount();

  Future<void> storeAuth({required String token, required String account});

  Future<void> clearAuth();

  /// Server logout with the stored credential; token stays in Rust.
  Future<void> authLogout();

  Future<void> authChangePassword({
    required String oldPassword,
    required String newPassword,
  });

  Future<void> notifyRadioUp();

  Future<void> notifyForeground();

  Future<KimCommandReceipt> enqueueMessage({
    required String dest,
    required ThreadKind kind,
    required rust_types.OutgoingContent content,
    String? clientId,
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

  Future<List<rust_types.RoomMember>> roomEnter(String dest, ThreadKind kind);

  Future<void> roomLeave(String dest, ThreadKind kind);

  /// Fire-and-forget typing indicator.
  Future<void> sendTyping(String dest, ThreadKind kind, {bool active = true});

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

  Future<rust_types.Settings> settingsGet();

  Future<rust_types.Settings> settingsPatch({String? wsUrl, String? env});

  Future<rust_types.Settings> settingsPreset(rust_types.SettingsPreset preset);

  Future<bool> settingsImported();

  Future<void> refreshContacts();

  Stream<rust_types.ContactsSnapshot> watchContacts();

  Future<List<rust_types.AgentProfile>> listAgentProfiles();

  Future<void> upsertAgentProfile(String documentJson);

  Future<void> deleteAgentProfile(String profileId);

  Future<List<rust_types.ProviderAccount>> listProviderAccounts();

  Future<void> upsertProviderAccount(rust_types.ProviderAccount row);

  Future<void> deleteProviderAccount(String id);

  Future<rust_types.DeviceOverlay?> getDeviceOverlay(String profileId);

  Future<void> upsertDeviceOverlay(rust_types.DeviceOverlay row);

  Future<rust_types.AgentFlags> agentFlags();

  Future<void> setAgentFlags(rust_types.AgentFlags flags);

  Future<void> syncAgentSpecs();

  Future<List<rust_types.MessageView>> searchMessages(
    String query, {
    String? dest,
  });

  Future<rust_types.LocalMedia> mediaFetch(String url);

  Future<rust_types.LocalMedia> mediaUploadBytes({
    required Uint8List bytes,
    required String mime,
    int width = 0,
    int height = 0,
  });

  Future<rust_types.Metrics> metricsSnapshot();

  /// One-shot secret handoff into the Rust-owned vault + Keychain.
  Future<void> storeAgentSecret({
    required String keyRef,
    required String secret,
  });

  Future<List<String>> fetchModels({
    required String vendorId,
    required String baseUrl,
    required String keyRef,
  });

  /// Vendor model cache (Rust `meta` table). Seeds new provider accounts;
  /// written by `fetchModels` on success.
  Future<List<String>> catalogModelCache(String vendor);

  Future<rust_handles.CapabilityPreview> previewProfile(String profileId);

  Future<void> workspaceGrantRegister({
    required String profileId,
    required String path,
    String? bookmark,
  });

  Future<String?> workspaceGrant(String profileId);

  /// macOS security-scoped bookmark bytes stored beside the grant. Rust
  /// stores and hands back; never interprets.
  Future<String> workspaceGrantBookmark(String profileId);

  /// Rust-owned sandbox (`support/agent/workspaces/<id>`), created + seeded.
  Future<String> ensureAgentSandbox(String profileId);

  Future<void> respondAgentPermission({
    required String callId,
    required bool allow,
  });

  Stream<rust_handles.AgentPermissionEvent> watchAgentPermission();

  Stream<rust_handles.AgentUiStatus> watchAgentUi();
}

/// Royal account HTTP login/register. Origin/UA are Rust-derived; Dart
/// passes credentials only. Logout / change-password use the stored
/// credential and live on [KimClientPort]. Tests inject a fake.
abstract class KimAuthPort {
  Future<KimAuthSession> login({
    required String account,
    required String password,
  });

  Future<KimAuthSession> register({
    required String account,
    required String password,
  });
}
