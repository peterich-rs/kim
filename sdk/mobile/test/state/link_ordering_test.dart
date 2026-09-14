import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/session/link.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('kickout event signs out', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.listen(linkProvider, (_, _) {});
    await Future<void>.delayed(Duration.zero);
    env.fake.emitKick();
    await Future<void>.delayed(const Duration(milliseconds: 50));
    expect(env.container.read(authProvider).signedIn, isFalse);
    expect(env.container.read(authProvider).notice, Copy.kicked);
  });

  test('authExpired event signs out', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.listen(linkProvider, (_, _) {});
    await Future<void>.delayed(Duration.zero);
    env.fake.emitAuthExpired();
    await Future<void>.delayed(const Duration(milliseconds: 50));
    expect(env.container.read(authProvider).signedIn, isFalse);
  });

  test('friend request is upserted from discrete event', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.listen(linkProvider, (_, _) {});
    env.fake.emitFriend(from: 'bob', nickname: 'Bob');
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(env.fake.sessionUpdateCtrl.hasListener, isTrue);
  });
}
