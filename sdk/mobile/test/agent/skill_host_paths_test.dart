import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';

import '../support/harness.dart';

class _FakeAccess extends WorkspaceAccess {
  _FakeAccess(this.skillsPath);

  String? skillsPath;

  @override
  Future<String?> realUserAgentsSkills() async => skillsPath;
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
}
