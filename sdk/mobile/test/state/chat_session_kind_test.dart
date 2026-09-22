import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/chats/providers/chat_session.dart';
import 'package:kim_mobile/features/chats/providers/inbox.dart';
import 'package:kim_mobile/src/rust/api/types.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('group dest enqueues kind=1', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.read(threadsProvider);
    env.fake.pushSnapshot(
      SessionSnapshot(
        link: env.fake.snapshot.link,
        lastError: env.fake.snapshot.lastError,
        threads: const [
          ThreadView(
            id: 'G1',
            kind: 1,
            title: 'G1',
            avatar: '',
            lastBody: '',
            lastAt: 0,
            unread: 0,
          ),
        ],
        unreadTotal: 0,
      ),
    );
    await Future<void>.delayed(Duration.zero);
    final sub = env.container.listen(chatSessionProvider('G1'), (_, _) {});
    addTearDown(sub.close);
    final ok = await env.container
        .read(chatSessionProvider('G1').notifier)
        .sendText('hi');
    expect(ok, isTrue);
    expect(env.fake.lastEnqueueDest, 'G1');
    expect(env.fake.lastEnqueueKind, 1);
  });

  test('user dest enqueues kind=0', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.read(threadsProvider);
    env.fake.pushSnapshot(
      SessionSnapshot(
        link: env.fake.snapshot.link,
        lastError: env.fake.snapshot.lastError,
        threads: const [
          ThreadView(
            id: 'bob',
            kind: 0,
            title: 'bob',
            avatar: '',
            lastBody: '',
            lastAt: 0,
            unread: 0,
          ),
        ],
        unreadTotal: 0,
      ),
    );
    await Future<void>.delayed(Duration.zero);
    final sub = env.container.listen(chatSessionProvider('bob'), (_, _) {});
    addTearDown(sub.close);
    final ok = await env.container
        .read(chatSessionProvider('bob').notifier)
        .sendText('hi');
    expect(ok, isTrue);
    expect(env.fake.lastEnqueueDest, 'bob');
    expect(env.fake.lastEnqueueKind, 0);
  });
}
