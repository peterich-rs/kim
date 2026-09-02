import 'package:flutter_riverpod/experimental/mutation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/auth.dart';
import 'package:kim_mobile/state/link.dart';
import 'package:kim_mobile/state/mutations.dart';
import 'package:kim_mobile/state/session.dart';

import '../support/harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('signed-out auth does not open a gateway session', () async {
    final env = await kimHarness();
    expect(env.container.read(authProvider).signedIn, isFalse);
    env.container.read(linkProvider);
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(linkProvider).status, ConnStatus.offline);
    expect(env.container.read(sessionProvider).status, ConnStatus.offline);
    expect(env.fake.connects, 0);
  });

  test('signIn mutation stores JWT and marks signed-in', () async {
    final env = await kimHarness();
    await signInMutation.run(env.container, (tsx) {
      return tsx
          .get(authProvider.notifier)
          .signIn(register: false, account: 'alice', password: 'secret123');
    });
    expect(env.fake.logins, 1);
    expect(env.container.read(authProvider).signedIn, isTrue);
    expect(env.container.read(authProvider).account, 'alice');
    expect(env.runtime.settings.token, 'tok.jwt');
    expect(env.container.read(signInMutation), isA<MutationSuccess<void>>());
  });

  test('signIn mutation surfaces 401 as MutationError', () async {
    final env = await kimHarness();
    env.fake.error = Exception('http 401: 账号或密码错误');
    await expectLater(
      signInMutation.run(env.container, (tsx) {
        return tsx
            .get(authProvider.notifier)
            .signIn(register: false, account: 'alice', password: 'nope');
      }),
      throwsA(isA<Exception>()),
    );
    expect(env.container.read(authProvider).signedIn, isFalse);
    expect(env.container.read(signInMutation), isA<MutationError<void>>());
  });

  test('signOut clears token and identity', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    expect(env.container.read(authProvider).signedIn, isTrue);
    await env.container.read(authProvider.notifier).signOut();
    expect(env.fake.logouts, 1);
    expect(env.container.read(authProvider).signedIn, isFalse);
    expect(env.runtime.settings.token, isEmpty);
  });
}
