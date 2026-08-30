import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/app.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/connectivity.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/core/runtime.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/kim_bridge.dart';
import 'package:shared_preferences/shared_preferences.dart';

class FakeAuth implements KimAuthPort {
  FakeAuth({this.session, this.error});

  KimAuthSession? session;
  Object? error;
  int logins = 0;
  int registers = 0;
  int logouts = 0;
  String lastUserAgent = '';
  String lastOrigin = '';

  KimAuthSession _ok() {
    return session ??
        const KimAuthSession(token: 'tok.jwt', exp: 1, account: 'alice');
  }

  Future<KimAuthSession> _run() async {
    if (error != null) {
      throw error!;
    }
    return _ok();
  }

  @override
  Future<KimAuthSession> login({
    required String origin,
    required String userAgent,
    required String account,
    required String password,
  }) async {
    logins += 1;
    lastOrigin = origin;
    lastUserAgent = userAgent;
    return _run();
  }

  @override
  Future<KimAuthSession> register({
    required String origin,
    required String userAgent,
    required String account,
    required String password,
  }) async {
    registers += 1;
    lastOrigin = origin;
    lastUserAgent = userAgent;
    return _run();
  }

  @override
  Future<void> logout({
    required String origin,
    required String userAgent,
    required String token,
  }) async {
    logouts += 1;
    lastUserAgent = userAgent;
  }

  @override
  Future<void> changePassword({
    required String origin,
    required String userAgent,
    required String token,
    required String oldPassword,
    required String newPassword,
  }) async {}

  @override
  String httpOriginFromWs(String wsUrl) => 'http://127.0.0.1:8080';
}

Future<KimRuntime> testRuntime({String token = '', String account = ''}) async {
  SharedPreferences.setMockInitialValues({});
  final tmp = Directory.systemTemp.createTempSync('kim-shell-');
  addTearDown(() {
    if (tmp.existsSync()) {
      tmp.deleteSync(recursive: true);
    }
  });
  final settings = await SettingsStore.load(useSecureStorage: false);
  if (token.isNotEmpty) {
    await settings.saveSession(token: token, account: account);
  }
  return KimRuntime.bootstrap(
    requestNotifications: false,
    paths: KimPaths.forTest(tmp),
    settings: settings,
    connectivity: KimConnectivity.fake(isOnline: true),
    appName: 'KIM',
    version: '1.0.0',
    buildNumber: '1',
  );
}

Future<void> pumpUi(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 800));
}

Future<void> tapKey(WidgetTester tester, Key key) async {
  final finder = find.byKey(key);
  await tester.ensureVisible(finder);
  await tester.tap(finder);
  await pumpUi(tester);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('signed-out shows login form, not the WS shell', (tester) async {
    final runtime = await testRuntime();
    final auth = FakeAuth();
    await tester.pumpWidget(KimApp(runtime: runtime, auth: auth));
    await pumpUi(tester);

    expect(find.text(Copy.loginTitle), findsWidgets);
    expect(find.byKey(const Key('auth-submit')), findsOneWidget);
    expect(find.text('KIM (shell)'), findsNothing);
  });

  testWidgets('invalid account stays on form', (tester) async {
    final runtime = await testRuntime();
    final auth = FakeAuth();
    await tester.pumpWidget(KimApp(runtime: runtime, auth: auth));
    await pumpUi(tester);

    await tester.enterText(find.byType(TextField).first, 'ab');
    await tester.enterText(find.byType(TextField).at(1), 'secret123');
    await tapKey(tester, const Key('auth-submit'));

    expect(find.text(Copy.invalidAccount), findsOneWidget);
    expect(auth.logins, 0);
  });

  testWidgets('successful login opens shell with account', (tester) async {
    final runtime = await testRuntime();
    final auth = FakeAuth();
    await tester.pumpWidget(KimApp(runtime: runtime, auth: auth));
    await pumpUi(tester);

    await tester.enterText(find.byType(TextField).first, 'alice');
    await tester.enterText(find.byType(TextField).at(1), 'secret123');
    await tapKey(tester, const Key('auth-submit'));

    expect(auth.logins, 1);
    expect(auth.lastUserAgent, contains('KIM/1.0.0'));
    expect(find.text('KIM (shell)'), findsOneWidget);
    expect(find.textContaining('alice'), findsWidgets);
    expect(find.text('signin'), findsOneWidget);
    expect(runtime.settings.token, 'tok.jwt');
  });

  testWidgets('http 401 maps to bad credentials', (tester) async {
    final runtime = await testRuntime();
    final auth = FakeAuth(error: Exception('http 401: 账号或密码错误'));
    await tester.pumpWidget(KimApp(runtime: runtime, auth: auth));
    await pumpUi(tester);

    await tester.enterText(find.byType(TextField).first, 'alice');
    await tester.enterText(find.byType(TextField).at(1), 'secret123');
    await tapKey(tester, const Key('auth-submit'));

    expect(find.text(Copy.badCredentials), findsOneWidget);
    expect(find.text('KIM (shell)'), findsNothing);
  });

  testWidgets('register toggle and success', (tester) async {
    final runtime = await testRuntime();
    final auth = FakeAuth(
      session: const KimAuthSession(token: 'reg.jwt', exp: 2, account: 'bob_1'),
    );
    await tester.pumpWidget(KimApp(runtime: runtime, auth: auth));
    await pumpUi(tester);
    await tapKey(tester, const Key('auth-toggle'));
    expect(find.text(Copy.registerTitle), findsOneWidget);

    await tester.enterText(find.byType(TextField).at(0), 'bob_1');
    await tester.enterText(find.byType(TextField).at(1), 'secret123');
    await tester.enterText(find.byType(TextField).at(2), 'secret123');
    await tapKey(tester, const Key('auth-submit'));
    expect(auth.registers, 1);
    expect(find.text('KIM (shell)'), findsOneWidget);
  });

  testWidgets('shell logout returns to login', (tester) async {
    final runtime = await testRuntime(token: 'tok.jwt', account: 'alice');
    final auth = FakeAuth();
    await tester.pumpWidget(KimApp(runtime: runtime, auth: auth));
    await pumpUi(tester);
    expect(find.text('KIM (shell)'), findsOneWidget);

    await tapKey(tester, const Key('logout'));
    expect(auth.logouts, 1);
    expect(find.text(Copy.loginTitle), findsWidgets);
    expect(runtime.settings.token, isEmpty);
  });

  testWidgets('offline banner appears when connectivity is down', (
    tester,
  ) async {
    final runtime = await testRuntime();
    runtime.connectivity.online.value = false;
    await tester.pumpWidget(KimApp(runtime: runtime, auth: FakeAuth()));
    await pumpUi(tester);
    expect(find.textContaining('Offline'), findsOneWidget);
  });
}
