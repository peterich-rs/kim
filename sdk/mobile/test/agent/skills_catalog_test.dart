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

  test('skillShelfOf maps origin to chips', () {
    expect(
      const CatalogSkill(
        id: 'kim-im',
        name: 'IM',
        description: '',
        origin: 'bundled',
        className: 'app',
      ).shelf,
      SkillShelf.internal,
    );
    expect(
      const CatalogSkill(
        id: 'kim-im',
        name: 'IM',
        description: '',
        origin: 'cache',
        className: 'app',
      ).shelf,
      SkillShelf.download,
    );
    expect(
      const CatalogSkill(
        id: 'git-commit',
        name: 'git',
        description: '',
        origin: 'global',
        className: 'portable',
      ).shelf,
      SkillShelf.global,
    );
    expect(
      const CatalogSkill(
        id: 'git-commit',
        name: 'git',
        description: '',
        origin: 'project',
        className: 'portable',
      ).shelf,
      SkillShelf.project,
    );
  });

  test('mergeSkillCatalogs prefers app over portable id', () {
    const app = CatalogSkill(
      id: 'kim-im',
      name: 'IM',
      description: 'app',
      className: 'app',
      origin: 'bundled',
    );
    const portable = CatalogSkill(
      id: 'git-commit',
      name: 'git',
      description: 'proj',
      className: 'portable',
      origin: 'project',
    );
    const clash = CatalogSkill(
      id: 'kim-im',
      name: 'IM portable',
      description: 'no',
      className: 'portable',
      origin: 'global',
    );
    final merged = mergeSkillCatalogs(
      app: const [app],
      portable: const [portable, clash],
    );
    expect(merged.map((s) => s.id).toSet(), {'git-commit', 'kim-im'});
    expect(
      merged.firstWhere((s) => s.id == 'kim-im').shelf,
      SkillShelf.internal,
    );
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
