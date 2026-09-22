import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/features/contacts/contacts.dart';
import 'package:kim_mobile/features/profile/profile.dart';
import 'package:kim_mobile/features/session/link.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart' hide AgentProfile;
import '../support/harness.dart';

Future<void> _online(dynamic env) async {
  env.container.read(linkProvider);
  for (var i = 0; i < 20; i++) {
    await Future<void>.delayed(Duration.zero);
    if (env.container.read(linkProvider).status == ConnStatus.online) {
      return;
    }
  }
}

ContactsSnapshot _contacts(List<Person> contacts, {String? syncError}) {
  return ContactsSnapshot(
    version: BigInt.one,
    contacts: contacts,
    syncError: syncError,
  );
}

Person _person({
  required String account,
  required String nickname,
  String avatar = '',
  String relation = 'friend',
  int kind = ProfileKind.user,
}) {
  return Person(
    account: account,
    nickname: nickname,
    avatar: avatar,
    bio: '',
    relation: relation,
    kind: kind,
  );
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('contacts list changes only from contacts snapshots', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await _online(env);
    await env.container.read(contactsProvider.notifier).request('bob');
    expect(env.container.read(contactsProvider).isOutgoing('bob'), isTrue);

    env.fake.emitFriend(from: 'bob', nickname: 'Bobby', accepted: true);
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(contactsProvider).isFriend('bob'), isFalse);

    env.fake.pushContacts(
      _contacts([_person(account: 'bob', nickname: 'Bobby')]),
    );
    await Future<void>.delayed(Duration.zero);
    final social = env.container.read(contactsProvider);
    expect(social.isFriend('bob'), isTrue);
    expect(social.isOutgoing('bob'), isFalse);
  });

  test('friend request event does not insert an incoming contact', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await _online(env);
    env.container.read(contactsProvider);
    env.fake.emitFriend(from: 'bob', nickname: 'Bobby');
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(contactsProvider).isIncoming('bob'), isFalse);

    env.fake.pushContacts(
      _contacts([
        _person(account: 'bob', nickname: 'Bobby', relation: 'incoming'),
      ]),
    );
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(contactsProvider).isIncoming('bob'), isTrue);
  });

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

  test('profile changes arrive through the contacts snapshot', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await env.container.read(agentProfilesProvider.notifier).ensureLoaded();
    await _online(env);
    env.container.read(contactsProvider);
    env.fake.pushContacts(
      _contacts([
        _person(account: 'bob', nickname: 'Bobby', avatar: 'old.png'),
      ]),
    );
    await Future<void>.delayed(Duration.zero);

    env.fake.pushEvent(
      SessionUpdate.profileUpdated(
        account: 'bob',
        nickname: 'Robert',
        avatar: 'new.png',
      ),
    );
    await Future<void>.delayed(Duration.zero);
    expect(
      env.container.read(contactsProvider).person('bob')?.nickname,
      'Bobby',
    );

    env.fake.pushContacts(
      _contacts([
        _person(account: 'bob', nickname: 'Robert', avatar: 'new.png'),
      ]),
    );
    await Future<void>.delayed(Duration.zero);
    final bob = env.container.read(contactsProvider).person('bob');
    expect(bob?.nickname, 'Robert');
    expect(bob?.avatar, 'new.png');
  });

  test('removePeer waits for a contacts snapshot after friendRemove', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await _online(env);
    env.container.read(contactsProvider);
    env.fake.pushContacts(
      _contacts([_person(account: 'bob', nickname: 'Bobby')]),
    );
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(contactsProvider).isFriend('bob'), isTrue);
    await env.container
        .read(contactsProvider.notifier)
        .removePeer('bob', isBot: false);
    expect(env.fake.friendRemoves, 1);
    expect(env.container.read(contactsProvider).isFriend('bob'), isTrue);
    env.fake.pushContacts(_contacts(const []));
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(contactsProvider).isFriend('bob'), isFalse);
  });

  test('removePeer waits for a contacts snapshot after botDelete', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    await _online(env);
    env.container.read(contactsProvider);
    env.fake.pushContacts(
      _contacts([
        _person(account: 'b_bot', nickname: '助手', kind: ProfileKind.bot),
      ]),
    );
    await Future<void>.delayed(Duration.zero);
    await env.container
        .read(contactsProvider.notifier)
        .removePeer('b_bot', isBot: true);
    expect(env.fake.botDeletes, 1);
    expect(env.container.read(contactsProvider).person('b_bot'), isNotNull);
    env.fake.pushContacts(_contacts(const []));
    await Future<void>.delayed(Duration.zero);
    expect(env.container.read(contactsProvider).person('b_bot'), isNull);
  });
}
