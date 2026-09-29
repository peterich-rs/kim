import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/agent/workspace.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';
import 'package:kim_mobile/l10n/app_localizations.dart';
import 'package:kim_mobile/features/agent/agent_overview_status.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:shared_preferences/shared_preferences.dart';

AgentProfile _profile(
  String id, {
  WorkspaceSpec? workspace,
  bool fs = false,
  bool fsWrite = false,
  bool bash = false,
}) {
  return AgentProfile(
    id: id,
    displayName: id,
    providerKind: 'openai',
    baseUrl: 'https://api.openai.com/v1',
    model: 'gpt-4o',
    keyRef: 'k',
    systemPrompt: '',
    workspace: workspace ?? WorkspaceSpec.sandbox,
    tools: AgentToolSet(fs: fs, fsWrite: fsWrite, bash: bash),
  );
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    SharedPreferences.setMockInitialValues({});
    FlutterSecureStorage.setMockInitialValues({});
  });

  test('two profiles get isolated sandbox directories', () async {
    final a = await resolveAgentProjectRoot(profile: _profile('p-a'));
    final b = await resolveAgentProjectRoot(profile: _profile('p-b'));
    expect(a.path, isNot(b.path));
    expect(a.path, contains('p-a'));
    expect(b.path, contains('p-b'));
    await Directory(a.path).create(recursive: true);
    await File('${a.path}/secret-a.txt').writeAsString('a');
    expect(File('${b.path}/secret-a.txt').existsSync(), isFalse);
  });

  test('repo workspace uses existing path when present', () async {
    final root = await Directory.systemTemp.createTemp('kim-agent-repo');
    addTearDown(() => root.delete(recursive: true));
    final repo = Directory('${root.path}/repo')..createSync();
    final access = WorkspaceAccess(
      secure: const FlutterSecureStorage(),
      pickDirectoryFallback: () async => repo.path,
    );
    final resolved = await resolveAgentProjectRoot(
      profile: _profile(
        'p-r',
        workspace: WorkspaceSpec(kind: WorkspaceSpec.kindRepo, path: repo.path),
      ),
      access: access,
    );
    expect(resolved.path, repo.absolute.path);
    expect(resolved.invalidRepo, isFalse);
  });

  test('missing repo path falls back with invalidRepo', () async {
    final resolved = await resolveAgentProjectRoot(
      profile: _profile(
        'p-miss',
        workspace: const WorkspaceSpec(
          kind: WorkspaceSpec.kindRepo,
          path: '/no/such/repo/path',
        ),
      ),
      access: WorkspaceAccess(secure: const FlutterSecureStorage()),
    );
    expect(resolved.path, contains('p-miss'));
    expect(resolved.invalidRepo, isTrue);
  });

  test('overview subtitle includes repo kind', () {
    final l10n = lookupAppLocalizations(const Locale('zh'));
    expect(
      agentWorkspaceSubtitle(l10n, _profile('a', fsWrite: true)),
      '应用内沙箱 · 读·写',
    );
    expect(
      agentWorkspaceSubtitle(
        l10n,
        _profile(
          'a',
          workspace: const WorkspaceSpec(
            kind: WorkspaceSpec.kindRepo,
            path: '/x',
          ),
          fs: true,
          bash: true,
        ),
      ),
      '本地仓库 · 只读·终端',
    );
  });
}
