part of 'agent_profiles.dart';

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
    this.runtime = 'goose',
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

  /// `goose` (default) or `codex`. Switching starts a new session.
  final String runtime;

  bool get usesCodex => runtime.trim() == 'codex';

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
    String? runtime,
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
      runtime: runtime ?? this.runtime,
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
      if (usesCodex) 'runtime': 'codex',
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
      runtime: _normalizeRuntime(json['runtime'] as String?),
    );
  }

  static String _normalizeRuntime(String? raw) {
    return raw?.trim() == 'codex' ? 'codex' : 'goose';
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
