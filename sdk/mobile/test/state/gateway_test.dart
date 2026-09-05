import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/auth.dart';
import 'package:kim_mobile/state/link.dart';
import 'package:kim_mobile/state/session.dart';

import '../support/harness.dart';

Future<void> _tick() => Future<void>.delayed(Duration.zero);

Future<void> _waitSignedOut(KimHarness env) async {
  for (var i = 0; i < 20; i++) {
    await _tick();
    if (!env.container.read(authProvider).signedIn) {
      return;
    }
  }
  fail('session did not sign out');
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('signed-in session starts the supervisor', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container.read(linkProvider);
    await _tick();
    expect(env.fake.connects, greaterThan(0));
    expect(env.container.read(sessionProvider).status, ConnStatus.online);
  });

  test('radio down keeps socket online', () async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      online: false,
    );
    env.container.read(linkProvider);
    await _tick();
    expect(env.container.read(sessionProvider).status, ConnStatus.online);
    expect(env.container.read(linkProvider).status, ConnStatus.online);
  });

  test('unknown ffi kind does not mark reconnecting', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container.read(linkProvider);
    await _tick();
    expect(env.container.read(linkProvider).status, ConnStatus.online);
    env.fake.eventsController.add(
      const KimEvent(kind: KimEventKind.closed, error: 'ignored'),
    );
    await _tick();
    expect(env.container.read(linkProvider).status, ConnStatus.online);
  });

  test('token renew does not restart session', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container.read(linkProvider);
    await _tick();
    final connects = env.fake.connects;
    await env.container.read(linkProvider.notifier).retry();
    await _tick();
    expect(env.fake.connects, connects);
    expect(env.fake.radioUps, greaterThan(0));
  });

  test('connect failure surfaces offline', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.connectError = Exception('Connection refused');
    env.container.read(linkProvider);
    await _tick();
    expect(env.container.read(linkProvider).status, ConnStatus.offline);
    expect(env.container.read(sessionProvider).status, ConnStatus.offline);

    env.fake.connectError = null;
    await env.container.read(linkProvider.notifier).retry();
    await _tick();
    expect(env.container.read(linkProvider).status, ConnStatus.online);
    expect(env.fake.connects, greaterThan(1));
  });

  test('authExpired event clears the session', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container.read(linkProvider);
    await _tick();
    expect(env.container.read(authProvider).signedIn, isTrue);
    env.fake.emitAuthExpired();
    await _waitSignedOut(env);
    expect(env.container.read(authProvider).notice, Copy.sessionExpired);
    expect(env.runtime.settings.token, isEmpty);
  });

  test('kick signs out without session-expired notice', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container.read(linkProvider);
    await _tick();
    env.fake.eventsController.add(const KimEvent(kind: KimEventKind.kick));
    await _waitSignedOut(env);
    expect(env.container.read(authProvider).notice, isNull);
  });

  test('401 connect failure stays offline', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.connectError = Exception('http 401: 账号或密码错误');
    env.container.read(linkProvider);
    await _tick();
    expect(env.container.read(linkProvider).status, ConnStatus.offline);
    await Future<void>.delayed(const Duration(milliseconds: 50));
    expect(env.fake.connects, 1);
  });
}
