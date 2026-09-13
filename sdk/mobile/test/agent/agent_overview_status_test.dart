import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/l10n/app_localizations.dart';
import 'package:kim_mobile/screens/agent/agent_overview_status.dart';
import 'package:kim_mobile/state/agent_profiles.dart';

AgentProfile _profile({
  bool fs = false,
  bool fsWrite = false,
  bool bash = false,
  int mcp = 0,
  List<SkillRef> skills = const [],
}) {
  return AgentProfile(
    id: 'a1',
    displayName: 'Work',
    providerKind: 'openai',
    baseUrl: 'https://api.openai.com/v1',
    model: 'gpt-4o',
    keyRef: 'k',
    systemPrompt: '',
    tools: AgentToolSet(fs: fs, fsWrite: fsWrite, bash: bash),
    skills: skills,
    extensions: [
      for (var i = 0; i < mcp; i++)
        AgentExtension(name: 'm$i', command: const ['cmd']),
    ],
  );
}

void main() {
  final l10n = lookupAppLocalizations(const Locale('zh'));

  test('workspace subtitle reflects fs write and bash', () {
    expect(
      agentWorkspaceSubtitle(l10n, _profile()),
      '应用内沙箱 · 未开读写',
    );
    expect(
      agentWorkspaceSubtitle(l10n, _profile(fs: true)),
      '应用内沙箱 · 只读',
    );
    expect(
      agentWorkspaceSubtitle(l10n, _profile(fsWrite: true)),
      '应用内沙箱 · 读·写',
    );
    expect(
      agentWorkspaceSubtitle(l10n, _profile(bash: true)),
      '应用内沙箱 · 终端',
    );
    expect(
      agentWorkspaceSubtitle(l10n, _profile(fs: true, bash: true)),
      '应用内沙箱 · 只读·终端',
    );
    expect(
      agentWorkspaceSubtitle(l10n, _profile(fsWrite: true, bash: true)),
      '应用内沙箱 · 读·写·终端',
    );
  });

  test('skills subtitle lists assigned app skills', () {
    expect(agentSkillsSubtitle(l10n, _profile()), '未分配');
    expect(
      agentSkillsSubtitle(
        l10n,
        _profile(skills: const [SkillRef(id: 'kim-im'), SkillRef(id: 'kim-memory')]),
      ),
      'kim-im · kim-memory',
    );
  });

  test('tools subtitle counts mcp', () {
    expect(agentToolsSubtitle(l10n, _profile()), '默认 · 无 MCP');
    expect(agentToolsSubtitle(l10n, _profile(mcp: 2)), '默认 · MCP 2');
  });
}
