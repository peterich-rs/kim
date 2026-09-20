import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_secure_storage/test/test_flutter_secure_storage_platform.dart';
// ignore: depend_on_referenced_packages
import 'package:flutter_secure_storage_platform_interface/flutter_secure_storage_platform_interface.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/bridge/agent_bridge.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';
import 'package:kim_mobile/src/rust/api/types.dart' hide SessionSnapshotDto;
import 'package:shared_preferences/shared_preferences.dart';

import '../support/harness.dart';

class _FakeAccess extends WorkspaceAccess {
  _FakeAccess(this.skillsPath);

  String? skillsPath;

  @override
  Future<String?> realUserAgentsSkills() async => skillsPath;
}

class _ImmediateSession implements AgentSessionPort {
  @override
  Stream<AgentUiEvent> listen() async* {
    yield AgentUiEvent(
      kind: 'assistant_finished',
      operationId: '',
      callId: '',
      name: '',
      delta: '',
      argumentsJson: '',
      outputPreview: '',
      ok: true,
      stopReason: '',
      message: 'ok',
      inputTokens: BigInt.zero,
      outputTokens: BigInt.zero,
      resumedOps: const [],
      recentlyActive: false,
    );
  }

  @override
  Future<String> prompt({required String text}) async => '';

  @override
  Future<String> promptWithContext({
    required String text,
    required String contextJson,
  }) async => '';

  @override
  Future<String> completeTool({
    required String callId,
    required String outputJson,
  }) async => '';

  @override
  Future<String> respondPermission({
    required String callId,
    required String permission,
  }) async => '';

  @override
  Future<void> close() async {}

  @override
  Future<void> abort() async {}

  @override
  Future<void> steer({required String text}) async {}

  @override
  Future<void> reconfigure({required SessionOpenOpts opts}) async {}

  @override
  Future<ResumeReportDto> resume() async =>
      const ResumeReportDto(resumedOps: [], statuses: []);

  @override
  Future<SessionSnapshotDto> snapshot() async => const SessionSnapshotDto(
    busy: false,
    lastOperationId: '',
    phase: '',
    pendingCallIds: [],
  );
}

class _RecordingBridge extends AgentBridge {
  String? lastProfileJson;
  String? lastSqlitePath;
  bool? lastResumeOnOpen;
  String? lastHarnessJson;

  @override
  Future<void> ensure() async {}

  @override
  bool get isReady => true;

  @override
  Future<AgentSessionPort> open({
    required String sqlitePath,
    required String projectRoot,
    required SessionOpenOpts opts,
  }) async {
    lastProfileJson = opts.profileJson;
    lastSqlitePath = sqlitePath;
    lastResumeOnOpen = opts.resumeOnOpen;
    lastHarnessJson = opts.harnessJson;
    return _ImmediateSession();
  }
}

const _account = ProviderAccount(
  id: 'acct-1',
  vendorId: 'openai',
  baseUrl: 'https://api.openai.com/v1',
  keyRef: 'agent.api_key.acct.acct-1',
  displayName: 'OpenAI',
  models: ['gpt-4o'],
);

AgentProfile _profile({WorkspaceSpec workspace = WorkspaceSpec.sandbox}) {
  return AgentProfile(
    id: 'p-1',
    displayName: 'Work',
    providerKind: 'openai',
    baseUrl: 'https://api.openai.com/v1',
    model: 'gpt-4o',
    keyRef: _account.keyRef,
    systemPrompt: '',
    accountId: _account.id,
    workspace: workspace,
    tools: const AgentToolSet(fs: true, fsWrite: true),
  );
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('resolveUserAgentsSkills prefers live over overlay', () {
    expect(
      resolveUserAgentsSkills(live: '/live/skills', overlay: '/old/skills'),
      '/live/skills',
    );
    expect(
      resolveUserAgentsSkills(live: '  ', overlay: '/old/skills'),
      '/old/skills',
    );
    expect(
      resolveUserAgentsSkills(live: null, overlay: '/old/skills'),
      '/old/skills',
    );
    expect(resolveUserAgentsSkills(live: null, overlay: ''), '');
  });

  test('toHostJson writes user_agents_skills only when non-empty', () {
    final profile = _profile();
    final empty = profile.toHostJson(_account);
    expect(empty.containsKey('user_agents_skills'), isFalse);
    final filled = profile.toHostJson(
      _account,
      userAgentsSkills: '/Users/me/.agents/skills',
    );
    expect(filled['user_agents_skills'], '/Users/me/.agents/skills');
  });

  test('save keeps overlay shelf when live path is missing', () async {
    final access = _FakeAccess('/Users/me/.agents/skills');
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [workspaceAccessProvider.overrideWithValue(access)],
    );
    addTearDown(env.container.dispose);
    final accounts = env.container.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    await accounts.upsert(_account);
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.saveEditor(_profile());
    expect(
      env.fake.overlays['p-1']?.userAgentsSkills,
      '/Users/me/.agents/skills',
    );

    access.skillsPath = null;
    await store.saveEditor(
      _profile(
        workspace: const WorkspaceSpec(
          kind: WorkspaceSpec.kindRepo,
          path: '/repo',
        ),
      ),
    );
    final overlay = env.fake.overlays['p-1'];
    expect(overlay?.workspacePath, '/repo');
    expect(overlay?.userAgentsSkills, '/Users/me/.agents/skills');
  });

  test('_promptGoose injects live shelf into session profile JSON', () async {
    await _withDesktopHost(() async {
      final previous = FlutterSecureStoragePlatform.instance;
      FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
        _account.keyRef: 'sk-test',
      });
      addTearDown(() {
        FlutterSecureStoragePlatform.instance = previous;
      });
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      addTearDown(env.container.dispose);
      final store = env.container.read(agentProfilesProvider.notifier);
      final accounts = env.container.read(providerAccountsProvider.notifier);
      await accounts.ensureLoaded();
      await accounts.upsert(_account);
      await store.ensureLoaded();
      await store.saveEditor(_profile());

      final bridge = _RecordingBridge();
      final loop = AgentRunLoop(
        env.fake,
        bridge,
        access: _FakeAccess('/Users/me/.agents/skills'),
      );
      final done = loop.start();
      env.fake.agentRunCtrl.add(
        AgentRunRequestDto(
          dest: 'agent:p-1',
          profileId: 'p-1',
          text: 'hi',
          inReplyTo: 1,
          epoch: BigInt.one,
        ),
      );
      await Future<void>.delayed(const Duration(milliseconds: 80));
      await env.fake.agentRunCtrl.close();
      await done;

      final raw = bridge.lastProfileJson;
      expect(raw, isNotNull);
      final json = jsonDecode(raw!) as Map<String, Object?>;
      expect(json['user_agents_skills'], '/Users/me/.agents/skills');
      expect(bridge.lastResumeOnOpen, isTrue);
      expect(bridge.lastSqlitePath, endsWith('.json'));
      expect(bridge.lastSqlitePath, contains('agent/sessions'));
    });
  });

  test('_promptGoose falls back to overlay when live path is null', () async {
    await _withDesktopHost(() async {
      final previous = FlutterSecureStoragePlatform.instance;
      FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
        _account.keyRef: 'sk-test',
      });
      addTearDown(() {
        FlutterSecureStoragePlatform.instance = previous;
      });
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      addTearDown(env.container.dispose);
      final store = env.container.read(agentProfilesProvider.notifier);
      final accounts = env.container.read(providerAccountsProvider.notifier);
      await accounts.ensureLoaded();
      await accounts.upsert(_account);
      await store.ensureLoaded();
      await store.saveEditor(_profile());
      env.fake.overlays['p-1'] = DeviceOverlayDto(
        profileId: 'p-1',
        workspacePath: '',
        workspaceBookmark: '',
        userAgentsSkills: '/cached/skills',
      );

      final bridge = _RecordingBridge();
      final loop = AgentRunLoop(env.fake, bridge, access: _FakeAccess(null));
      final done = loop.start();
      env.fake.agentRunCtrl.add(
        AgentRunRequestDto(
          dest: 'agent:p-1',
          profileId: 'p-1',
          text: 'hi',
          inReplyTo: 1,
          epoch: BigInt.one,
        ),
      );
      await Future<void>.delayed(const Duration(milliseconds: 80));
      await env.fake.agentRunCtrl.close();
      await done;

      final raw = bridge.lastProfileJson;
      expect(raw, isNotNull);
      final json = jsonDecode(raw!) as Map<String, Object?>;
      expect(json['user_agents_skills'], '/cached/skills');
    });
  });

  test('_promptGoose kill-switch sends enabled false not empty json', () async {
    await _withDesktopHost(() async {
      final previous = FlutterSecureStoragePlatform.instance;
      FlutterSecureStoragePlatform.instance = TestFlutterSecureStoragePlatform({
        _account.keyRef: 'sk-test',
      });
      addTearDown(() {
        FlutterSecureStoragePlatform.instance = previous;
      });
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      addTearDown(env.container.dispose);
      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool('agent.harness_v1', false);
      final store = env.container.read(agentProfilesProvider.notifier);
      final accounts = env.container.read(providerAccountsProvider.notifier);
      await accounts.ensureLoaded();
      await accounts.upsert(_account);
      await store.ensureLoaded();
      await store.saveEditor(_profile());

      final bridge = _RecordingBridge();
      final loop = AgentRunLoop(
        env.fake,
        bridge,
        access: _FakeAccess('/Users/me/.agents/skills'),
      );
      final done = loop.start();
      env.fake.agentRunCtrl.add(
        AgentRunRequestDto(
          dest: 'agent:p-1',
          profileId: 'p-1',
          text: 'hi',
          inReplyTo: 1,
          epoch: BigInt.one,
        ),
      );
      await Future<void>.delayed(const Duration(milliseconds: 80));
      await env.fake.agentRunCtrl.close();
      await done;
      expect(bridge.lastHarnessJson, '{"enabled":false}');
    });
  });
}

Future<void> _withDesktopHost(Future<void> Function() body) async {
  debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
  try {
    await body();
  } finally {
    debugDefaultTargetPlatformOverride = null;
  }
}
