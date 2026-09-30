part of 'agent_profiles.dart';

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
