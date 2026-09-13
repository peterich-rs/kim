library;

import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../agent/catalog.dart';
import '../agent/host_support.dart';
import '../agent/mention.dart';
import '../copy.dart';
import '../core/settings.dart';
import 'agent_settings.dart';
import 'auth.dart';
import 'provider_accounts.dart';
import 'providers.dart';

const _kProfiles = 'agent.profiles';
const _kActive = 'agent.active_profile_id';
const _kGooseKey = 'agent.api_key.goose';
const _kMulti = 'agent.multi_profile';
const _kMultiMigrated = 'agent.multi_profile_migrated_on';
const _kServerIdentity = 'agent.server_identity';
const _kIdentityMigrated = 'agent.identity_migrated_on';

/// Byte-identical to Rust `DEFAULT_SYSTEM_PROMPT`. Empty prompt injects this.
const kDefaultSystemPrompt =
    'You are 助手, a local desktop agent inside the KIM messenger. '
    'You run on the user\'s machine (not a cloud bot). Reply in the user\'s language. '
    'Be concise. You can see the current conversation because the host pasted it into this session. '
    'You have search_contacts, search_messages, get_conversation_context, list_profiles, '
    'send_message, and read_clipboard. send_message and clipboard require user confirmation. '
    'You do not have filesystem or shell access. Do not claim you have tools you were not given.';

/// New Agent tools. Do not rewrite an existing goose row.
const kCreateDefaultTools = AgentToolSet(
  sendMessage: true,
  readClipboard: true,
);

/// Aligns with Chat `BOT_MAX_PER_OWNER`.
const kMaxBotsPerOwner = 20;

class AgentProfileCapExceeded implements Exception {
  @override
  String toString() => Copy.agentCapReached;
}

/// Profiles that have or will have a cloud bot when [serverIdentity] is on,
/// including disabled rows.
int cloudIdentitySlots(
  Iterable<AgentProfile> profiles, {
  required bool serverIdentity,
}) {
  var n = 0;
  for (final p in profiles) {
    if (p.serverAccount.isNotEmpty || serverIdentity) {
      n++;
    }
  }
  return n;
}

bool isBotAlreadyGone(Object err) {
  final msg = err.toString().toLowerCase();
  return msg.contains('status 108') ||
      msg.contains('not_owner') ||
      msg.contains('not owner') ||
      msg.contains(Copy.userNotFound.toLowerCase());
}

String agentRegisterError(Object err) {
  final msg = err.toString();
  if (msg.contains('status 2')) {
    return Copy.agentRegisterFailed;
  }
  if (err is StateError && err.message.isNotEmpty) {
    return err.message;
  }
  return Copy.agentRegisterFailed;
}

class AgentToolSet {
  const AgentToolSet({
    this.sendMessage = false,
    this.searchContacts = false,
    this.searchMessages = false,
    this.getConversationContext = false,
    this.readClipboard = false,
    this.listProfiles = false,
    this.fs = false,
    this.fsWrite = false,
    this.bash = false,
    this.subagent = false,
  });

  final bool sendMessage;
  final bool searchContacts;
  final bool searchMessages;
  final bool getConversationContext;
  final bool readClipboard;
  final bool listProfiles;
  final bool fs;
  final bool fsWrite;
  final bool bash;
  final bool subagent;

  Map<String, Object?> toJson() => {
    'send_message': sendMessage,
    'search_contacts': searchContacts,
    'search_messages': searchMessages,
    'get_conversation_context': getConversationContext,
    'read_clipboard': readClipboard,
    'list_profiles': listProfiles,
    'fs': fs,
    'fs_write': fsWrite,
    'bash': bash,
    'subagent': subagent,
  };

  factory AgentToolSet.fromJson(Map<String, Object?> json) {
    bool b(String k) => json[k] == true;
    return AgentToolSet(
      sendMessage: b('send_message'),
      searchContacts: b('search_contacts'),
      searchMessages: b('search_messages'),
      getConversationContext: b('get_conversation_context'),
      readClipboard: b('read_clipboard'),
      listProfiles: b('list_profiles'),
      fs: b('fs'),
      fsWrite: b('fs_write'),
      bash: b('bash'),
      subagent: b('subagent'),
    );
  }

  AgentToolSet copyWith({
    bool? sendMessage,
    bool? searchContacts,
    bool? searchMessages,
    bool? getConversationContext,
    bool? readClipboard,
    bool? listProfiles,
    bool? fs,
    bool? fsWrite,
    bool? bash,
    bool? subagent,
  }) {
    return AgentToolSet(
      sendMessage: sendMessage ?? this.sendMessage,
      searchContacts: searchContacts ?? this.searchContacts,
      searchMessages: searchMessages ?? this.searchMessages,
      getConversationContext:
          getConversationContext ?? this.getConversationContext,
      readClipboard: readClipboard ?? this.readClipboard,
      listProfiles: listProfiles ?? this.listProfiles,
      fs: fs ?? this.fs,
      fsWrite: fsWrite ?? this.fsWrite,
      bash: bash ?? this.bash,
      subagent: subagent ?? this.subagent,
    );
  }
}

class AgentExtension {
  const AgentExtension({
    required this.name,
    this.transport = 'stdio',
    this.command = const [],
    this.url = '',
  });

  final String name;
  final String transport;
  final List<String> command;
  final String url;

  Map<String, Object?> toJson() => {
    'name': name,
    'transport': transport,
    'command': command,
    'url': url,
  };

  factory AgentExtension.fromJson(Map<String, Object?> json) {
    final cmd = json['command'];
    return AgentExtension(
      name: '${json['name'] ?? ''}',
      transport: '${json['transport'] ?? 'stdio'}',
      command: cmd is List ? [for (final c in cmd) '$c'] : const [],
      url: '${json['url'] ?? ''}',
    );
  }
}

class ReasoningChoice {
  const ReasoningChoice({
    this.v = 1,
    required this.kind,
    this.on,
    this.value,
    this.budget,
    this.advanced,
  });

  final int v;
  final String kind;
  final bool? on;
  final String? value;
  final int? budget;
  final Map<String, Object?>? advanced;

  Map<String, Object?> toJson() => {
    'v': v,
    'kind': kind,
    if (on != null) 'on': on,
    if (value != null) 'value': value,
    if (budget != null) 'value': budget,
    if (advanced != null) 'json': advanced,
  };

  factory ReasoningChoice.fromJson(Map<String, Object?> json) {
    final raw = json['value'];
    return ReasoningChoice(
      v: json['v'] is int ? json['v'] as int : 1,
      kind: '${json['kind'] ?? 'none'}',
      on: json['on'] as bool?,
      value: raw is String ? raw : null,
      budget: raw is int ? raw : null,
      advanced: json['json'] is Map
          ? Map<String, Object?>.from(json['json'] as Map)
          : null,
    );
  }

  static ReasoningChoice? fromThinkingEffort(String effort) {
    final trimmed = effort.trim();
    if (trimmed.isEmpty) {
      return null;
    }
    if (trimmed == 'off' || trimmed == 'none' || trimmed == 'disabled') {
      return const ReasoningChoice(kind: 'none');
    }
    return ReasoningChoice(kind: 'effort_enum', value: trimmed);
  }
}

class AgentProfile {
  const AgentProfile({
    required this.id,
    required this.displayName,
    this.aliases = const [],
    required this.providerKind,
    required this.baseUrl,
    required this.model,
    required this.keyRef,
    required this.systemPrompt,
    this.mode = 'smart_approve',
    this.maxTurns,
    this.thinkingEffort = '',
    this.accountId = '',
    this.reasoning,
    this.tools = const AgentToolSet(),
    this.permissionOverrides = const {},
    this.extensions = const [],
    this.enabled = true,
    this.steer = '',
    this.serverAccount = '',
  });

  final String id;
  final String displayName;
  final List<String> aliases;
  final String providerKind;
  final String baseUrl;
  final String model;
  final String keyRef;
  final String systemPrompt;
  final String mode;
  final int? maxTurns;
  final String thinkingEffort;
  final String accountId;
  final ReasoningChoice? reasoning;
  final AgentToolSet tools;
  final Map<String, String> permissionOverrides;
  final List<AgentExtension> extensions;
  final bool enabled;
  final String steer;

  /// Empty = unregistered. Not a secret. IM dest after `chat.bot.create`.
  final String serverAccount;

  String get dest => id == kGooseAgentId ? kGooseAgentId : 'agent:$id';

  AgentProfile copyWith({
    String? displayName,
    List<String>? aliases,
    String? providerKind,
    String? baseUrl,
    String? model,
    String? systemPrompt,
    String? mode,
    int? maxTurns,
    String? thinkingEffort,
    String? accountId,
    ReasoningChoice? reasoning,
    AgentToolSet? tools,
    Map<String, String>? permissionOverrides,
    List<AgentExtension>? extensions,
    bool? enabled,
    String? steer,
    String? serverAccount,
  }) {
    return AgentProfile(
      id: id,
      displayName: displayName ?? this.displayName,
      aliases: aliases ?? this.aliases,
      providerKind: providerKind ?? this.providerKind,
      baseUrl: baseUrl ?? this.baseUrl,
      model: model ?? this.model,
      keyRef: keyRef,
      systemPrompt: systemPrompt ?? this.systemPrompt,
      mode: mode ?? this.mode,
      maxTurns: maxTurns ?? this.maxTurns,
      thinkingEffort: thinkingEffort ?? this.thinkingEffort,
      accountId: accountId ?? this.accountId,
      reasoning: reasoning ?? this.reasoning,
      tools: tools ?? this.tools,
      permissionOverrides: permissionOverrides ?? this.permissionOverrides,
      extensions: extensions ?? this.extensions,
      enabled: enabled ?? this.enabled,
      steer: steer ?? this.steer,
      serverAccount: serverAccount ?? this.serverAccount,
    );
  }

  /// Disk JSON: no `provider` block (C-KD 1).
  Map<String, Object?> toJson() => {
    'id': id,
    'display_name': displayName,
    'aliases': aliases,
    if (accountId.isNotEmpty) 'account_id': accountId,
    'model': {
      'name': model,
      if (reasoning == null && thinkingEffort.isNotEmpty)
        'thinking_effort': thinkingEffort,
    },
    if (reasoning != null) 'reasoning': reasoning!.toJson(),
    'system_prompt': systemPrompt,
    'mode': mode,
    'max_turns': maxTurns,
    'tools': tools.toJson(),
    'permissions': {'tools': permissionOverrides},
    'extensions': [for (final e in extensions) e.toJson()],
    'enabled': enabled,
    if (steer.isNotEmpty) 'steer': steer,
    'server_account': serverAccount,
  };

  Map<String, Object?> toHostJson(ProviderAccount account) {
    final json = toJson();
    json['provider'] = {
      'kind': canonicalizeVendorId(account.vendorId),
      'base_url': account.baseUrl,
      'key_ref': '',
    };
    return json;
  }

  factory AgentProfile.fromJson(Map<String, Object?> json) {
    final provider = json['provider'];
    final model = json['model'];
    final providerMap = provider is Map
        ? Map<String, Object?>.from(provider)
        : <String, Object?>{};
    final modelMap = model is Map
        ? Map<String, Object?>.from(model)
        : <String, Object?>{};
    final toolsRaw = json['tools'];
    final permRaw = json['permissions'];
    final permMap = permRaw is Map
        ? Map<String, Object?>.from(permRaw)
        : <String, Object?>{};
    final permTools = permMap['tools'];
    final overrides = <String, String>{};
    if (permTools is Map) {
      for (final e in permTools.entries) {
        overrides['${e.key}'] = '${e.value}';
      }
    }
    final aliasesRaw = json['aliases'];
    final reasoningRaw = json['reasoning'];
    final kind = canonicalizeVendorId(providerMap['kind'] as String? ?? '');
    return AgentProfile(
      id: json['id'] as String? ?? kGooseAgentId,
      displayName: json['display_name'] as String? ?? kGooseAgentName,
      aliases: aliasesRaw is List
          ? [for (final a in aliasesRaw) '$a']
          : const [],
      providerKind: kind.isEmpty ? 'openai' : kind,
      baseUrl: providerMap['base_url'] as String? ?? '',
      model: modelMap['name'] as String? ?? 'gpt-4o',
      keyRef: providerMap['key_ref'] as String? ?? 'agent.api_key.goose',
      systemPrompt: json['system_prompt'] as String? ?? '',
      mode: json['mode'] as String? ?? 'smart_approve',
      maxTurns: json['max_turns'] is int ? json['max_turns'] as int : null,
      thinkingEffort: modelMap['thinking_effort'] as String? ?? '',
      accountId: json['account_id'] as String? ?? '',
      reasoning: reasoningRaw is Map
          ? ReasoningChoice.fromJson(Map<String, Object?>.from(reasoningRaw))
          : ReasoningChoice.fromThinkingEffort(
              modelMap['thinking_effort'] as String? ?? '',
            ),
      tools: toolsRaw is Map
          ? AgentToolSet.fromJson(Map<String, Object?>.from(toolsRaw))
          : const AgentToolSet(),
      permissionOverrides: overrides,
      extensions: () {
        final raw = json['extensions'];
        if (raw is! List) {
          return const <AgentExtension>[];
        }
        return [
          for (final item in raw)
            if (item is Map)
              AgentExtension.fromJson(Map<String, Object?>.from(item)),
        ];
      }(),
      enabled: json['enabled'] != false,
      steer: json['steer'] as String? ?? '',
      serverAccount: json['server_account'] as String? ?? '',
    );
  }

  static AgentProfile gooseFromSettings(AgentSettings s) {
    return AgentProfile(
      id: kGooseAgentId,
      displayName: kGooseAgentName,
      aliases: const ['助手'],
      providerKind: canonicalizeVendorId(s.llmBackend),
      baseUrl: s.baseUrl,
      model: s.model,
      keyRef: _kGooseKey,
      accountId: '',
      systemPrompt: kDefaultSystemPrompt,
      mode: 'smart_approve',
      maxTurns: 16,
      thinkingEffort: s.thinkingEffort,
      reasoning: ReasoningChoice.fromThinkingEffort(s.thinkingEffort),
      tools: const AgentToolSet(
        sendMessage: true,
        searchContacts: true,
        searchMessages: true,
        getConversationContext: true,
        readClipboard: true,
        listProfiles: true,
      ),
    );
  }
}

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
      await _persist([...state, next]);
      await ensureBotIdentity(next);
    } else {
      await _persist([
        for (final p in state)
          if (p.id == next.id) next else p,
      ]);
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
        .botCreate(clientProfileId: profile.id, nickname: profile.displayName);
    if (person.account.isEmpty) {
      throw StateError(Copy.agentRegisterFailed);
    }
    final next = profile.copyWith(serverAccount: person.account);
    await _persist([
      for (final p in state)
        if (p.id == profile.id) next else p,
    ]);
    return next;
  }

  void _assertCanInsert() {
    if (state.length >= kMaxBotsPerOwner) {
      throw AgentProfileCapExceeded();
    }
  }

  /// Explicit user action (`setServerIdentity(true)`), not login / online.
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
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_kServerIdentity, value);
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
    final prefs = await SharedPreferences.getInstance();
    if (agentHostSupported && prefs.getBool(_kMultiMigrated) != true) {
      if (prefs.getBool(_kMulti) == false) {
        await prefs.setBool(_kMulti, true);
      }
      await prefs.setBool(_kMultiMigrated, true);
    }
    if (agentHostSupported && prefs.getBool(_kIdentityMigrated) != true) {
      if (prefs.getBool(_kServerIdentity) == false) {
        await prefs.setBool(_kServerIdentity, true);
      }
      await prefs.setBool(_kIdentityMigrated, true);
    }
    multiProfile = prefs.getBool(_kMulti) ?? agentHostSupported;
    serverIdentity = prefs.getBool(_kServerIdentity) ?? agentHostSupported;
    final raw = prefs.getString(_kProfiles);
    var profiles = <AgentProfile>[];
    if (raw != null && raw.isNotEmpty) {
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
        profiles = [];
      }
    }
    if (!ref.mounted) {
      return;
    }
    profiles = await _migrateAccounts(profiles);
    if (!ref.mounted) {
      return;
    }
    profilesReady = true;
    state = List<AgentProfile>.from(profiles);
    await _migrateAccountModels();
  }

  Future<List<AgentProfile>> _migrateAccounts(
    List<AgentProfile> profiles,
  ) async {
    if (!ref.mounted) {
      return profiles;
    }
    final accounts = ref.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    if (!ref.mounted) {
      return profiles;
    }
    var changed = false;
    final next = <AgentProfile>[];
    for (var p in profiles) {
      if (p.accountId.isNotEmpty) {
        final account = accounts.byId(p.accountId);
        if (account == null) {
          // Keep the dangling account_id so readApiKey / open throw.
          next.add(p);
          continue;
        }
        p = p.copyWith(
          providerKind: canonicalizeVendorId(account.vendorId),
          baseUrl: account.baseUrl,
        );
        next.add(p);
        continue;
      }
      final account = ProviderAccount.fromLegacyProfile(
        profileId: p.id,
        providerKind: p.providerKind,
        baseUrl: p.baseUrl,
        keyRef: p.keyRef,
      );
      await accounts.upsert(account);
      if (!ref.mounted) {
        return next;
      }
      p = p.copyWith(
        accountId: account.id,
        providerKind: canonicalizeVendorId(account.vendorId),
        baseUrl: account.baseUrl,
      );
      changed = true;
      next.add(p);
    }
    if (changed) {
      await _persist(next);
    }
    return next;
  }

  Future<void> _migrateAccountModels() async {
    if (!ref.mounted) {
      return;
    }
    final accounts = ref.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    if (!ref.mounted) {
      return;
    }
    final pending = [
      for (final a in ref.read(providerAccountsProvider))
        if (a.models.isEmpty) a,
    ];
    if (pending.isEmpty) {
      return;
    }
    var vendors = const <VendorSummaryDto>[];
    try {
      vendors = await ref.read(catalogRepositoryProvider).ensureVendors();
    } catch (_) {}
    if (!ref.mounted) {
      return;
    }
    for (final account in pending) {
      VendorSummaryDto? vendor;
      for (final v in vendors) {
        if (v.id == account.vendorId) {
          vendor = v;
          break;
        }
      }
      final models = await migrateAccountModelIds(
        vendorId: account.vendorId,
        existing: account.models,
        catalogModels: vendor?.models ?? const [],
        defaultModel: vendor?.defaultModel ?? '',
      );
      if (!ref.mounted) {
        return;
      }
      if (models.isEmpty) {
        continue;
      }
      await accounts.upsert(account.copyWith(models: models));
    }
  }

  Future<void> saveGoose(AgentProfile goose, {required String apiKey}) async {
    await ensureLoaded();
    final accounts = ref.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    final accountId = goose.accountId.isNotEmpty
        ? goose.accountId
        : kGooseAccountId;
    final vendorId = canonicalizeVendorId(goose.providerKind);
    var account = accounts.byId(accountId);
    account ??= ProviderAccount(
      id: accountId,
      vendorId: vendorId,
      baseUrl: goose.baseUrl,
      keyRef: _kGooseKey,
      displayName: vendorId,
    );
    account = account.copyWith(vendorId: vendorId, baseUrl: goose.baseUrl);
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
      baseUrl: goose.baseUrl,
      reasoning:
          goose.reasoning ??
          ReasoningChoice.fromThinkingEffort(goose.thinkingEffort),
    );
    final next = [persisted, ...state.where((p) => p.id != kGooseAgentId)];
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(
      _kProfiles,
      jsonEncode([for (final p in next) p.toJson()]),
    );
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
    state = next;
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

  Future<void> _persist(List<AgentProfile> next) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(
      _kProfiles,
      jsonEncode([for (final p in next) p.toJson()]),
    );
    state = next;
  }

  Future<void> setMultiProfile(bool value) async {
    multiProfile = value;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_kMulti, value);
    state = [...state];
  }

  Future<void> setEnabled(String id, bool enabled) async {
    await ensureLoaded();
    await _persist([
      for (final p in state)
        if (p.id == id) p.copyWith(enabled: enabled) else p,
    ]);
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
      permissionOverrides: source.permissionOverrides,
      extensions: source.extensions,
      enabled: true,
      steer: source.steer,
    );
    await _persist([...state, copy]);
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
      tools: kCreateDefaultTools,
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
    await _persist([
      for (final p in state)
        if (p.id != id) p,
    ]);
  }
}

final agentProfilesProvider =
    NotifierProvider<AgentProfileStore, List<AgentProfile>>(
      AgentProfileStore.new,
    );
