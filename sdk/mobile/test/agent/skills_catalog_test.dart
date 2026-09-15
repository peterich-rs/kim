import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';

void main() {
  test('kim-memory requires fs write tools', () {
    final profile = AgentProfile(
      id: 'a',
      displayName: 'a',
      providerKind: 'openai',
      baseUrl: 'https://api.openai.com/v1',
      model: 'gpt-4o',
      keyRef: 'k',
      systemPrompt: '',
    );
    expect(missingToolsForAppSkill(profile, 'kim-memory'), ['fs', 'fs_write']);
    final enabled = enableRequiredTools(profile.tools, ['fs', 'fs_write']);
    expect(enabled.fs, isTrue);
    expect(enabled.fsWrite, isTrue);
    expect(
      missingToolsForAppSkill(profile.copyWith(tools: enabled), 'kim-memory'),
      isEmpty,
    );
  });

  test('assignAppSkillToProfile writes capabilities not just tools', () {
    final profile = AgentProfile(
      id: 'a',
      displayName: 'a',
      providerKind: 'openai',
      baseUrl: 'https://api.openai.com/v1',
      model: 'gpt-4o',
      keyRef: 'k',
      systemPrompt: '',
      capabilities: kCreateDefaultCapabilities,
      tools: kCreateDefaultTools,
    );
    const skill = CatalogSkill(id: 'kim-im', name: 'IM', description: 'send');
    final next = assignAppSkillToProfile(
      profile: profile,
      skill: skill,
      enableMissingTools: true,
    );
    expect(next.skills.single.id, 'kim-im');
    expect(
      next.capabilities.any((c) => c.kind == CapabilityKinds.imSearchContacts),
      isTrue,
    );
    expect(next.tools.searchContacts, isTrue);
    expect(next.tools.sendMessage, isTrue);
  });

  test('parseSkillsJson reads app catalog payload', () {
    final skills = parseSkillsJson(
      '{"skills":[{"id":"kim-im","name":"IM","description":"send","class":"app","version":"1"}]}',
    );
    expect(skills, hasLength(1));
    expect(skills.first.id, 'kim-im');
    expect(skills.first.isApp, isTrue);
    expect(skills.first.listDescription, 'send');
  });

  test('listDescription is frontmatter desc, not id or name', () {
    const named = CatalogSkill(
      id: 'git-commit',
      name: 'git-commit',
      description: ' Create Conventional Commits ',
    );
    expect(named.listDescription, 'Create Conventional Commits');
    const empty = CatalogSkill(id: 'x', name: 'x', description: '  ');
    expect(empty.listDescription, isEmpty);
  });
}
