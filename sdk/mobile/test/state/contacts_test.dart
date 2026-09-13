import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/mention.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/agent_profiles.dart';
import 'package:kim_mobile/state/contacts.dart';
import 'package:kim_mobile/state/link.dart';
import 'package:kim_mobile/state/profile.dart';

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

  test('server bot dest is a friend without a list row', () {
    final social = ContactsState.empty();
    expect(social.isFriend('b_4Q03DOP0NRPC'), isTrue);
    expect(social.isFriend('alice'), isFalse);
  });

  test('conversation list prefers nickname over account id', () {
    final social = ContactsState(
      friends: const [
        KimPerson(
          account: 'b_4Q03DOP0NRPC',
          nickname: '助手',
          kind: ProfileKind.bot,
        ),
      ],
      incoming: const [],
      outgoing: const {},
      hits: const [],
    );
    const thread = KimThread(
      id: 'b_4Q03DOP0NRPC',
      kind: ThreadKind.user,
      title: 'b_4Q03DOP0NRPC',
    );
    expect(threadDisplayTitle(thread, social), '助手');
  });

  test('person goose is null after the row is deleted', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    addTearDown(() {
      debugDefaultTargetPlatformOverride = null;
    });
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final store = env.container.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    await store.saveProfile(
      const AgentProfile(
        id: kGooseAgentId,
        displayName: kGooseAgentName,
        providerKind: 'openai',
        baseUrl: '',
        model: 'gpt-4o',
        keyRef: 'agent.api_key.goose',
        systemPrompt: '',
      ),
    );
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(contactsProvider).person('goose'), isNotNull);
    await store.delete(kGooseAgentId);
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(contactsProvider).person('goose'), isNull);
  });

  test('onProfileUpdated patches friend nickname and avatar', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await env.container.read(agentProfilesProvider.notifier).ensureLoaded();
    await _online(env);
    env.fake.friends = const [
      KimPerson(account: 'bob', nickname: 'Bobby', avatar: 'old.png'),
    ];
    await env.container.read(contactsProvider.notifier).refresh();
    await Future<void>.delayed(Duration.zero);
    env.container
        .read(contactsProvider.notifier)
        .onProfileUpdated('bob', 'Robert', 'new.png');
    final bob = env.container.read(contactsProvider).person('bob');
    expect(bob?.nickname, 'Robert');
    expect(bob?.avatar, 'new.png');
  });
}
