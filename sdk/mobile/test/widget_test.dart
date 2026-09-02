import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/app.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/connectivity.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/core/runtime.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/data/conversation_store.dart';
import 'package:kim_mobile/kim_bridge.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/widgets/kim_avatar.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'support/fake_kim.dart';

Future<({KimRuntime runtime, ConversationStore store})> testRuntime({
  String token = '',
  String account = '',
}) async {
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
  final runtime = await KimRuntime.bootstrap(
    requestNotifications: false,
    paths: KimPaths.forTest(tmp),
    settings: settings,
    connectivity: KimConnectivity.fake(isOnline: true),
    appName: 'KIM',
    version: '1.0.0',
    buildNumber: '1',
  );
  final store = ConversationStore.memory();
  addTearDown(store.close);
  return (runtime: runtime, store: store);
}

Widget host(KimRuntime runtime, FakeKim fake, ConversationStore store) {
  return KimAppHost(runtime: runtime, auth: fake, client: fake, store: store);
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

  testWidgets('signed-out shows login form, not the chat list', (tester) async {
    final env = await testRuntime();
    final fake = FakeKim();
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    expect(find.text(Copy.loginTitle), findsWidgets);
    expect(find.byKey(const Key('auth-submit')), findsOneWidget);
    expect(find.text(Copy.conversations), findsNothing);
  });

  testWidgets('invalid account stays on form', (tester) async {
    final env = await testRuntime();
    final fake = FakeKim();
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    await tester.enterText(find.byType(TextField).first, 'ab');
    await tester.enterText(find.byType(TextField).at(1), 'secret123');
    await tapKey(tester, const Key('auth-submit'));

    expect(find.text(Copy.invalidAccount), findsOneWidget);
    expect(fake.logins, 0);
  });

  testWidgets('successful login opens conversation list', (tester) async {
    final env = await testRuntime();
    final fake = FakeKim();
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    await tester.enterText(find.byType(TextField).first, 'alice');
    await tester.enterText(find.byType(TextField).at(1), 'secret123');
    await tapKey(tester, const Key('auth-submit'));

    expect(fake.logins, 1);
    expect(fake.lastUserAgent, contains('KIM/1.0.0'));
    expect(find.text(Copy.conversations), findsWidgets);
    expect(find.text(Copy.noConversations), findsOneWidget);
    expect(env.runtime.settings.token, 'tok.jwt');
    expect(fake.connects, greaterThan(0));
  });

  testWidgets('http 401 maps to bad credentials', (tester) async {
    final env = await testRuntime();
    final fake = FakeKim(error: Exception('http 401: 账号或密码错误'));
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    await tester.enterText(find.byType(TextField).first, 'alice');
    await tester.enterText(find.byType(TextField).at(1), 'secret123');
    await tapKey(tester, const Key('auth-submit'));

    expect(find.text(Copy.badCredentials), findsOneWidget);
    expect(find.text(Copy.conversations), findsNothing);
  });

  testWidgets('register toggle and success', (tester) async {
    final env = await testRuntime();
    final fake = FakeKim(
      session: const KimAuthSession(token: 'reg.jwt', exp: 2, account: 'bob_1'),
    );
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);
    await tapKey(tester, const Key('auth-toggle'));
    expect(find.text(Copy.registerTitle), findsOneWidget);

    await tester.enterText(find.byType(TextField).at(0), 'bob_1');
    await tester.enterText(find.byType(TextField).at(1), 'secret123');
    await tester.enterText(find.byType(TextField).at(2), 'secret123');
    await tapKey(tester, const Key('auth-submit'));
    expect(fake.registers, 1);
    expect(find.text(Copy.conversations), findsWidgets);
  });

  testWidgets('me tab logout returns to login', (tester) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);
    expect(find.text(Copy.conversations), findsWidgets);

    await tester.tap(find.byIcon(LucideIcons.user));
    await pumpUi(tester);
    expect(find.byKey(const Key('logout')), findsOneWidget);
    await tester.ensureVisible(find.byKey(const Key('logout')));
    await tester.tap(find.text(Copy.logout));
    await pumpUi(tester);
    expect(fake.logouts, 1);
    expect(find.text(Copy.loginTitle), findsWidgets);
    expect(env.runtime.settings.token, isEmpty);
  });

  testWidgets('offline banner appears when connectivity is down', (
    tester,
  ) async {
    final env = await testRuntime();
    env.runtime.connectivity.online.value = false;
    await tester.pumpWidget(host(env.runtime, FakeKim(), env.store));
    await pumpUi(tester);
    expect(find.textContaining(Copy.offlineBanner), findsOneWidget);
  });

  testWidgets('radio drop after login shows the offline banner', (
    tester,
  ) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    await tester.pumpWidget(host(env.runtime, FakeKim(), env.store));
    await pumpUi(tester);
    expect(find.text(Copy.conversations), findsWidgets);
    expect(find.textContaining(Copy.offlineBanner), findsNothing);

    env.runtime.connectivity.online.value = false;
    await tester.pump();
    expect(find.textContaining(Copy.offlineBanner), findsOneWidget);
  });

  testWidgets('contacts tab requires adding friends', (tester) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);
    await tester.tap(find.text(Copy.contacts).last);
    await pumpUi(tester);
    expect(find.text(Copy.noFriends), findsOneWidget);
    expect(find.text(Copy.noFriendsHint), findsOneWidget);
    expect(find.text(Copy.addFriend), findsWidgets);
  });

  testWidgets('new chat lists friends from the server graph', (tester) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);
    await tester.tap(find.byKey(const Key('compose-chat')));
    await pumpUi(tester);
    expect(find.text('Bobby'), findsOneWidget);
    expect(find.text('@bob'), findsOneWidget);
  });

  testWidgets('incoming talk appears on the conversation list', (tester) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    fake.emitTalk(
      dest: 'bob',
      sender: 'bob',
      body: 'hello from bob',
      sendTime: 1788077118498491646,
    );
    await pumpUi(tester);

    expect(find.text('hello from bob'), findsOneWidget);
    expect(fake.acks, greaterThan(0));
  });

  testWidgets('leaving a chat does not use ref after dispose', (tester) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    fake.emitTalk(dest: 'bob', sender: 'bob', body: 'hello from bob');
    await pumpUi(tester);
    await tester.tap(find.text('hello from bob'));
    await pumpUi(tester);
    expect(find.byType(BackButton), findsOneWidget);

    await tester.tap(find.byType(BackButton));
    await pumpUi(tester);
    expect(find.text(Copy.conversations), findsWidgets);
  });

  testWidgets('chat thread groups peer rows and right-aligns own bubbles', (
    tester,
  ) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    fake.emitTalk(dest: 'bob', sender: 'bob', body: 'hello from bob');
    await pumpUi(tester);
    await tester.tap(find.text('hello from bob'));
    await pumpUi(tester);

    expect(find.text('hello from bob'), findsOneWidget);
    expect(find.text(Copy.you), findsNothing);
    expect(find.text('Bobby'), findsWidgets);
  });

  testWidgets('chat app bar is two-line title plus status', (tester) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    fake.emitTalk(dest: 'bob', sender: 'bob', body: 'hello from bob');
    await pumpUi(tester);
    await tester.tap(find.text('hello from bob'));
    await pumpUi(tester);

    final appBar = find.byType(AppBar);
    expect(appBar, findsOneWidget);
    expect(
      find.descendant(of: appBar, matching: find.byType(KimAvatar)),
      findsNothing,
    );
    expect(
      find.descendant(of: appBar, matching: find.byType(Hero)),
      findsNothing,
    );
    expect(
      find.descendant(of: appBar, matching: find.text('Bobby')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: appBar, matching: find.text(Copy.online)),
      findsOneWidget,
    );
    expect(find.byType(BackButton), findsOneWidget);
  });

  testWidgets('chat composer sits below the message list', (tester) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    fake.emitTalk(dest: 'bob', sender: 'bob', body: 'hello from bob');
    await pumpUi(tester);
    await tester.tap(find.text('hello from bob'));
    await pumpUi(tester);

    final composer = tester.getRect(find.byKey(const Key('chat-composer')));
    final message = tester.getRect(find.text('hello from bob'));
    expect(composer.top, greaterThan(message.bottom));
    expect(
      composer.bottom,
      closeTo(
        tester.view.physicalSize.height / tester.view.devicePixelRatio,
        8,
      ),
    );
  });

  testWidgets('swiping from the left of a chat returns to the list', (
    tester,
  ) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    fake.emitTalk(dest: 'bob', sender: 'bob', body: 'hello from bob');
    await pumpUi(tester);
    await tester.tap(find.text('hello from bob'));
    await pumpUi(tester);
    expect(find.byKey(const Key('chat-composer')), findsOneWidget);

    final gesture = await tester.startGesture(const Offset(40, 400));
    await tester.pump();
    await gesture.moveBy(const Offset(20, 0));
    await tester.pump();
    await gesture.moveBy(const Offset(480, 0));
    await tester.pump();
    await gesture.up();
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('chat-composer')), findsNothing);
    expect(find.text(Copy.conversations), findsWidgets);
  });

  testWidgets('long-pressing a message offers copy', (tester) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    fake.emitTalk(dest: 'bob', sender: 'bob', body: 'hello from bob');
    await pumpUi(tester);
    await tester.tap(find.text('hello from bob'));
    await pumpUi(tester);

    await tester.longPress(find.text('hello from bob'));
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.text(Copy.copy), findsOneWidget);
    expect(find.text(Copy.quote), findsOneWidget);
  });

  testWidgets('auth toggle keeps typed account without swapping routes', (
    tester,
  ) async {
    final env = await testRuntime();
    await tester.pumpWidget(host(env.runtime, FakeKim(), env.store));
    await pumpUi(tester);

    await tester.enterText(find.byType(TextField).first, 'alice');
    await tapKey(tester, const Key('auth-toggle'));
    expect(find.text(Copy.registerTitle), findsOneWidget);
    expect(find.text('alice'), findsOneWidget);
    expect(find.text(Copy.confirmPassword), findsOneWidget);
  });

  testWidgets('failed send shows retry on the message row', (tester) async {
    final env = await testRuntime(token: 'tok.jwt', account: 'alice');
    final fake = FakeKim();
    fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    fake.talkError = Exception('offline');
    await tester.pumpWidget(host(env.runtime, fake, env.store));
    await pumpUi(tester);

    fake.emitTalk(dest: 'bob', sender: 'bob', body: 'hello from bob');
    await pumpUi(tester);
    await tester.tap(find.text('hello from bob'));
    await pumpUi(tester);

    await tester.enterText(find.byType(TextField).last, 'ping');
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('composer-send')));
    await pumpUi(tester);

    expect(find.text('ping'), findsOneWidget);
    expect(find.text(Copy.retry), findsWidgets);
  });
}
