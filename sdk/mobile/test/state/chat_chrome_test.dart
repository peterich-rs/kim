import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/chats/chat_view.dart';
import 'package:kim_mobile/features/contacts/contacts.dart';
import 'package:kim_mobile/features/profile/profile.dart';
import 'package:kim_mobile/models/models.dart';

void main() {
  const me = ProfileState(account: 'me', nickname: 'Me', avatar: '');

  ContactsState social({
    List<KimPerson> friends = const [],
    bool ready = true,
  }) {
    return ContactsState(
      friends: friends,
      incoming: const [],
      outgoing: const {},
      hits: const [],
      ready: ready,
    );
  }

  test('stranger user thread is gated and writable', () {
    final chrome = deriveChatChrome(
      dest: 'alice',
      kind: ThreadKind.user,
      social: social(),
      me: me,
      profiles: const [],
      profilesReady: true,
      hostSupported: true,
    );
    expect(chrome.gated, isTrue);
    expect(chrome.readOnly, isFalse);
    expect(chrome.userThread, isTrue);
    expect(chrome.liveTitle, 'alice');
  });

  test('friend user thread is not gated', () {
    final chrome = deriveChatChrome(
      dest: 'alice',
      kind: ThreadKind.user,
      social: social(
        friends: const [KimPerson(account: 'alice', nickname: 'Alice')],
      ),
      me: me,
      profiles: const [],
      profilesReady: true,
      hostSupported: true,
    );
    expect(chrome.gated, isFalse);
    expect(chrome.liveTitle, 'Alice');
  });

  test('desktop orphan bot is read-only once profiles are ready', () {
    final chrome = deriveChatChrome(
      dest: 'b_orphan',
      kind: ThreadKind.user,
      social: social(),
      me: me,
      profiles: const [],
      profilesReady: true,
      hostSupported: true,
    );
    expect(chrome.readOnly, isTrue);
    expect(chrome.agentThread, isTrue);
    expect(chrome.gated, isFalse);
  });

  test('bot stays writable while the profile store is still loading', () {
    final chrome = deriveChatChrome(
      dest: 'goose',
      kind: ThreadKind.user,
      social: social(),
      me: me,
      profiles: const [],
      profilesReady: false,
      hostSupported: true,
    );
    expect(chrome.readOnly, isFalse);
    expect(chrome.agentChat, isTrue);
  });

  test('phone orphan server bot is read-only when not a friend', () {
    final chrome = deriveChatChrome(
      dest: 'b_phone',
      kind: ThreadKind.user,
      social: social(),
      me: me,
      profiles: const [],
      profilesReady: true,
      hostSupported: false,
    );
    expect(chrome.readOnly, isTrue);
  });
}
