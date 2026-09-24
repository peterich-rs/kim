library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/errors.dart';
import 'package:kim_mobile/core/failures.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/src/rust/api/types.dart' as rust_types;
import 'package:kim_mobile/features/agent/agent_catalog.dart';
import 'package:kim_mobile/features/agent/agent_settings.dart';
import 'package:kim_mobile/features/agent/context_window.dart';
import 'package:kim_mobile/features/auth/providers/auth.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';
import 'package:kim_mobile/features/session/providers.dart';

part 'agent_profile_defaults.dart';
part 'agent_capability_kinds.dart';
part 'agent_profile_types.dart';
part 'agent_profile_model.dart';

class AgentProfileStore extends Notifier<List<AgentProfile>> {
  final _secure = SettingsStore.productionSecureStorage();
  Future<void>? _load;
  final _ensureInFlight = <String, Future<AgentProfile>>{};
  var multiProfile = false;

  /// `agent.server_identity`. Desktop defaults on so 1:1 can register.
  var serverIdentity = false;
  String? identityError;

  /// False until the first `_reload` finishes. Empty `[]` before this is
  /// "not loaded yet", not "no agents".
  var profilesReady = false;
  final _opaqueUnsupported = <String>{};

  @override
  List<AgentProfile> build() {
    _load = _reload();
    return const [];
  }

  Future<void> ensureLoaded() async {
    await (_load ?? _reload());
  }

  List<AgentProfile> get visibleAgents {
    if (!agentHostSupported) {
      return const [];
    }
    if (!multiProfile) {
      final g = goose;
      return g == null ? const [] : [g];
    }
    return [
      for (final p in state)
        if (p.enabled) p,
    ];
  }

  AgentProfile? get goose {
    for (final p in state) {
      if (p.id == kGooseAgentId) {
        return p;
      }
    }
    return null;
  }

  Future<String> readApiKey(AgentProfile profile) async {
    Future<String> read(String key) async {
      try {
        return await _secure.read(key: key) ?? '';
      } catch (_) {
        return '';
      }
    }

    await ref.read(providerAccountsProvider.notifier).ensureLoaded();
    final accountId = profile.accountId;
    if (accountId.isEmpty) {
      throw MissingProviderAccount(accountId);
    }
    final account = ref.read(providerAccountsProvider.notifier).byId(accountId);
    if (account == null) {
      throw MissingProviderAccount(accountId);
    }
    final keyed = await read(account.keyRef);
    if (keyed.isNotEmpty) {
      return keyed;
    }
    if (account.keyRef == _kGooseKey || profile.id == kGooseAgentId) {
      final goose = await read(_kGooseKey);
      if (goose.isNotEmpty) {
        return goose;
      }
      final legacy = await read('agent.api_key');
      if (legacy.isNotEmpty) {
        return legacy;
      }
      return ref.read(agentSettingsProvider).apiKey;
    }
    return '';
  }

  Future<void> saveProfile(AgentProfile profile) async {
    await ensureLoaded();
    final accounts = ref.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    var next = profile;
    if (next.accountId.isEmpty) {
      final account = ProviderAccount.fromLegacyProfile(
        profileId: next.id,
        providerKind: next.providerKind,
        baseUrl: next.baseUrl,
        keyRef: next.keyRef,
      );
      await accounts.upsert(account);
      next = next.copyWith(accountId: account.id);
    } else if (accounts.byId(next.accountId) == null) {
      throw MissingProviderAccount(next.accountId);
    }
    final isNew = !state.any((p) => p.id == next.id);
    if (isNew) {
      _assertCanInsert();
      await _upsertOne(next);
      await ensureBotIdentity(next);
    } else {
      await _upsertOne(next);
      // Local prefs already updated; server nickname/model sync can lag.
      unawaited(_syncBotConfig(next));
    }
  }

  /// Register a local profile on Chat when the identity flag is on and logged in.
  Future<AgentProfile> ensureBotIdentity(AgentProfile profile) {
    final pending = _ensureInFlight[profile.id];
    if (pending != null) {
      return pending;
    }
    final future = _ensureBotIdentity(profile);
    _ensureInFlight[profile.id] = future;
    return future.whenComplete(() => _ensureInFlight.remove(profile.id));
  }

  Future<AgentProfile> _ensureBotIdentity(AgentProfile profile) async {
    if (!serverIdentity ||
        !ref.read(authProvider).signedIn ||
        profile.serverAccount.isNotEmpty) {
      return profile;
    }
    final person = await ref
        .read(clientPortProvider)
        .botCreate(
          clientProfileId: profile.id,
          nickname: profile.displayName,
          model: profile.model,
          thinkingEffort: profile.thinkingEffort,
          contextTokens: profile.contextTokens,
        );
    if (person.account.isEmpty) {
      throw StateError(Copy.agentRegisterFailed);
    }
    final next = profile.copyWith(serverAccount: person.account);
    await _upsertOne(next);
    return next;
  }

  Future<void> _syncBotConfig(AgentProfile profile) async {
    if (!serverIdentity ||
        !ref.read(authProvider).signedIn ||
        profile.serverAccount.isEmpty) {
      return;
    }
    try {
      await ref
          .read(clientPortProvider)
          .botUpdate(
            dest: profile.serverAccount,
            nickname: profile.displayName,
            model: profile.model,
            thinkingEffort: profile.thinkingEffort,
            contextTokens: profile.contextTokens,
          );
    } catch (err) {
      identityError = agentRegisterError(err);
      state = [...state];
    }
  }

  void _assertCanInsert() {
    if (state.length >= kMaxBotsPerOwner) {
      throw AgentProfileCapExceeded();
    }
  }

  /// Explicit user action (`setServerIdentity(true)`), not login / online.
  /// Also registers enabled desktop personas (idempotent) so mobile can see bots.
  Future<void> ensureVisibleIdentities() async {
    await ensureLoaded();
    if (!serverIdentity || !ref.read(authProvider).signedIn) {
      return;
    }
    identityError = null;
    for (final profile in visibleAgents) {
      if (profile.serverAccount.isNotEmpty) {
        continue;
      }
      try {
        await ensureBotIdentity(profile);
      } catch (err) {
        identityError = agentRegisterError(err);
        state = [...state];
        rethrow;
      }
    }
  }

  Future<void> setServerIdentity(bool value) async {
    serverIdentity = value;
    await _writeFlags();
    state = [...state];
    if (value) {
      try {
        await ensureVisibleIdentities();
      } catch (_) {
        // [identityError] already set for settings / chat UI.
      }
    }
  }

  Future<void> _reload() async {
    await _loadFlags();
    var profiles = <AgentProfile>[];
    try {
      final client = ref.read(clientPortProvider);
      final catalog = AgentCatalog(client);
      var rows = await catalog.profiles();
      if (rows.isEmpty) {
        final imported = await _importPrefsProfiles(client);
        if (imported.isNotEmpty) {
          rows = await catalog.profiles();
        }
      }
      for (final row in rows) {
        final profile = await _fromRow(row);
        if (profile != null) {
          profiles.add(profile);
        }
      }
    } catch (e, st) {
      KimLogger.warn('agent profile rust load', e, st);
    }
    if (!ref.mounted) {
      return;
    }
    profilesReady = true;
    state = List<AgentProfile>.from(profiles);
  }

  Future<void> _loadFlags() async {
    var flags = <String, Object?>{};
    try {
      final raw = await ref.read(clientPortProvider).agentFlags();
      flags = {
        'multi_profile': raw.multiProfile,
        'server_identity': raw.serverIdentity,
      };
    } catch (_) {}
    final prefs = await SharedPreferences.getInstance();
    if (flags.isEmpty) {
      if (agentHostSupported && prefs.getBool(_kMultiMigrated) != true) {
        await prefs.setBool(_kMultiMigrated, true);
      }
      if (agentHostSupported && prefs.getBool(_kIdentityMigrated) != true) {
        await prefs.setBool(_kIdentityMigrated, true);
      }
      multiProfile = prefs.getBool(_kMulti) ?? agentHostSupported;
      serverIdentity = prefs.getBool(_kServerIdentity) ?? agentHostSupported;
      await _writeFlags();
      await prefs.remove(_kMulti);
      await prefs.remove(_kServerIdentity);
    } else {
      multiProfile = flags['multi_profile'] == true;
      serverIdentity = flags['server_identity'] == true;
    }
    await prefs.remove(_kProfiles);
  }

  Future<void> _writeFlags() async {
    try {
      await ref
          .read(clientPortProvider)
          .setAgentFlags(
            rust_types.AgentFlags(
              multiProfile: multiProfile,
              serverIdentity: serverIdentity,
            ),
          );
    } catch (e, st) {
      KimLogger.warn('agent flags persist', e, st);
    }
  }

  Future<List<AgentProfile>> _importPrefsProfiles(KimClientPort client) async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_kProfiles);
    if (raw == null || raw.isEmpty) {
      return const [];
    }
    final profiles = <AgentProfile>[];
    try {
      final decoded = jsonDecode(raw);
      if (decoded is List) {
        for (final item in decoded) {
          if (item is Map) {
            profiles.add(
              AgentProfile.fromJson(Map<String, Object?>.from(item)),
            );
          }
        }
      }
    } catch (_) {
      return const [];
    }
    if (profiles.isEmpty) {
      return const [];
    }
    await client.importAgentProfiles([
      for (final p in profiles) await _toRow(p),
    ]);
    await prefs.remove(_kProfiles);
    return profiles;
  }

  Future<rust_types.AgentProfile> _toRow(AgentProfile p) async {
    final client = ref.read(clientPortProvider);
    final blob = await client.specJsonToBlob(jsonEncode(p.toJson()));
    return rust_types.AgentProfile(
      profileId: p.id,
      nickname: p.displayName,
      serverAccount: p.serverAccount,
      bodyJson: '',
      bodyBlob: blob,
      placement: 'local',
      updatedAt: DateTime.now().millisecondsSinceEpoch,
    );
  }

  Future<AgentProfile?> _fromRow(rust_types.AgentProfile row) async {
    try {
      final client = ref.read(clientPortProvider);
      var rawJson = row.bodyJson;
      if (row.bodyBlob.isNotEmpty) {
        rawJson = await client.specBlobToJson(row.bodyBlob);
      }
      final raw = jsonDecode(rawJson);
      if (raw is Map) {
        var profile = AgentProfile.fromJson(Map<String, Object?>.from(raw))
            .copyWith(serverAccount: row.serverAccount);
        final accounts = ref.read(providerAccountsProvider.notifier);
        await accounts.ensureLoaded();
        final account = accounts.byId(profile.accountId);
        if (account != null) {
          profile = profile.copyWith(
            providerKind: canonicalizeVendorId(account.vendorId),
            baseUrl: account.baseUrl,
            keyRef: account.keyRef,
          );
        }
        try {
          final overlay = await client.getDeviceOverlay(row.profileId);
          if (overlay != null && overlay.workspacePath.isNotEmpty) {
            profile = profile.copyWith(
              workspace: profile.workspace.copyWith(
                path: overlay.workspacePath,
                bookmarkRef: overlay.workspaceBookmark,
              ),
            );
          }
        } catch (_) {}
        _opaqueUnsupported.remove(row.profileId);
        return profile;
      }
    } catch (e, st) {
      final msg = apiFailureDetail(e);
      if (msg.contains('schema_version') || msg.contains('UnsupportedSchema')) {
        _opaqueUnsupported.add(row.profileId);
        identityError = 'Agent config requires an app upgrade';
        KimLogger.warn('agent profile schema', e, st);
        return null;
      }
      KimLogger.warn('agent profile decode', e, st);
    }
    return AgentProfile(
      id: row.profileId,
      displayName: row.nickname,
      providerKind: '',
      baseUrl: '',
      model: '',
      keyRef: '',
      systemPrompt: '',
      serverAccount: row.serverAccount,
    );
  }

  Future<void> saveGoose(AgentProfile goose, {required String apiKey}) async {
    await ensureLoaded();
    final accounts = ref.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    final accountId = goose.accountId.isNotEmpty
        ? goose.accountId
        : kGooseAccountId;
    var account = accounts.byId(accountId);
    account ??= ProviderAccount(
      id: accountId,
      vendorId: canonicalizeVendorId(goose.providerKind),
      baseUrl: goose.baseUrl,
      keyRef: _kGooseKey,
      displayName: canonicalizeVendorId(goose.providerKind),
    );
    final vendorId = canonicalizeVendorId(account.vendorId);
    await accounts.upsert(account);
    try {
      if (apiKey.isEmpty) {
        await _secure.delete(key: account.keyRef);
      } else {
        await _secure.write(key: account.keyRef, value: apiKey);
      }
    } catch (_) {}
    final persisted = goose.copyWith(
      accountId: account.id,
      providerKind: vendorId,
      baseUrl: account.baseUrl,
      reasoning:
          goose.reasoning ??
          ReasoningChoice.fromThinkingEffort(goose.thinkingEffort),
    );
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_kActive, persisted.id);
    final settings = AgentSettings(
      llmBackend: vendorId,
      baseUrl: persisted.baseUrl,
      model: persisted.model,
      apiKey: apiKey,
      enableFsTools: persisted.tools.fs,
      bashEnabled: persisted.tools.bash,
      thinkingEffort: persisted.thinkingEffort,
    );
    await ref
        .read(agentSettingsProvider.notifier)
        .save(settings, persistKey: false);
    await _upsertOne(persisted);
    state = [persisted, ...state.where((p) => p.id != kGooseAgentId)];
  }

  Future<void> saveEditor(AgentProfile profile) async {
    await ensureLoaded();
    final accounts = ref.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    if (profile.accountId.isEmpty || accounts.byId(profile.accountId) == null) {
      throw MissingProviderAccount(profile.accountId);
    }
    await saveProfile(profile);
  }

  Future<void> _upsertOne(AgentProfile p) async {
    if (_opaqueUnsupported.contains(p.id)) {
      return;
    }
    try {
      final client = ref.read(clientPortProvider);
      await AgentCatalog(client).upsert(await _toRow(p));
      String? live;
      try {
        live = await ref.read(workspaceAccessProvider).realUserAgentsSkills();
      } catch (_) {
        live = null;
      }
      final previous = await client.getDeviceOverlay(p.id);
      await client.upsertDeviceOverlay(
        rust_types.DeviceOverlay(
          profileId: p.id,
          workspacePath: p.workspace.path,
          workspaceBookmark: p.workspace.bookmarkRef,
          userAgentsSkills: resolveUserAgentsSkills(
            live: live,
            overlay: previous?.userAgentsSkills ?? '',
          ),
        ),
      );
    } catch (e, st) {
      KimLogger.warn('agent profile persist', e, st);
    }
    final exists = state.any((e) => e.id == p.id);
    state = [
      for (final e in state)
        if (e.id == p.id) p else e,
      if (!exists) p,
    ];
  }

  Future<void> _removeOne(String id) async {
    try {
      await AgentCatalog(ref.read(clientPortProvider)).delete(id);
    } catch (e, st) {
      KimLogger.warn('agent profile delete', e, st);
    }
    state = [
      for (final p in state)
        if (p.id != id) p,
    ];
  }

  Future<void> setMultiProfile(bool value) async {
    multiProfile = value;
    await _writeFlags();
    state = [...state];
  }

  Future<void> setEnabled(String id, bool enabled) async {
    await ensureLoaded();
    AgentProfile? target;
    for (final p in state) {
      if (p.id == id) {
        target = p.copyWith(enabled: enabled);
        break;
      }
    }
    if (target != null) {
      await _upsertOne(target);
    }
  }

  Future<void> duplicate(AgentProfile source) async {
    await ensureLoaded();
    _assertCanInsert();
    final accounts = ref.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    var accountId = source.accountId;
    if (accountId.isEmpty) {
      final account = ProviderAccount.fromLegacyProfile(
        profileId: source.id,
        providerKind: source.providerKind,
        baseUrl: source.baseUrl,
        keyRef: source.keyRef,
      );
      await accounts.upsert(account);
      accountId = account.id;
    }
    final id = 'p-${DateTime.now().microsecondsSinceEpoch}';
    final copy = AgentProfile(
      id: id,
      displayName: '${source.displayName} copy',
      aliases: source.aliases,
      providerKind: source.providerKind,
      baseUrl: source.baseUrl,
      model: source.model,
      keyRef: source.keyRef,
      accountId: accountId,
      reasoning: source.reasoning,
      systemPrompt: source.systemPrompt,
      mode: source.mode,
      maxTurns: source.maxTurns,
      thinkingEffort: source.thinkingEffort,
      tools: source.tools,
      capabilities: source.capabilities,
      permissionOverrides: source.permissionOverrides,
      extensions: source.extensions,
      workspace: source.workspace,
      skills: source.skills,
      portableDenylist: source.portableDenylist,
      enabled: true,
      steer: source.steer,
      runtime: source.runtime,
    );
    await _upsertOne(copy);
    await ensureBotIdentity(copy);
  }

  /// In-memory persona. Does not persist or call `chat.bot.create`.
  AgentProfile draftNew({required String accountId, required String model}) {
    final id = 'p-${DateTime.now().microsecondsSinceEpoch}';
    return AgentProfile(
      id: id,
      displayName: '',
      aliases: const [],
      providerKind: '',
      baseUrl: '',
      model: model,
      keyRef: '',
      accountId: accountId,
      systemPrompt: '',
      contextTokens: defaultContextTokens(model),
      capabilities: kCreateDefaultCapabilities,
      tools: kCreateDefaultTools,
      runtime: 'goose',
    );
  }

  Future<void> delete(String id) async {
    await ensureLoaded();
    AgentProfile? profile;
    for (final p in state) {
      if (p.id == id) {
        profile = p;
        break;
      }
    }
    if (profile == null) {
      return;
    }
    if (profile.serverAccount.isNotEmpty) {
      try {
        await ref.read(clientPortProvider).botDelete(profile.serverAccount);
      } catch (err) {
        if (!isBotAlreadyGone(err)) {
          identityError = agentRegisterError(err);
          state = [...state];
          rethrow;
        }
      }
    }
    await _removeOne(id);
  }
}

final agentProfilesProvider =
    NotifierProvider<AgentProfileStore, List<AgentProfile>>(
      AgentProfileStore.new,
    );
