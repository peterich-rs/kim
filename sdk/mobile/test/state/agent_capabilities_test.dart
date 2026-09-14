import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/state/agent_profiles.dart';

void main() {
  test('CapabilityRef round-trip preserves kind/id/params/enabled', () {
    const ref = CapabilityRef(
      kind: CapabilityKinds.fs,
      id: '',
      params: {'writable': true},
      enabled: true,
    );
    final again = CapabilityRef.fromJson(ref.toJson());
    expect(again, ref);
    expect(again.params['writable'], true);
  });

  test('legacy tools+extensions derive capabilities (Appendix A)', () {
    const tools = AgentToolSet(
      sendMessage: true,
      readClipboard: true,
      fs: true,
      fsWrite: false,
    );
    final caps = capabilitiesFromLegacy(tools, const []);
    expect(caps.map((c) => c.kind).toList(), [
      CapabilityKinds.imSendMessage,
      CapabilityKinds.imReadClipboard,
      CapabilityKinds.fs,
    ]);
    expect(caps.last.params['writable'], false);
    final projected = projectToolSet(caps);
    expect(projected.sendMessage, true);
    expect(projected.readClipboard, true);
    expect(projected.fs, true);
    expect(projected.fsWrite, false);
  });

  test('fromJson derives capabilities when missing; toJson writes them', () {
    final legacy = {
      'id': 'p-1',
      'display_name': 'Work',
      'model': {'name': 'gpt-4o'},
      'system_prompt': '',
      'tools': {
        'send_message': true,
        'read_clipboard': true,
        'fs': true,
        'fs_write': false,
      },
      'extensions': [
        {
          'name': 'github',
          'transport': 'stdio',
          'command': ['npx', 'mcp'],
        },
      ],
    };
    final profile = AgentProfile.fromJson(Map<String, Object?>.from(legacy));
    expect(profile.capabilities, isNotEmpty);
    expect(
      profile.capabilities.any((c) => c.kind == CapabilityKinds.imSendMessage),
      true,
    );
    expect(
      profile.capabilities.any((c) => c.kind == CapabilityKinds.mcp),
      true,
    );
    final json = profile.toJson();
    expect(json['capabilities'], isA<List<Object?>>());
    expect(json['tools'], isA<Map<String, Object?>>());
    final caps = json['capabilities'] as List<Object?>;
    expect(
      caps.any(
        (c) => c is Map<String, Object?> && c['kind'] == 'im.send_message',
      ),
      true,
    );
    expect(
      caps.any((c) => c is Map<String, Object?> && c['kind'] == 'fs'),
      true,
    );
    expect(
      caps.any((c) => c is Map<String, Object?> && c['kind'] == 'mcp'),
      true,
    );
    // No API keys in export surface.
    final encoded = jsonEncode(json);
    expect(encoded.contains('api_key'), false);
  });

  test('draftNew uses default capabilities', () {
    final draft = AgentProfile(
      id: 'p-x',
      displayName: '',
      providerKind: '',
      baseUrl: '',
      model: 'gpt-4o',
      keyRef: '',
      accountId: 'a1',
      systemPrompt: '',
      capabilities: kCreateDefaultCapabilities,
      tools: kCreateDefaultTools,
    );
    expect(draft.tools.sendMessage, true);
    expect(draft.tools.readClipboard, true);
    expect(draft.tools.fs, false);
    expect(draft.capabilities.map((c) => c.kind), [
      CapabilityKinds.imSendMessage,
      CapabilityKinds.imReadClipboard,
    ]);
  });

  test('withCapabilities projects tools and mcp extensions', () {
    final base = AgentProfile(
      id: 'p-1',
      displayName: 'A',
      providerKind: 'openai',
      baseUrl: '',
      model: 'gpt-4o',
      keyRef: '',
      systemPrompt: '',
    );
    final next = base.withCapabilities([
      const CapabilityRef(kind: CapabilityKinds.imSendMessage),
      const CapabilityRef(kind: CapabilityKinds.fs, params: {'writable': true}),
      const CapabilityRef(
        kind: CapabilityKinds.mcp,
        id: 'mcp:gh',
        params: {
          'name': 'gh',
          'command': ['uvx', 'mcp'],
          'transport': 'stdio',
        },
      ),
    ]);
    expect(next.tools.sendMessage, true);
    expect(next.tools.fsWrite, true);
    expect(next.extensions.single.name, 'gh');
  });

  test('withCapabilities empty MCP list wipes leftover extensions', () {
    final base = AgentProfile(
      id: 'p-1',
      displayName: 'A',
      providerKind: 'openai',
      baseUrl: '',
      model: 'gpt-4o',
      keyRef: '',
      systemPrompt: '',
      extensions: const [
        AgentExtension(name: 'gh', command: ['uvx', 'mcp']),
      ],
    );
    final next = base.withCapabilities([
      const CapabilityRef(kind: CapabilityKinds.imSendMessage),
    ]);
    expect(next.extensions, isEmpty);
    expect(next.tools.sendMessage, true);
    final json = next.toJson();
    expect(json['extensions'], isEmpty);
  });

  test('toJson does not keep leftover extensions when caps omit mcp', () {
    final profile = AgentProfile(
      id: 'p-1',
      displayName: 'A',
      providerKind: 'openai',
      baseUrl: '',
      model: 'gpt-4o',
      keyRef: '',
      systemPrompt: '',
      capabilities: const [CapabilityRef(kind: CapabilityKinds.imSendMessage)],
      tools: const AgentToolSet(sendMessage: true),
      extensions: const [
        AgentExtension(name: 'stale', command: ['npx', 'mcp']),
      ],
    );
    final json = profile.toJson();
    expect(json['extensions'], isEmpty);
  });

  test(
    'fromJson projects extensions from capabilities; migrates laundry identity',
    () {
      final profile = AgentProfile.fromJson({
        'id': 'p-1',
        'display_name': 'Work',
        'model': {'name': 'gpt-4o'},
        'system_prompt': kLegacyToolLaundryIdentity,
        'capabilities': [
          {'kind': CapabilityKinds.imSendMessage, 'enabled': true},
        ],
        'extensions': [
          {
            'name': 'stale',
            'transport': 'stdio',
            'command': ['npx', 'mcp'],
          },
        ],
      });
      expect(profile.systemPrompt, isEmpty);
      expect(profile.extensions, isEmpty);
      expect(profile.capabilities.single.kind, CapabilityKinds.imSendMessage);
    },
  );

  test('mergeCapsWithMcpLines keeps IM upsert and field MCP', () {
    final caps = [
      const CapabilityRef(kind: CapabilityKinds.imSendMessage),
      const CapabilityRef(
        kind: CapabilityKinds.mcp,
        id: 'mcp:old',
        params: {
          'name': 'old',
          'command': ['old'],
        },
      ),
    ];
    final merged = mergeCapsWithMcpLines(caps, 'gh uvx mcp');
    expect(merged.any((c) => c.kind == CapabilityKinds.imSendMessage), true);
    expect(merged.where((c) => c.kind == CapabilityKinds.mcp), hasLength(1));
    expect(merged.last.params['name'], 'gh');
    expect(mcpTextFromCaps(merged), 'gh uvx mcp');
    expect(mcpCapsFromText('').isEmpty, true);
  });
}
