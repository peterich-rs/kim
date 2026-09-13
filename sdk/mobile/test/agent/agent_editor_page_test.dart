import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/catalog.dart';
import 'package:kim_mobile/agent_bridge.dart';
import 'package:kim_mobile/l10n/app_localizations.dart';
import 'package:kim_mobile/screens/agent/agent_settings_page.dart';
import 'package:kim_mobile/state/agent_profiles.dart';
import 'package:kim_mobile/state/provider_accounts.dart';

import '../support/harness.dart';

class _FakeCatalog extends CatalogRepository {
  _FakeCatalog() : super(AgentBridge());

  @override
  Future<List<VendorSummaryDto>> ensureVendors() async {
    vendors = const [
      VendorSummaryDto(
        id: 'openai',
        displayName: 'OpenAI',
        group: 'primary',
        sortRank: 0,
        defaultBaseUrl: 'https://api.openai.com/v1',
        defaultModel: 'gpt-4o',
        models: ['gpt-4o', 'gpt-4.1'],
      ),
      VendorSummaryDto(
        id: 'anthropic',
        displayName: 'Anthropic',
        group: 'primary',
        sortRank: 1,
        defaultBaseUrl: 'https://api.anthropic.com',
        defaultModel: 'claude-sonnet-4-5',
        models: ['claude-sonnet-4-5'],
      ),
    ];
    return vendors;
  }

  @override
  Future<ReasoningSurfaceDto> surface({
    required String vendor,
    required String model,
  }) async {
    return const ReasoningSurfaceDto(kind: 'none');
  }

  @override
  Future<CatalogValidateResult> validate({
    required String vendor,
    required String model,
    required ReasoningChoice choice,
  }) async {
    return CatalogValidateResult(choice: choice);
  }
}

Future<void> _pumpEditor(
  WidgetTester tester,
  ProviderContainer container, {
  String? profileId,
}) async {
  tester.view.physicalSize = const Size(800, 2400);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        locale: const Locale('zh'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: AgentEditorPage(profileId: profileId),
      ),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 50));
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('create and edit show prompt with default placeholder', (
    tester,
  ) async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [catalogRepositoryProvider.overrideWithValue(_FakeCatalog())],
    );
    addTearDown(env.container.dispose);
    final accounts = env.container.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    await accounts.upsert(
      const ProviderAccount(
        id: 'acct-1',
        vendorId: 'openai',
        baseUrl: 'https://api.openai.com/v1',
        keyRef: 'agent.api_key.acct.acct-1',
        displayName: 'OpenAI',
        models: ['gpt-4o', 'gpt-4.1'],
      ),
    );
    await _pumpEditor(tester, env.container);
    var prompt = tester.widget<TextField>(
      find.byKey(const Key('agent-prompt')),
    );
    expect(prompt.decoration?.hintText, kDefaultSystemPrompt);
    expect(find.text('留空则使用该默认'), findsOneWidget);
    expect(find.text('高级'), findsNothing);
    expect(find.byKey(const Key('agent-inline-key')), findsNothing);
    expect(find.byKey(const Key('agent-add-provider')), findsNothing);

    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.saveEditor(
      store
          .draftNew(accountId: 'acct-1', model: 'gpt-4o')
          .copyWith(displayName: 'Work'),
    );
    final id = env.container.read(agentProfilesProvider).single.id;
    await _pumpEditor(tester, env.container, profileId: id);
    prompt = tester.widget<TextField>(find.byKey(const Key('agent-prompt')));
    expect(prompt.decoration?.hintText, kDefaultSystemPrompt);
    expect(find.text('高级'), findsOneWidget);
  });

  testWidgets('model picker only lists the selected provider models', (
    tester,
  ) async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [catalogRepositoryProvider.overrideWithValue(_FakeCatalog())],
    );
    addTearDown(env.container.dispose);
    final accounts = env.container.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    await accounts.upsert(
      const ProviderAccount(
        id: 'acct-1',
        vendorId: 'openai',
        baseUrl: 'https://api.openai.com/v1',
        keyRef: 'agent.api_key.acct.acct-1',
        displayName: 'OpenAI work',
        models: ['gpt-4o', 'gpt-4.1'],
      ),
    );
    await accounts.upsert(
      const ProviderAccount(
        id: 'acct-2',
        vendorId: 'anthropic',
        baseUrl: 'https://api.anthropic.com',
        keyRef: 'agent.api_key.acct.acct-2',
        displayName: 'Claude',
        models: ['claude-sonnet-4-5'],
      ),
    );
    await _pumpEditor(tester, env.container);
    await tester.tap(find.byKey(const Key('agent-model')));
    await tester.pumpAndSettle();
    expect(find.text('gpt-4o'), findsWidgets);
    expect(find.text('gpt-4.1'), findsOneWidget);
    expect(find.text('claude-sonnet-4-5'), findsNothing);
    expect(find.text('其他…'), findsWidgets);
  });

  testWidgets('save agent without key succeeds', (tester) async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [catalogRepositoryProvider.overrideWithValue(_FakeCatalog())],
    );
    addTearDown(env.container.dispose);
    final accounts = env.container.read(providerAccountsProvider.notifier);
    await accounts.ensureLoaded();
    await accounts.upsert(
      const ProviderAccount(
        id: 'acct-1',
        vendorId: 'openai',
        baseUrl: 'https://api.openai.com/v1',
        keyRef: 'agent.api_key.acct.acct-1',
        displayName: 'OpenAI',
        models: ['gpt-4o'],
      ),
    );
    await _pumpEditor(tester, env.container);
    await tester.enterText(find.byKey(const Key('agent-name')), 'Work');
    await tester.ensureVisible(find.byKey(const Key('agent-save')));
    await tester.tap(find.byKey(const Key('agent-save')));
    await tester.pump();
    await tester.pump(const Duration(seconds: 3));
    expect(
      env.container
          .read(agentProfilesProvider)
          .any((p) => p.displayName == 'Work'),
      isTrue,
    );
  });

  testWidgets('create without provider shows add-account CTA and save fails', (
    tester,
  ) async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [catalogRepositoryProvider.overrideWithValue(_FakeCatalog())],
    );
    addTearDown(env.container.dispose);
    await _pumpEditor(tester, env.container);
    expect(find.byKey(const Key('agent-inline-key')), findsNothing);
    expect(find.byKey(const Key('agent-add-provider')), findsOneWidget);
    expect(find.text('还没有厂商账号'), findsOneWidget);
    await tester.enterText(find.byKey(const Key('agent-name')), 'Work');
    await tester.ensureVisible(find.byKey(const Key('agent-save')));
    await tester.tap(find.byKey(const Key('agent-save')));
    await tester.pump();
    await tester.pump(const Duration(seconds: 5));
    expect(env.container.read(agentProfilesProvider), isEmpty);
  });
}
