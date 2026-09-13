import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/mention.dart';
import 'package:kim_mobile/l10n/app_localizations.dart';
import 'package:kim_mobile/screens/agent/agent_list_page.dart';
import 'package:kim_mobile/state/agent_profiles.dart';

import '../support/harness.dart';

Future<void> _pumpList(WidgetTester tester, ProviderContainer container) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(
        locale: Locale('zh'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: AgentListPage(),
      ),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 50));
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('empty list shows create CTA and no kill-switch tiles', (
    tester,
  ) async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    addTearDown(env.container.dispose);
    await env.container.read(agentProfilesProvider.notifier).ensureLoaded();
    await _pumpList(tester, env.container);
    expect(find.byKey(const Key('agent-create')), findsOneWidget);
    expect(find.text('创建 Agent'), findsWidgets);
    expect(find.text('还没有 Agent'), findsOneWidget);
    expect(find.byType(SwitchListTile), findsNothing);
    expect(find.text('空白'), findsNothing);
    expect(find.text('译者'), findsNothing);
    expect(find.text('编码'), findsNothing);
  });

  testWidgets('goose row has delete and tapping removes the store row', (
    tester,
  ) async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    addTearDown(env.container.dispose);
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.saveProfile(
      const AgentProfile(
        id: kGooseAgentId,
        displayName: kGooseAgentName,
        providerKind: 'openai',
        baseUrl: '',
        model: 'gpt-4o',
        keyRef: 'agent.api_key.goose',
        systemPrompt: '',
      ),
    );
    await _pumpList(tester, env.container);
    expect(find.byKey(const Key('agent-delete-goose')), findsOneWidget);
    expect(find.byType(SwitchListTile), findsNothing);
    await tester.tap(find.byKey(const Key('agent-delete-goose')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    expect(store.goose, isNull);
  });
}
