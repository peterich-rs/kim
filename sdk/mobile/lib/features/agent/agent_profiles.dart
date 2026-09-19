library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:kim_mobile/features/agent/agent_settings.dart';
import 'package:kim_mobile/features/agent/context_window.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';
import 'package:kim_mobile/features/session/providers.dart';

const _kProfiles = 'agent.profiles';
const _kActive = 'agent.active_profile_id';
const _kGooseKey = 'agent.api_key.goose';
const _kMulti = 'agent.multi_profile';
const _kMultiMigrated = 'agent.multi_profile_migrated_on';
const _kServerIdentity = 'agent.server_identity';
const _kIdentityMigrated = 'agent.identity_migrated_on';

/// Byte-identical to Rust `DEFAULT_IDENTITY_PROMPT`. Empty prompt injects this.
const kDefaultSystemPrompt =
    'You are a local desktop agent inside the KIM messenger. '
    'You run on the user\'s machine (not a cloud bot). Reply in the user\'s language. Be concise. '
    'Only use tools that appear in your tool list; never claim tools you were not given.';

/// Pre-B-KD 4 identity that enumerated every IM tool. Load as empty.
const kLegacyToolLaundryIdentity =
    'You are 助手, a local desktop agent inside the KIM messenger. '
    'You run on the user\'s machine (not a cloud bot). Reply in the user\'s language. '
    'Be concise. You can see the current conversation because the host pasted it into this session. '
    'You have search_contacts, search_messages, get_conversation_context, list_profiles, '
    'send_message, and read_clipboard. send_message and clipboard require user confirmation. '
    'You do not have filesystem or shell access. Do not claim you have tools you were not given.';

bool isLegacyToolLaundryIdentity(String prompt) {
  final t = prompt.trim();
  if (t.isEmpty) {
    return false;
  }
  if (t == kLegacyToolLaundryIdentity) {
    return true;
  }
  return t.contains('You are 助手') &&
      t.contains('search_contacts') &&
      t.contains('search_messages') &&
      t.contains('get_conversation_context') &&
      t.contains('list_profiles') &&
      t.contains('send_message') &&
      t.contains('read_clipboard') &&
      t.contains('You do not have filesystem or shell access');
}

String migrateIdentityPrompt(String prompt) {
  return isLegacyToolLaundryIdentity(prompt) ? '' : prompt;
}

/// Default capabilities for a new persona (B-KD create defaults).
const kCreateDefaultCapabilities = <CapabilityRef>[
  CapabilityRef(kind: CapabilityKinds.imSendMessage),
  CapabilityRef(kind: CapabilityKinds.imReadClipboard),
];

/// Projection of [kCreateDefaultCapabilities] for one-release ToolSet compat.
final kCreateDefaultTools = projectToolSet(kCreateDefaultCapabilities);

/// Capability kind strings — keep in sync with host registry (B-KD 2).
abstract final class CapabilityKinds {
  static const imSendMessage = 'im.send_message';
  static const imSearchContacts = 'im.search_contacts';
  static const imSearchMessages = 'im.search_messages';
  static const imGetConversationContext = 'im.get_conversation_context';
  static const imReadClipboard = 'im.read_clipboard';
  static const imListProfiles = 'im.list_profiles';
  static const fs = 'fs';
  static const bash = 'bash';
  static const mcp = 'mcp';
  static const subagent = 'subagent';
}

/// Persisted capability reference. Mirrors host `CapabilityRef` serde.
class CapabilityRef {
  const CapabilityRef({
    required this.kind,
    this.id = '',
    this.params = const {},
    this.enabled = true,
  });

  final String kind;
  final String id;
  final Map<String, Object?> params;
  final bool enabled;

  Map<String, Object?> toJson() => {
    'kind': kind,
    if (id.isNotEmpty) 'id': id,
    if (params.isNotEmpty) 'params': params,
    'enabled': enabled,
  };

  factory CapabilityRef.fromJson(Map<String, Object?> json) {
    final rawParams = json['params'];
    return CapabilityRef(
      kind: '${json['kind'] ?? ''}',
      id: '${json['id'] ?? ''}',
      params: rawParams is Map
          ? Map<String, Object?>.from(rawParams)
          : const {},
      enabled: json['enabled'] != false,
    );
  }

  CapabilityRef copyWith({
    String? kind,
    String? id,
    Map<String, Object?>? params,
    bool? enabled,
  }) {
    return CapabilityRef(
      kind: kind ?? this.kind,
      id: id ?? this.id,
      params: params ?? this.params,
      enabled: enabled ?? this.enabled,
    );
  }

  @override
  bool operator ==(Object other) {
    return other is CapabilityRef &&
        other.kind == kind &&
        other.id == id &&
        other.enabled == enabled &&
        jsonEncode(other.params) == jsonEncode(params);
  }

  @override
  int get hashCode => Object.hash(kind, id, enabled, jsonEncode(params));
}

/// Appendix A / host `CapabilityRef::from_legacy`.
List<CapabilityRef> capabilitiesFromLegacy(
  AgentToolSet tools,
  List<AgentExtension> extensions,
) {
  final out = <CapabilityRef>[];
  if (tools.sendMessage) {
    out.add(const CapabilityRef(kind: CapabilityKinds.imSendMessage));
  }
  if (tools.searchContacts) {
    out.add(const CapabilityRef(kind: CapabilityKinds.imSearchContacts));
  }
  if (tools.searchMessages) {
    out.add(const CapabilityRef(kind: CapabilityKinds.imSearchMessages));
  }
  if (tools.getConversationContext) {
    out.add(
      const CapabilityRef(kind: CapabilityKinds.imGetConversationContext),
    );
  }
  if (tools.readClipboard) {
    out.add(const CapabilityRef(kind: CapabilityKinds.imReadClipboard));
  }
  if (tools.listProfiles) {
    out.add(const CapabilityRef(kind: CapabilityKinds.imListProfiles));
  }
  if (tools.fsWrite) {
    out.add(
      const CapabilityRef(kind: CapabilityKinds.fs, params: {'writable': true}),
    );
  } else if (tools.fs) {
    out.add(
      const CapabilityRef(
        kind: CapabilityKinds.fs,
        params: {'writable': false},
      ),
    );
  }
  if (tools.bash) {
    out.add(const CapabilityRef(kind: CapabilityKinds.bash));
  }
  if (tools.subagent) {
    out.add(const CapabilityRef(kind: CapabilityKinds.subagent));
  }
  for (final e in extensions) {
    if (e.name.isEmpty) {
      continue;
    }
    out.add(
      CapabilityRef(
        kind: CapabilityKinds.mcp,
        id: 'mcp:${e.name}',
        params: {
          'name': e.name,
          'command': e.command,
          'transport': e.transport,
          if (e.url.isNotEmpty) 'url': e.url,
        },
      ),
    );
  }
  return out;
}

/// Host `ToolSet::from_capabilities` / `project_toolset`.
AgentToolSet projectToolSet(List<CapabilityRef> capabilities) {
  var sendMessage = false;
  var searchContacts = false;
  var searchMessages = false;
  var getConversationContext = false;
  var readClipboard = false;
  var listProfiles = false;
  var fs = false;
  var fsWrite = false;
  var bash = false;
  var subagent = false;
  for (final c in capabilities) {
    if (!c.enabled) {
      continue;
    }
    switch (c.kind) {
      case CapabilityKinds.imSendMessage:
        sendMessage = true;
      case CapabilityKinds.imSearchContacts:
        searchContacts = true;
      case CapabilityKinds.imSearchMessages:
        searchMessages = true;
      case CapabilityKinds.imGetConversationContext:
        getConversationContext = true;
      case CapabilityKinds.imReadClipboard:
        readClipboard = true;
      case CapabilityKinds.imListProfiles:
        listProfiles = true;
      case CapabilityKinds.fs:
        fs = true;
        if (c.params['writable'] == true) {
          fsWrite = true;
        }
      case CapabilityKinds.bash:
        bash = true;
      case CapabilityKinds.subagent:
        subagent = true;
      default:
        break;
    }
  }
  return AgentToolSet(
    sendMessage: sendMessage,
    searchContacts: searchContacts,
    searchMessages: searchMessages,
    getConversationContext: getConversationContext,
    readClipboard: readClipboard,
    listProfiles: listProfiles,
    fs: fs,
    fsWrite: fsWrite,
    bash: bash,
    subagent: subagent,
  );
}

/// MCP capability refs → [AgentExtension] rows (one-release dual write).
/// Empty input → empty output (clearing MCP must wipe leftover extensions).
List<AgentExtension> projectExtensions(List<CapabilityRef> capabilities) {
  final out = <AgentExtension>[];
  for (final c in capabilities) {
    if (!c.enabled || c.kind != CapabilityKinds.mcp) {
      continue;
    }
    final name = '${c.params['name'] ?? ''}'.trim();
    if (name.isEmpty) {
      continue;
    }
    final cmd = c.params['command'];
    out.add(
      AgentExtension(
        name: name,
        transport: '${c.params['transport'] ?? 'stdio'}',
        command: cmd is List ? [for (final x in cmd) '$x'] : const [],
        url: '${c.params['url'] ?? ''}',
      ),
    );
  }
  return out;
}

/// Parse MCP textarea lines (`name argv0 argv1…`).
List<CapabilityRef> mcpCapsFromText(String text) {
  final mcp = <CapabilityRef>[];
  for (final line in text.split('\n')) {
    final parts = line
        .trim()
        .split(RegExp(r'\s+'))
        .where((p) => p.isNotEmpty)
        .toList();
    if (parts.length < 2) {
      continue;
    }
    final name = parts.first;
    mcp.add(
      CapabilityRef(
        kind: CapabilityKinds.mcp,
        id: 'mcp:$name',
        params: {
          'name': name,
          'command': parts.sublist(1),
          'transport': 'stdio',
        },
      ),
    );
  }
  return mcp;
}

String mcpTextFromCaps(List<CapabilityRef> caps) {
  final lines = <String>[];
  for (final c in caps) {
    if (!c.enabled || c.kind != CapabilityKinds.mcp) {
      continue;
    }
    final name = '${c.params['name'] ?? ''}'.trim();
    final cmd = c.params['command'];
    final args = cmd is List ? [for (final x in cmd) '$x'] : const <String>[];
    if (name.isEmpty || args.isEmpty) {
      continue;
    }
    lines.add('$name ${args.join(' ')}');
  }
  return lines.join('\n');
}

/// Keep singleton caps, replace MCP from the textarea (autosave must not drop it).
List<CapabilityRef> mergeCapsWithMcpLines(
  List<CapabilityRef> caps,
  String mcpText,
) {
  return [
    for (final c in caps)
      if (c.kind != CapabilityKinds.mcp) c,
    ...mcpCapsFromText(mcpText),
  ];
}

/// Local fallback tool names when host `preview_assembled` is unavailable.
List<String> projectedToolNames(List<CapabilityRef> capabilities) {
  final tools = projectToolSet(capabilities);
  final names = <String>[
    if (tools.sendMessage) 'send_message',
    if (tools.searchContacts) 'search_contacts',
    if (tools.searchMessages) 'search_messages',
    if (tools.getConversationContext) 'get_conversation_context',
    if (tools.readClipboard) 'read_clipboard',
    if (tools.listProfiles) 'list_profiles',
    if (tools.fs || tools.fsWrite) 'read_file',
    if (tools.fs || tools.fsWrite) 'list_dir',
    if (tools.fsWrite) 'write_file',
    if (tools.bash) 'bash',
    if (tools.subagent) 'delegate',
  ];
  for (final c in capabilities) {
    if (c.enabled && c.kind == CapabilityKinds.mcp) {
      final name = '${c.params['name'] ?? ''}'.trim();
      if (name.isNotEmpty) {
        names.add('mcp:$name');
      }
    }
  }
  return names;
}

/// Upsert/remove a singleton kind (IM / fs / bash / subagent).
List<CapabilityRef> upsertCapability(
  List<CapabilityRef> current, {
  required String kind,
  required bool enabled,
  Map<String, Object?> params = const {},
  String id = '',
}) {
  final without = [
    for (final c in current)
      if (c.kind != kind || (id.isNotEmpty && c.id != id)) c,
  ];
  if (!enabled) {
    return without;
  }
  return [
    ...without,
    CapabilityRef(kind: kind, id: id, params: params, enabled: true),
  ];
}

/// Enable capability kinds required by legacy tool-name requires lists.
List<CapabilityRef> enableRequiredCapabilities(
  List<CapabilityRef> caps,
  List<String> missingToolNames,
) {
  var next = List<CapabilityRef>.from(caps);
  for (final name in missingToolNames) {
    switch (name) {
      case 'send_message':
        next = upsertCapability(
          next,
          kind: CapabilityKinds.imSendMessage,
          enabled: true,
        );
      case 'search_contacts':
        next = upsertCapability(
          next,
          kind: CapabilityKinds.imSearchContacts,
          enabled: true,
        );
      case 'search_messages':
        next = upsertCapability(
          next,
          kind: CapabilityKinds.imSearchMessages,
          enabled: true,
        );
      case 'get_conversation_context':
        next = upsertCapability(
          next,
          kind: CapabilityKinds.imGetConversationContext,
          enabled: true,
        );
      case 'list_profiles':
        next = upsertCapability(
          next,
          kind: CapabilityKinds.imListProfiles,
          enabled: true,
        );
      case 'read_clipboard':
        next = upsertCapability(
          next,
          kind: CapabilityKinds.imReadClipboard,
          enabled: true,
        );
      case 'fs':
        next = upsertCapability(
          next,
          kind: CapabilityKinds.fs,
          enabled: true,
          params: const {'writable': false},
        );
      case 'fs_write':
        next = upsertCapability(
          next,
          kind: CapabilityKinds.fs,
          enabled: true,
          params: const {'writable': true},
        );
      case 'bash':
        next = upsertCapability(
          next,
          kind: CapabilityKinds.bash,
          enabled: true,
        );
      case 'subagent':
        next = upsertCapability(
          next,
          kind: CapabilityKinds.subagent,
          enabled: true,
        );
      default:
        break;
    }
  }
  return next;
}

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

/// Per-agent cwd. Missing / unknown → sandbox (S-KD 4).
class WorkspaceSpec {
  const WorkspaceSpec({
    this.kind = kindSandbox,
    this.path = '',
    this.bookmarkRef = '',
  });

  static const kindSandbox = 'sandbox';
  static const kindRepo = 'repo';
  static const sandbox = WorkspaceSpec();

  final String kind;
  final String path;

  /// Keychain / support key for macOS security-scoped bookmark (PR2).
  /// Persisted in prefs with the profile; stripped from [toHostJson].
  final String bookmarkRef;

  bool get isSandbox => kind != kindRepo;
  bool get isRepo => kind == kindRepo;

  Map<String, Object?> toJson() => {
    'kind': isRepo ? kindRepo : kindSandbox,
    if (path.isNotEmpty) 'path': path,
    if (bookmarkRef.isNotEmpty) 'bookmark_ref': bookmarkRef,
  };

  /// Host only needs kind + path; bookmark stays on the Dart side.
  Map<String, Object?> toHostJson() => {
    'kind': isRepo ? kindRepo : kindSandbox,
    if (path.isNotEmpty) 'path': path,
  };

  factory WorkspaceSpec.fromJson(Map<String, Object?>? json) {
    if (json == null) {
      return sandbox;
    }
    final kind = '${json['kind'] ?? ''}'.trim();
    return WorkspaceSpec(
      kind: kind == kindRepo ? kindRepo : kindSandbox,
      path: '${json['path'] ?? ''}',
      bookmarkRef: '${json['bookmark_ref'] ?? ''}',
    );
  }

  WorkspaceSpec copyWith({String? kind, String? path, String? bookmarkRef}) {
    return WorkspaceSpec(
      kind: kind ?? this.kind,
      path: path ?? this.path,
      bookmarkRef: bookmarkRef ?? this.bookmarkRef,
    );
  }

  @override
  bool operator ==(Object other) {
    return other is WorkspaceSpec &&
        other.kind == kind &&
        other.path == path &&
        other.bookmarkRef == bookmarkRef;
  }

  @override
  int get hashCode => Object.hash(kind, path, bookmarkRef);
}

/// App / built-in skill assignment (portable skills are discovered, not stored).
class SkillRef {
  const SkillRef({
    required this.id,
    this.className = classApp,
    this.origin = 'bundled',
    this.version = '',
    this.enabled = true,
  });

  static const classApp = 'app';
  static const classPortable = 'portable';

  final String id;
  final String className;
  final String origin;
  final String version;
  final bool enabled;

  Map<String, Object?> toJson() => {
    'id': id,
    'class': className,
    if (origin.isNotEmpty) 'origin': origin,
    if (version.isNotEmpty) 'version': version,
    'enabled': enabled,
  };

  factory SkillRef.fromJson(Map<String, Object?> json) {
    return SkillRef(
      id: '${json['id'] ?? ''}',
      className: '${json['class'] ?? classApp}',
      origin: '${json['origin'] ?? 'bundled'}',
      version: '${json['version'] ?? ''}',
      enabled: json['enabled'] != false,
    );
  }

  SkillRef copyWith({
    String? id,
    String? className,
    String? origin,
    String? version,
    bool? enabled,
  }) {
    return SkillRef(
      id: id ?? this.id,
      className: className ?? this.className,
      origin: origin ?? this.origin,
      version: version ?? this.version,
      enabled: enabled ?? this.enabled,
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
    this.contextTokens,
    this.accountId = '',
    this.reasoning,
    this.tools = const AgentToolSet(),
    this.capabilities = const [],
    this.permissionOverrides = const {},
    this.extensions = const [],
    this.workspace = WorkspaceSpec.sandbox,
    this.skills = const [],
    this.portableDenylist = const [],
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
  final int? contextTokens;
  final String accountId;
  final ReasoningChoice? reasoning;
  final AgentToolSet tools;

  /// Authoritative assembly units (B-KD 1). Empty only for chat-only personas.
  final List<CapabilityRef> capabilities;
  final Map<String, String> permissionOverrides;
  final List<AgentExtension> extensions;
  final WorkspaceSpec workspace;
  final List<SkillRef> skills;
  final List<String> portableDenylist;
  final bool enabled;
  final String steer;

  /// Empty = unregistered. Not a secret. IM dest after `chat.bot.create`.
  final String serverAccount;

  String get dest => id == kGooseAgentId ? kGooseAgentId : 'agent:$id';

  /// Resolve capabilities for UI / host; derive from legacy tools when empty.
  List<CapabilityRef> resolveCapabilities() {
    if (capabilities.isNotEmpty) {
      return capabilities;
    }
    return capabilitiesFromLegacy(tools, extensions);
  }

  /// Copy with capabilities as source of truth; projects [tools] (+ MCP [extensions]).
  AgentProfile withCapabilities(
    List<CapabilityRef> next, {
    List<AgentExtension>? extensionsOverride,
  }) {
    final projected = projectToolSet(next);
    final mcp = projectExtensions(next);
    return copyWith(
      capabilities: next,
      tools: projected,
      extensions: extensionsOverride ?? mcp,
    );
  }

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
    int? contextTokens,
    String? accountId,
    ReasoningChoice? reasoning,
    AgentToolSet? tools,
    List<CapabilityRef>? capabilities,
    Map<String, String>? permissionOverrides,
    List<AgentExtension>? extensions,
    WorkspaceSpec? workspace,
    List<SkillRef>? skills,
    List<String>? portableDenylist,
    bool? enabled,
    String? steer,
    String? serverAccount,
    String? keyRef,
  }) {
    return AgentProfile(
      id: id,
      displayName: displayName ?? this.displayName,
      aliases: aliases ?? this.aliases,
      providerKind: providerKind ?? this.providerKind,
      baseUrl: baseUrl ?? this.baseUrl,
      model: model ?? this.model,
      keyRef: keyRef ?? this.keyRef,
      systemPrompt: systemPrompt ?? this.systemPrompt,
      mode: mode ?? this.mode,
      maxTurns: maxTurns ?? this.maxTurns,
      thinkingEffort: thinkingEffort ?? this.thinkingEffort,
      contextTokens: contextTokens ?? this.contextTokens,
      accountId: accountId ?? this.accountId,
      reasoning: reasoning ?? this.reasoning,
      tools: tools ?? this.tools,
      capabilities: capabilities ?? this.capabilities,
      permissionOverrides: permissionOverrides ?? this.permissionOverrides,
      extensions: extensions ?? this.extensions,
      workspace: workspace ?? this.workspace,
      skills: skills ?? this.skills,
      portableDenylist: portableDenylist ?? this.portableDenylist,
      enabled: enabled ?? this.enabled,
      steer: steer ?? this.steer,
      serverAccount: serverAccount ?? this.serverAccount,
    );
  }

  /// Disk JSON: no `provider` block (C-KD 1).
  Map<String, Object?> toJson() {
    final caps = resolveCapabilities();
    final projectedTools = projectToolSet(caps);
    final mcpExt = projectExtensions(caps);
    return {
      'id': id,
      'display_name': displayName,
      'aliases': aliases,
      if (accountId.isNotEmpty) 'account_id': accountId,
      'model': {
        'name': model,
        if (reasoning == null && thinkingEffort.isNotEmpty)
          'thinking_effort': thinkingEffort,
        if (contextTokens != null) 'context_tokens': contextTokens,
      },
      if (reasoning != null) 'reasoning': reasoning!.toJson(),
      'system_prompt': systemPrompt,
      'mode': mode,
      'max_turns': maxTurns,
      'capabilities': [for (final c in caps) c.toJson()],
      // One-release ToolSet projection for old readers.
      'tools': projectedTools.toJson(),
      'permissions': {'tools': permissionOverrides},
      'extensions': [for (final e in mcpExt) e.toJson()],
      'workspace': workspace.toJson(),
      'skills': [for (final s in skills) s.toJson()],
      if (portableDenylist.isNotEmpty) 'portable_denylist': portableDenylist,
      'enabled': enabled,
      if (steer.isNotEmpty) 'steer': steer,
      'server_account': serverAccount,
    };
  }

  Map<String, Object?> toHostJson(
    ProviderAccount account, {
    String userAgentsSkills = '',
  }) {
    final json = toJson();
    json['provider'] = {
      'kind': canonicalizeVendorId(account.vendorId),
      'base_url': account.baseUrl,
      'key_ref': '',
    };
    json['workspace'] = workspace.toHostJson();
    if (userAgentsSkills.isNotEmpty) {
      json['user_agents_skills'] = userAgentsSkills;
    }
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
    final workspaceRaw = json['workspace'];
    final skillsRaw = json['skills'];
    final denylistRaw = json['portable_denylist'];
    final capsRaw = json['capabilities'];
    final kind = canonicalizeVendorId(providerMap['kind'] as String? ?? '');
    final tools = toolsRaw is Map
        ? AgentToolSet.fromJson(Map<String, Object?>.from(toolsRaw))
        : const AgentToolSet();
    final extensions = () {
      final raw = json['extensions'];
      if (raw is! List) {
        return const <AgentExtension>[];
      }
      return [
        for (final item in raw)
          if (item is Map)
            AgentExtension.fromJson(Map<String, Object?>.from(item)),
      ];
    }();
    var capabilities = <CapabilityRef>[];
    if (capsRaw is List) {
      for (final item in capsRaw) {
        if (item is Map) {
          capabilities.add(
            CapabilityRef.fromJson(Map<String, Object?>.from(item)),
          );
        }
      }
    }
    if (capabilities.isEmpty) {
      capabilities = capabilitiesFromLegacy(tools, extensions);
    }
    final projectedTools = projectToolSet(capabilities);
    final projectedExt = projectExtensions(capabilities);
    return AgentProfile(
      id: json['id'] as String? ?? kGooseAgentId,
      displayName: json['display_name'] as String? ?? kGooseAgentName,
      aliases: aliasesRaw is List
          ? [for (final a in aliasesRaw) '$a']
          : const [],
      providerKind: kind,
      baseUrl: providerMap['base_url'] as String? ?? '',
      model: modelMap['name'] as String? ?? '',
      keyRef: providerMap['key_ref'] as String? ?? '',
      systemPrompt: migrateIdentityPrompt(
        json['system_prompt'] as String? ?? '',
      ),
      mode: json['mode'] as String? ?? 'smart_approve',
      maxTurns: json['max_turns'] is int ? json['max_turns'] as int : null,
      thinkingEffort: modelMap['thinking_effort'] as String? ?? '',
      contextTokens: modelMap['context_tokens'] is int
          ? modelMap['context_tokens'] as int
          : null,
      accountId: json['account_id'] as String? ?? '',
      reasoning: reasoningRaw is Map
          ? ReasoningChoice.fromJson(Map<String, Object?>.from(reasoningRaw))
          : ReasoningChoice.fromThinkingEffort(
              modelMap['thinking_effort'] as String? ?? '',
            ),
      tools: projectedTools,
      capabilities: capabilities,
      permissionOverrides: overrides,
      extensions: projectedExt,
      workspace: WorkspaceSpec.fromJson(
        workspaceRaw is Map ? Map<String, Object?>.from(workspaceRaw) : null,
      ),
      skills: () {
        if (skillsRaw is! List) {
          return const <SkillRef>[];
        }
        return [
          for (final item in skillsRaw)
            if (item is Map) SkillRef.fromJson(Map<String, Object?>.from(item)),
        ];
      }(),
      portableDenylist: denylistRaw is List
          ? [for (final d in denylistRaw) '$d']
          : const [],
      enabled: json['enabled'] != false,
      steer: json['steer'] as String? ?? '',
      serverAccount: json['server_account'] as String? ?? '',
    );
  }

  static AgentProfile gooseFromSettings(AgentSettings s) {
    const caps = <CapabilityRef>[
      CapabilityRef(kind: CapabilityKinds.imSendMessage),
      CapabilityRef(kind: CapabilityKinds.imSearchContacts),
      CapabilityRef(kind: CapabilityKinds.imSearchMessages),
      CapabilityRef(kind: CapabilityKinds.imGetConversationContext),
      CapabilityRef(kind: CapabilityKinds.imReadClipboard),
      CapabilityRef(kind: CapabilityKinds.imListProfiles),
    ];
    return AgentProfile(
      id: kGooseAgentId,
      displayName: kGooseAgentName,
      aliases: const ['助手'],
      providerKind: canonicalizeVendorId(s.llmBackend),
      baseUrl: s.baseUrl,
      model: s.model,
      keyRef: _kGooseKey,
      accountId: '',
      systemPrompt: '',
      mode: 'smart_approve',
      maxTurns: 16,
      thinkingEffort: s.thinkingEffort,
      contextTokens: defaultContextTokens(s.model),
      reasoning: ReasoningChoice.fromThinkingEffort(s.thinkingEffort),
      capabilities: caps,
      tools: projectToolSet(caps),
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
      var rows = await client.listAgentProfiles();
      if (rows.isEmpty) {
        final imported = await _importPrefsProfiles(client);
        if (imported.isNotEmpty) {
          rows = await client.listAgentProfiles();
        }
      }
      for (final row in rows) {
        final profile = await _fromDto(row);
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
      final decoded = jsonDecode(raw);
      if (decoded is Map) {
        flags = Map<String, Object?>.from(decoded);
      }
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
            jsonEncode({
              'multi_profile': multiProfile,
              'server_identity': serverIdentity,
            }),
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
      for (final p in profiles) await _toDto(p),
    ]);
    await prefs.remove(_kProfiles);
    return profiles;
  }

  Future<AgentProfileDto> _toDto(AgentProfile p) async {
    final client = ref.read(clientPortProvider);
    final blob = await client.specJsonToBlob(jsonEncode(p.toJson()));
    return AgentProfileDto(
      profileId: p.id,
      nickname: p.displayName,
      serverAccount: p.serverAccount,
      bodyJson: '',
      bodyBlob: blob,
      placement: 'local',
      updatedAt: DateTime.now().millisecondsSinceEpoch,
    );
  }

  Future<AgentProfile?> _fromDto(AgentProfileDto row) async {
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
      final msg = e.toString();
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
      await client.upsertAgentProfile(await _toDto(p));
      String? live;
      try {
        live = await ref.read(workspaceAccessProvider).realUserAgentsSkills();
      } catch (_) {
        live = null;
      }
      final previous = await client.getDeviceOverlay(p.id);
      await client.upsertDeviceOverlay(
        DeviceOverlayDto(
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
      await ref.read(clientPortProvider).deleteAgentProfile(id);
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
