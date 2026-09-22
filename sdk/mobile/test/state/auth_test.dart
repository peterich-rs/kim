import 'package:flutter_riverpod/experimental/mutation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/session/link.dart';
import 'package:kim_mobile/features/session/mutations.dart';
import 'package:kim_mobile/features/session/session.dart';
import 'package:kim_mobile/src/rust/api/failure.dart';

import '../support/harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('empty token is signed out and does not open a session', () async {
    final env = await kimHarness();
    expect(env.container.read(authProvider).signedIn, isFalse);
    env.container.read(linkProvider);
    await Future<void>.delayed(Duration.zero);
    expect(env.fake.connects, 0);
  });

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

  test('signIn mutation surfaces unauthorized as MutationError', () async {
    final env = await kimHarness();
    env.fake.error = const ApiFailure.unauthorized();
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
    expect(env.container.read(authProvider).notice, isNull);
    expect(env.runtime.settings.token, isEmpty);
  });

  test('signOut(notice) keeps the kicked copy on the login screen', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await env.container
        .read(authProvider.notifier)
        .signOut(notice: Copy.kicked);
    expect(env.container.read(authProvider).signedIn, isFalse);
    expect(env.container.read(authProvider).notice, Copy.kicked);
    expect(env.runtime.settings.token, isEmpty);
  });
}
