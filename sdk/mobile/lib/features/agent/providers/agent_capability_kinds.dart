part of 'agent_profiles.dart';

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
