import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/agent/agent_plaza_page.dart';
import 'package:kim_mobile/l10n/app_localizations.dart';

import '../support/harness.dart';

Future<void> _pumpPlaza(
  WidgetTester tester,
  ProviderContainer container,
) async {
  tester.view.physicalSize = const Size(400, 700);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(
        locale: Locale('zh'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: AgentPlazaPage(),
      ),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 50));
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('import action stays in the header, not after the list', (
    tester,
  ) async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    addTearDown(env.container.dispose);
    await _pumpPlaza(tester, env.container);

    final import = find.byKey(const Key('agent-plaza-import'));
    expect(import, findsOneWidget);
    expect(
      find.descendant(of: find.byType(SliverAppBar), matching: import),
      findsOneWidget,
    );
    expect(find.byType(OutlinedButton), findsNothing);
    expect(tester.widget<IconButton>(import).tooltip, '导入技能文件夹');
  });
}
