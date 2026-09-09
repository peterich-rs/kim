import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:kim_mobile/app.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/widgets/kim_dock.dart';
import 'package:kim_mobile/core/connectivity.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/core/runtime.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/data/conversation_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../test/support/fake_kim.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('login golden path opens chats', (tester) async {
    SharedPreferences.setMockInitialValues({});
    final tmp = Directory.systemTemp.createTempSync('kim-it-');
    addTearDown(() {
      if (tmp.existsSync()) {
        tmp.deleteSync(recursive: true);
      }
    });
    final settings = await SettingsStore.load(useSecureStorage: false);
    final runtime = await KimRuntime.bootstrap(
      requestNotifications: false,
      paths: KimPaths.forTest(tmp),
      settings: settings,
      connectivity: KimConnectivity.fake(isOnline: true),
      appName: 'KIM',
      version: '1.0.0',
      buildNumber: '1',
    );
    final fake = FakeKim();
    final store = ConversationStore.memory();
    addTearDown(store.close);

    await tester.pumpWidget(
      KimAppHost(runtime: runtime, auth: fake, client: fake, store: store),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.text(Copy.loginTitle), findsWidgets);
    await tester.enterText(find.byType(TextField).first, 'alice');
    await tester.enterText(find.byType(TextField).at(1), 'secret123');
    await tester.tap(find.byKey(const Key('auth-submit')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 800));

    expect(fake.logins, 1);
    expect(find.byType(KimDock), findsOneWidget);
  });
}
