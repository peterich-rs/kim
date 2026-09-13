import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_secure_storage/test/test_flutter_secure_storage_platform.dart';
import 'package:flutter_secure_storage_platform_interface/flutter_secure_storage_platform_interface.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/workspace.dart';
import 'package:kim_mobile/agent/workspace_access.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/l10n/app_localizations.dart';
import 'package:kim_mobile/screens/agent/agent_overview_status.dart';
import 'package:kim_mobile/state/agent_profiles.dart';
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
  late FlutterSecureStoragePlatform previousPlatform;

  setUp(() {
    SharedPreferences.setMockInitialValues({});
    previousPlatform = FlutterSecureStoragePlatform.instance;
    FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform(
      {},
    );
  });

  tearDown(() {
    FlutterSecureStoragePlatform.instance = previousPlatform;
  });

  test('ensureAgentDirs no longer seeds shared .agents/skills', () async {
    final root = await Directory.systemTemp.createTemp('kim-agent-paths');
    addTearDown(() => root.delete(recursive: true));
    final paths = KimPaths.forTest(root);
    await paths.ensureAgentDirs();
    expect(paths.agentSessions.existsSync(), isTrue);
    expect(paths.agentWorkspaces.existsSync(), isTrue);
    expect(
      Directory('${paths.agentWorkspace.path}/.agents/skills').existsSync(),
      isFalse,
    );
  });

  test('two profiles get isolated sandbox directories', () async {
    final root = await Directory.systemTemp.createTemp('kim-agent-iso');
    addTearDown(() => root.delete(recursive: true));
    final paths = KimPaths.forTest(root);
    final a = await resolveAgentProjectRoot(
      profile: _profile('p-a'),
      paths: paths,
    );
    final b = await resolveAgentProjectRoot(
      profile: _profile('p-b'),
      paths: paths,
    );
    expect(a.path, isNot(b.path));
    expect(a.path, isNot(paths.agentWorkspace.path));
    await File('${a.path}/secret-a.txt').writeAsString('a');
    expect(File('${b.path}/secret-a.txt').existsSync(), isFalse);
  });

  test('repo workspace uses existing path when present', () async {
    final root = await Directory.systemTemp.createTemp('kim-agent-repo');
    addTearDown(() => root.delete(recursive: true));
    final paths = KimPaths.forTest(root);
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
      paths: paths,
      access: access,
    );
    expect(resolved.path, repo.absolute.path);
    expect(resolved.invalidRepo, isFalse);
  });

  test('missing repo path falls back with invalidRepo', () async {
    final root = await Directory.systemTemp.createTemp('kim-agent-miss');
    addTearDown(() => root.delete(recursive: true));
    final paths = KimPaths.forTest(root);
    final resolved = await resolveAgentProjectRoot(
      profile: _profile(
        'p-miss',
        workspace: const WorkspaceSpec(
          kind: WorkspaceSpec.kindRepo,
          path: '/no/such/repo/path',
        ),
      ),
      paths: paths,
      access: WorkspaceAccess(secure: const FlutterSecureStorage()),
    );
    expect(resolved.path, paths.sandboxFor('p-miss').path);
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
