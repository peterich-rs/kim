import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/contacts.dart';
import 'package:kim_mobile/state/gateway.dart';
import 'package:kim_mobile/state/inbox.dart';

import '../support/harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('send talks over the client after login', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    await env.container.read(gatewayProvider.future);
    await env.container.read(contactsProvider.notifier).refresh();
    final msg = await env.container
        .read(inboxProvider.notifier)
        .send('bob', 'hello');
    expect(msg.body, 'hello');
    expect(env.fake.talks, 1);
    expect(env.fake.lastTalkDest, 'bob');
    expect(
      env.container.read(inboxProvider).threads.map((t) => t.id),
      contains('bob'),
    );
  });

  test('send while offline throws not connected', () async {
    final env = await kimHarness(
      token: 'tok.jwt',
      account: 'alice',
      online: false,
    );
    await expectLater(
      env.container.read(inboxProvider.notifier).send('bob', 'hello'),
      throwsA(
        isA<StateError>().having(
          (e) => e.message,
          'message',
          Copy.notConnected,
        ),
      ),
    );
    expect(env.fake.talks, 0);
  });
}
