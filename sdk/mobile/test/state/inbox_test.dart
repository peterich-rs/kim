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
      SessionSnapshotDto(
        link: const LinkStateDto.online(),
        threads: [
          ThreadViewDto(
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

  test('fakeIncomingText updates threads and timeline', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.read(threadsProvider);
    env.container.read(threadMessagesProvider('bob'));
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
    env.container.read(threadMessagesProvider('bob'));
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

  test('query filters snapshot threads without a second list', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.read(threadsProvider);
    env.fake.pushSnapshot(
      SessionSnapshotDto(
        link: const LinkStateDto.online(),
        threads: [
          ThreadViewDto(
            id: 'bob',
            kind: 0,
            title: 'Bob',
            avatar: '',
            lastBody: '',
            lastAt: 1,
            unread: 0,
          ),
          ThreadViewDto(
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
