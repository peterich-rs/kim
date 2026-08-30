import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/gateway.dart';
import 'package:kim_mobile/state/session.dart';

import '../support/harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('signed-in session connects the gateway', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await env.container.read(gatewayProvider.future);
    expect(env.fake.connects, greaterThan(0));
    expect(env.container.read(sessionProvider).status, ConnStatus.online);
  });

  test('radio down returns offline without calling connect', () async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      online: false,
    );
    expect(
      await env.container.read(gatewayProvider.future),
      ConnStatus.offline,
    );
    expect(env.fake.connects, 0);
    expect(env.container.read(sessionProvider).status, ConnStatus.offline);
  });

  test('transient connect failure is retried', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.connectError = Exception('Connection refused');
    env.container.read(gatewayProvider);
    await Future<void>.delayed(Duration.zero);
    final failed = env.container.read(gatewayProvider);
    expect(failed.hasError, isTrue);
    expect(failed.retrying, isTrue);
    expect(env.container.read(sessionProvider).status, ConnStatus.reconnecting);

    env.fake.connectError = null;
    await Future<void>.delayed(const Duration(milliseconds: 250));
    expect(await env.container.read(gatewayProvider.future), ConnStatus.online);
    expect(env.fake.connects, greaterThan(1));
  });

  test('401 connect failure is not retried', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.connectError = Exception('http 401: 账号或密码错误');
    env.container.read(gatewayProvider);
    await Future<void>.delayed(Duration.zero);
    final failed = env.container.read(gatewayProvider);
    expect(failed.hasError, isTrue);
    expect(failed.retrying, isFalse);
    await Future<void>.delayed(const Duration(milliseconds: 250));
    expect(env.fake.connects, 1);
  });
}
