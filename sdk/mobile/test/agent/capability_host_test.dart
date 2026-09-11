import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/capability_host.dart';
import 'package:kim_mobile/agent/mention.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/contacts.dart';
import 'package:kim_mobile/state/inbox.dart';

import '../support/harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('send_message to goose dest returns error JSON', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final host = env.container.read(Provider((ref) => KimCapabilityHost(ref)));
    final out = await host.execute(
      name: 'send_message',
      argumentsJson: '{"dest":"$kGooseAgentId","text":"hi"}',
      sessionDest: 'bob',
      profileId: 'goose',
    );
    expect(out, contains('"ok":false'));
    expect(out, contains('local agent'));
  });

  test('send_message to group dest returns error JSON', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final host = env.container.read(Provider((ref) => KimCapabilityHost(ref)));
    final out = await host.execute(
      name: 'send_message',
      argumentsJson: '{"dest":"room/1","text":"hi"}',
      sessionDest: 'bob',
      profileId: 'goose',
    );
    expect(out, contains('"ok":false'));
    expect(out, contains('group dest'));
  });

  test('send_message to a known group id returns error JSON', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container.read(threadsProvider.notifier).mergeInbox(const [
      KimThread(id: 'squad', kind: ThreadKind.group, title: 'squad'),
    ]);
    final host = env.container.read(Provider((ref) => KimCapabilityHost(ref)));
    final out = await host.execute(
      name: 'send_message',
      argumentsJson: '{"dest":"squad","text":"hi"}',
      sessionDest: 'bob',
      profileId: 'goose',
    );
    expect(out, contains('"ok":false'));
    expect(out, contains('group dest'));
  });

  test('search_contacts returns friends', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    env.container.read(contactsProvider.notifier).state = ContactsState(
      friends: const [
        KimPerson(account: 'bob', nickname: 'Bob'),
        kGooseAgentPerson,
      ],
      incoming: const [],
      outgoing: const {},
      hits: const [],
      ready: true,
    );
    final host = env.container.read(Provider((ref) => KimCapabilityHost(ref)));
    final out = await host.execute(
      name: 'search_contacts',
      argumentsJson: '{"query":"bo"}',
      sessionDest: 'goose',
      profileId: 'goose',
    );
    expect(out, contains('bob'));
    expect(out, isNot(contains('goose')));
  });
}
