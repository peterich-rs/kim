import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/app.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/connectivity.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/core/runtime.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/data/conversation_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'support/fake_kim.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('smoke signed-out', (tester) async {
    SharedPreferences.setMockInitialValues({});
    final tmp = Directory.systemTemp.createTempSync('kim-smoke-');
    addTearDown(() => tmp.deleteSync(recursive: true));
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
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.text(Copy.loginTitle), findsWidgets);
  });
}
