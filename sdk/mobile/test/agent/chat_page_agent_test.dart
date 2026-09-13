import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/mention.dart';
import 'package:kim_mobile/l10n/app_localizations.dart';
import 'package:kim_mobile/screens/chat/chat_page.dart';
import 'package:kim_mobile/state/agent_profiles.dart';
import 'package:kim_mobile/state/provider_accounts.dart';

import '../support/harness.dart';

Future<void> _pumpChat(
  WidgetTester tester,
  ProviderContainer container,
  String id,
) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        locale: const Locale('zh'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: ChatPage(id: id),
      ),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 50));
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('desktop live goose is not read-only while store loads', (
    tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    try {
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      addTearDown(env.container.dispose);
      final store = env.container.read(agentProfilesProvider.notifier);
      await store.ensureLoaded();
      await store.saveProfile(
        const AgentProfile(
          id: kGooseAgentId,
          displayName: '助手',
          providerKind: 'openai',
          baseUrl: '',
          model: 'gpt-4o',
          keyRef: '',
          systemPrompt: '',
        ),
      );
      await _pumpChat(tester, env.container, 'goose');
      expect(find.byKey(const Key('chat-composer')), findsOneWidget);
      expect(find.byKey(const Key('agent-deleted-readonly')), findsNothing);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });

  testWidgets('desktop deleted goose thread is read-only', (tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    try {
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      addTearDown(env.container.dispose);
      await env.container.read(agentProfilesProvider.notifier).ensureLoaded();
      await _pumpChat(tester, env.container, 'goose');
      expect(find.byKey(const Key('chat-composer')), findsNothing);
      expect(find.byKey(const Key('agent-deleted-readonly')), findsOneWidget);
      expect(find.text('助手'), findsNothing);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });

  testWidgets('desktop orphan b_* thread is read-only', (tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    try {
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      addTearDown(env.container.dispose);
      await env.container.read(agentProfilesProvider.notifier).ensureLoaded();
      await _pumpChat(tester, env.container, 'b_orphan');
      expect(find.byKey(const Key('chat-composer')), findsNothing);
      expect(find.byKey(const Key('agent-deleted-readonly')), findsOneWidget);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });

  testWidgets('phone live b_* keeps the composer', (tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
    try {
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      addTearDown(env.container.dispose);
      await _pumpChat(tester, env.container, 'b_live');
      expect(find.byKey(const Key('chat-composer')), findsOneWidget);
      expect(find.byKey(const Key('agent-deleted-readonly')), findsNothing);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });

  testWidgets('desktop disabled profile still has composer on b_*', (
    tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    try {
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      addTearDown(env.container.dispose);
      final accounts = env.container.read(providerAccountsProvider.notifier);
      await accounts.ensureLoaded();
      await accounts.upsert(
        const ProviderAccount(
          id: 'acct-1',
          vendorId: 'openai',
          baseUrl: 'https://api.openai.com/v1',
          keyRef: 'agent.api_key.acct.acct-1',
          models: ['gpt-4o'],
        ),
      );
      final store = env.container.read(agentProfilesProvider.notifier);
      await store.ensureLoaded();
      await store.saveProfile(
        const AgentProfile(
          id: 'p-1',
          displayName: 'Work',
          providerKind: 'openai',
          baseUrl: '',
          model: 'gpt-4o',
          keyRef: '',
          accountId: 'acct-1',
          systemPrompt: '',
          enabled: false,
          serverAccount: 'b_bot',
        ),
      );
      await _pumpChat(tester, env.container, 'b_bot');
      expect(find.byKey(const Key('chat-composer')), findsOneWidget);
      expect(find.byKey(const Key('agent-deleted-readonly')), findsNothing);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });
}
