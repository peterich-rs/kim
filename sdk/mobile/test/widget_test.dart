import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/app.dart';
import 'package:kim_mobile/core/connectivity.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/core/runtime.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:shared_preferences/shared_preferences.dart';

Future<KimRuntime> testRuntime() async {
  SharedPreferences.setMockInitialValues({});
  final tmp = Directory.systemTemp.createTempSync('kim-shell-');
  addTearDown(() {
    if (tmp.existsSync()) {
      tmp.deleteSync(recursive: true);
    }
  });
  return KimRuntime.bootstrap(
    requestNotifications: false,
    paths: KimPaths.forTest(tmp),
    settings: await SettingsStore.load(useSecureStorage: false),
    connectivity: KimConnectivity.fake(isOnline: true),
    appName: 'KIM',
    version: '1.0.0',
    buildNumber: '1',
  );
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('shell shows WGateway fields and foundation status', (
    tester,
  ) async {
    final runtime = await testRuntime();
    await tester.pumpWidget(KimApp(runtime: runtime));
    await tester.pumpAndSettle();

    expect(find.text('KIM (shell)'), findsOneWidget);
    expect(find.text('connect'), findsOneWidget);
    expect(find.text('login'), findsOneWidget);
    expect(find.text('talk_to_user'), findsOneWidget);
    expect(find.textContaining('Flutter 3.47.2'), findsOneWidget);
    expect(find.textContaining('app 1.0.0+1'), findsOneWidget);
    expect(find.textContaining('WGateway URL'), findsOneWidget);
  });

  testWidgets('offline banner appears when connectivity is down', (
    tester,
  ) async {
    final runtime = await testRuntime();
    runtime.connectivity.online.value = false;
    await tester.pumpWidget(KimApp(runtime: runtime));
    await tester.pumpAndSettle();
    expect(find.textContaining('Offline'), findsOneWidget);
  });
}
