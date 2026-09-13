import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/catalog.dart';
import 'package:kim_mobile/agent_bridge.dart';
import 'package:kim_mobile/l10n/app_localizations.dart';
import 'package:kim_mobile/screens/agent/provider_account_page.dart';
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
        models: ['gpt-4o'],
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

Future<void> _pumpPage(WidgetTester tester, ProviderContainer container) async {
  tester.view.physicalSize = const Size(400, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(
        locale: Locale('zh'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: ProviderAccountPage(),
      ),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 50));
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('create without key does not persist an account', (tester) async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [catalogRepositoryProvider.overrideWithValue(_FakeCatalog())],
    );
    addTearDown(env.container.dispose);
    await env.container.read(providerAccountsProvider.notifier).ensureLoaded();
    await _pumpPage(tester, env.container);
    expect(find.byKey(const Key('provider-key')), findsOneWidget);
    await tester.ensureVisible(find.byKey(const Key('provider-save')));
    await tester.tap(find.byKey(const Key('provider-save')));
    await tester.pump();
    await tester.pump(const Duration(seconds: 5));
    expect(env.container.read(providerAccountsProvider), isEmpty);
  });

  testWidgets('create with key saves and pops the account id', (tester) async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      overrides: [catalogRepositoryProvider.overrideWithValue(_FakeCatalog())],
    );
    addTearDown(env.container.dispose);
    await env.container.read(providerAccountsProvider.notifier).ensureLoaded();
    await _pumpPage(tester, env.container);
    await tester.enterText(find.byKey(const Key('provider-key')), 'sk-test');
    await tester.ensureVisible(find.byKey(const Key('provider-save')));
    await tester.tap(find.byKey(const Key('provider-save')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    final accounts = env.container.read(providerAccountsProvider);
    expect(accounts, isNotEmpty);
    expect(accounts.single.vendorId, 'openai');
  });
}
