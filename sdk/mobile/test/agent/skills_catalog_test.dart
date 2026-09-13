import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/skills_catalog.dart';
import 'package:kim_mobile/state/agent_profiles.dart';

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
    expect(
      missingToolsForAppSkill(profile, 'kim-memory'),
      ['fs', 'fs_write'],
    );
    final enabled = enableRequiredTools(profile.tools, ['fs', 'fs_write']);
    expect(enabled.fs, isTrue);
    expect(enabled.fsWrite, isTrue);
    expect(missingToolsForAppSkill(profile.copyWith(tools: enabled), 'kim-memory'), isEmpty);
  });

  test('parseSkillsJson reads app catalog payload', () {
    final skills = parseSkillsJson(
      '{"skills":[{"id":"kim-im","name":"IM","description":"send","class":"app","version":"1"}]}',
    );
    expect(skills, hasLength(1));
    expect(skills.first.id, 'kim-im');
    expect(skills.first.isApp, isTrue);
  });
}
