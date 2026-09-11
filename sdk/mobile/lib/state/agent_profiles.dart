library;

import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../agent/mention.dart';
import '../core/settings.dart';
import 'agent_settings.dart';

const _kProfiles = 'agent.profiles';
const _kActive = 'agent.active_profile_id';
const _kGooseKey = 'agent.api_key.goose';
const _kMulti = 'agent.multi_profile';

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
    this.tools = const AgentToolSet(),
    this.permissionOverrides = const {},
    this.extensions = const [],
    this.enabled = true,
    this.steer = '',
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
  final AgentToolSet tools;
  final Map<String, String> permissionOverrides;
  final List<AgentExtension> extensions;
  final bool enabled;
  final String steer;

  String get dest => id == kGooseAgentId ? kGooseAgentId : 'agent:$id';

  AgentProfile copyWith({
    String? displayName,
    String? providerKind,
    String? baseUrl,
    String? model,
    String? systemPrompt,
    String? mode,
    int? maxTurns,
    String? thinkingEffort,
    AgentToolSet? tools,
    Map<String, String>? permissionOverrides,
    List<AgentExtension>? extensions,
    bool? enabled,
    String? steer,
  }) {
    return AgentProfile(
      id: id,
      displayName: displayName ?? this.displayName,
      aliases: aliases,
      providerKind: providerKind ?? this.providerKind,
      baseUrl: baseUrl ?? this.baseUrl,
      model: model ?? this.model,
      keyRef: keyRef,
      systemPrompt: systemPrompt ?? this.systemPrompt,
      mode: mode ?? this.mode,
      maxTurns: maxTurns ?? this.maxTurns,
      thinkingEffort: thinkingEffort ?? this.thinkingEffort,
      tools: tools ?? this.tools,
      permissionOverrides: permissionOverrides ?? this.permissionOverrides,
      extensions: extensions ?? this.extensions,
      enabled: enabled ?? this.enabled,
      steer: steer ?? this.steer,
    );
  }

  Map<String, Object?> toJson() => {
    'id': id,
    'display_name': displayName,
    'aliases': aliases,
    'provider': {'kind': providerKind, 'base_url': baseUrl, 'key_ref': keyRef},
    'model': {
      'name': model,
      if (thinkingEffort.isNotEmpty) 'thinking_effort': thinkingEffort,
    },
    'system_prompt': systemPrompt,
    'mode': mode,
    'max_turns': maxTurns,
    'tools': tools.toJson(),
    'permissions': {'tools': permissionOverrides},
    'extensions': [for (final e in extensions) e.toJson()],
    'enabled': enabled,
    if (steer.isNotEmpty) 'steer': steer,
  };

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
    return AgentProfile(
      id: json['id'] as String? ?? kGooseAgentId,
      displayName: json['display_name'] as String? ?? kGooseAgentName,
      aliases: aliasesRaw is List
          ? [for (final a in aliasesRaw) '$a']
          : const [],
      providerKind: providerMap['kind'] as String? ?? 'openai',
      baseUrl: providerMap['base_url'] as String? ?? '',
      model: modelMap['name'] as String? ?? 'gpt-4o',
      keyRef: providerMap['key_ref'] as String? ?? 'agent.api_key.goose',
      systemPrompt: json['system_prompt'] as String? ?? '',
      mode: json['mode'] as String? ?? 'smart_approve',
      maxTurns: json['max_turns'] is int ? json['max_turns'] as int : null,
      thinkingEffort: modelMap['thinking_effort'] as String? ?? '',
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
    );
  }

  static AgentProfile gooseFromSettings(AgentSettings s) {
    return AgentProfile(
      id: kGooseAgentId,
      displayName: kGooseAgentName,
      aliases: const ['助手'],
      providerKind: s.llmBackend,
      baseUrl: s.baseUrl,
      model: s.model,
      keyRef: _kGooseKey,
      systemPrompt:
          'You are 助手, a local desktop agent inside the KIM messenger. '
          'You run on the user\'s machine (not a cloud bot). Reply in the user\'s language. '
          'Be concise. You can see the current conversation because the host pasted it into this session. '
          'Do not claim you have tools you were not given.',
      mode: 'smart_approve',
      maxTurns: 16,
      thinkingEffort: s.thinkingEffort,
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
  var multiProfile = false;

  @override
  List<AgentProfile> build() {
    _load = _reload();
    return const [];
  }

  Future<void> ensureLoaded() async {
    await (_load ?? _reload());
  }

  List<AgentProfile> get visibleAgents {
    if (!multiProfile) {
      final g = goose;
      return g == null ? const [] : [g];
    }
    return [for (final p in state) if (p.enabled) p];
  }

  AgentProfile? get goose {
    for (final p in state) {
      if (p.id == kGooseAgentId) {
        return p;
      }
    }
    return null;
  }

  Future<void> _reload() async {
    final prefs = await SharedPreferences.getInstance();
    multiProfile = prefs.getBool(_kMulti) ?? false;
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
    if (profiles.isEmpty) {
      await ref.read(agentSettingsProvider.notifier).ensureLoaded();
      if (!ref.mounted) {
        return;
      }
      profiles = [
        AgentProfile.gooseFromSettings(ref.read(agentSettingsProvider)),
      ];
    } else {
      profiles = [
        for (final p in profiles)
          if (p.id == kGooseAgentId &&
              !p.tools.sendMessage &&
              !p.tools.readClipboard)
            p.copyWith(
              tools: p.tools.copyWith(sendMessage: true, readClipboard: true),
            )
          else
            p,
      ];
    }
    state = profiles;
  }

  Future<void> saveGoose(AgentProfile goose, {required String apiKey}) async {
    await ensureLoaded();
    final next = [
      goose,
      ...state.where((p) => p.id != kGooseAgentId),
    ];
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(
      _kProfiles,
      jsonEncode([for (final p in next) p.toJson()]),
    );
    await prefs.setString(_kActive, goose.id);
    final settings = AgentSettings(
      llmBackend: goose.providerKind,
      baseUrl: goose.baseUrl,
      model: goose.model,
      apiKey: apiKey,
      enableFsTools: goose.tools.fs,
      bashEnabled: goose.tools.bash,
      thinkingEffort: goose.thinkingEffort,
    );
    await ref.read(agentSettingsProvider.notifier).save(settings);
    try {
      if (apiKey.isEmpty) {
        await _secure.delete(key: _kGooseKey);
      } else {
        await _secure.write(key: _kGooseKey, value: apiKey);
      }
    } catch (_) {}
    state = next;
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
    if (id == kGooseAgentId) {
      return;
    }
    await _persist([
      for (final p in state)
        if (p.id == id) p.copyWith(enabled: enabled) else p,
    ]);
  }

  Future<void> duplicate(AgentProfile source) async {
    await ensureLoaded();
    final id = 'p-${DateTime.now().millisecondsSinceEpoch}';
    final copy = AgentProfile(
      id: id,
      displayName: '${source.displayName} copy',
      aliases: source.aliases,
      providerKind: source.providerKind,
      baseUrl: source.baseUrl,
      model: source.model,
      keyRef: 'agent.api_key.$id',
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
  }

  Future<void> delete(String id) async {
    await ensureLoaded();
    if (id == kGooseAgentId) {
      return;
    }
    await _persist([for (final p in state) if (p.id != id) p]);
  }
}

final agentProfilesProvider =
    NotifierProvider<AgentProfileStore, List<AgentProfile>>(
      AgentProfileStore.new,
    );
