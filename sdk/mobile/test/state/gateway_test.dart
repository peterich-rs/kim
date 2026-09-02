import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/link.dart';
import 'package:kim_mobile/state/session.dart';

import '../support/harness.dart';

Future<void> _tick() => Future<void>.delayed(Duration.zero);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('signed-in session starts the supervisor', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container.read(linkProvider);
    await _tick();
    expect(env.fake.connects, greaterThan(0));
    expect(env.container.read(sessionProvider).status, ConnStatus.online);
  });

  test('radio down shows offline', () async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      online: false,
    );
    env.container.read(linkProvider);
    await _tick();
    expect(env.container.read(sessionProvider).status, ConnStatus.offline);
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
