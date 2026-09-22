import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:kim_mobile/features/chats/inbox.dart';
import 'package:kim_mobile/features/session/kim_session.dart';
import 'package:kim_mobile/features/chats/messages.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('pushSnapshot updates threadsProvider from snapshot', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.read(threadsProvider);
    env.fake.pushSnapshot(
      SessionSnapshot(
        link: const LinkState.online(),
        threads: [
          ThreadView(
            id: 'bob',
            kind: 0,
            title: 'Bob',
            avatar: '',
            lastBody: 'hi',
            lastAt: 2,
            unread: 1,
          ),
        ],
        unreadTotal: 1,
      ),
    );
    await Future<void>.delayed(Duration.zero);
    final threads = env.container.read(threadsProvider).threads;
    expect(threads.single.id, 'bob');
    expect(threads.single.unread, 1);
    expect(env.container.read(kimSessionProvider).threads, isNotEmpty);
  });

  test('watchThread throw leaves threadMessages empty, not error', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.fake.watchThreadError = StateError('no reactor running');
    final sub = env.container.listen(threadMessagesProvider('bob'), (_, _) {});
    addTearDown(sub.close);
    final state = env.container.read(threadMessagesProvider('bob'));
    expect(state.items, isEmpty);
  });

  test('fakeIncomingText updates threads and timeline', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.read(threadsProvider);
    final sub = env.container.listen(threadMessagesProvider('bob'), (_, _) {});
    addTearDown(sub.close);
    env.fake.fakeIncomingText(dest: 'bob', body: 'hello');
    await Future<void>.delayed(Duration.zero);
    expect(
      env.container.read(threadsProvider).thread('bob')?.lastBody,
      'hello',
    );
    final items = env.container.read(threadMessagesProvider('bob')).items;
    expect(items.any((m) => m.body == 'hello'), isTrue);
  });

  test('enqueueMessage pending appears on timeline', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    final sub = env.container.listen(threadMessagesProvider('bob'), (_, _) {});
    addTearDown(sub.close);
    await env.fake.enqueueMessage(
      dest: 'bob',
      kind: ThreadKind.user,
      content: const KimOutgoingContent.text('hi'),
      clientId: 'cid-1',
    );
    await Future<void>.delayed(Duration.zero);
    final items = env.container.read(threadMessagesProvider('bob')).items;
    expect(items.any((m) => m.key == 'cid-1' && m.body == 'hi'), isTrue);
  });

  test('loadOlder renders the SDK-expanded timeline snapshot', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    final sub = env.container.listen(threadMessagesProvider('bob'), (_, _) {});
    addTearDown(sub.close);
    final notifier = env.container.read(threadMessagesProvider('bob').notifier);
    env.fake.pushTimeline(
      'bob',
      TimelineUpdate.snapshot(
        snapshot: TimelineSnapshot(
          dest: 'bob',
          version: BigInt.one,
          messages: [
            MessageView(
              key: 'newer',
              dest: 'bob',
              sender: 'bob',
              body: 'newer',
              at: 2,
              sys: false,
              kind: 1,
              width: 0,
              height: 0,
              messageId: 2,
              sendStatus: SendStatus.sent,
            ),
          ],
          pending: const [],
          unread: 0,
          lastReadMessageId: 0,
          hasMore: true,
          loadingOlder: false,
          historyError: null,
        ),
      ),
    );
    env.fake.setOlderTimeline('bob', [
      MessageView(
        key: 'older',
        dest: 'bob',
        sender: 'bob',
        body: 'older',
        at: 1,
        sys: false,
        kind: 1,
        width: 0,
        height: 0,
        messageId: 1,
        sendStatus: SendStatus.sent,
      ),
    ]);
    await Future<void>.delayed(Duration.zero);

    await notifier.loadOlder();
    await Future<void>.delayed(Duration.zero);

    final state = env.container.read(threadMessagesProvider('bob'));
    expect(state.items.map((message) => message.key), ['older', 'newer']);
    expect(state.hasMore, isFalse);
    expect(state.loadingOlder, isFalse);
  });

  test('query filters snapshot threads without a second list', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.read(threadsProvider);
    env.fake.pushSnapshot(
      SessionSnapshot(
        link: const LinkState.online(),
        threads: [
          ThreadView(
            id: 'bob',
            kind: 0,
            title: 'Bob',
            avatar: '',
            lastBody: '',
            lastAt: 1,
            unread: 0,
          ),
          ThreadView(
            id: 'carol',
            kind: 0,
            title: 'Carol',
            avatar: '',
            lastBody: '',
            lastAt: 1,
            unread: 0,
          ),
        ],
        unreadTotal: 0,
      ),
    );
    await Future<void>.delayed(Duration.zero);
    env.container.read(threadsProvider.notifier).setQuery('bo');
    expect(env.container.read(threadsProvider).visible.map((t) => t.id), [
      'bob',
    ]);
  });
}
