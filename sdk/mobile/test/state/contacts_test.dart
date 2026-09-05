import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/contacts.dart';
import 'package:kim_mobile/state/link.dart';

import '../support/harness.dart';

Future<void> _online(dynamic env) async {
  env.container.read(linkProvider);
  await Future<void>.delayed(Duration.zero);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('accept push moves an outgoing request into friends', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await _online(env);
    await env.container.read(contactsProvider.notifier).request('bob');
    expect(env.container.read(contactsProvider).isOutgoing('bob'), isTrue);
    env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
    env.container.read(contactsProvider.notifier).onAccepted('bob', 'Bobby');
    await Future<void>.delayed(Duration.zero);
    final social = env.container.read(contactsProvider);
    expect(social.isFriend('bob'), isTrue);
    expect(social.isOutgoing('bob'), isFalse);
  });

  test(
    'request push against an outgoing dest is treated as accepted',
    () async {
      final env = await kimHarness(token: 'tok.jwt', account: 'alice');
      await _online(env);
      await env.container.read(contactsProvider.notifier).request('bob');
      env.fake.friends = const [KimPerson(account: 'bob', nickname: 'Bobby')];
      env.container.read(contactsProvider.notifier).onRequest('bob', 'Bobby');
      await Future<void>.delayed(Duration.zero);
      final social = env.container.read(contactsProvider);
      expect(social.isFriend('bob'), isTrue);
      expect(social.isOutgoing('bob'), isFalse);
      expect(social.isIncoming('bob'), isFalse);
    },
  );
}
