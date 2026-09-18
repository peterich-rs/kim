import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/chats/chat_session.dart';
import 'package:kim_mobile/features/chats/messages.dart';
import 'package:kim_mobile/src/rust/api/types.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('send receipt does not insert a message before timeline push', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.fake.autoPushEnqueueTimeline = false;
    final msgs = env.container.listen(threadMessagesProvider('bob'), (_, _) {});
    final session = env.container.listen(chatSessionProvider('bob'), (_, _) {});
    addTearDown(msgs.close);
    addTearDown(session.close);
    final accepted = await env.container
        .read(chatSessionProvider('bob').notifier)
        .sendText('queued');
    await Future<void>.delayed(Duration.zero);

    expect(accepted, isTrue);
    expect(env.container.read(threadMessagesProvider('bob')).items, isEmpty);
    expect(env.fake.enqueues, 1);

    final clientId = env.fake.lastClientId;
    env.fake.pushTimeline(
      'bob',
      TimelineUpdateDto.delta(
        delta: TimelineDeltaDto(
          dest: 'bob',
          fromVersion: BigInt.zero,
          toVersion: BigInt.one,
          upserts: [
            MessageViewDto(
              key: clientId,
              dest: 'bob',
              sender: 'alice',
              body: 'queued',
              at: 1,
              sys: false,
              kind: 1,
              width: 0,
              height: 0,
              messageId: 0,
              sendStatus: SendStatusDto.pending,
            ),
          ],
          deletedKeys: const [],
        ),
      ),
    );
    await Future<void>.delayed(Duration.zero);

    final items = env.container.read(threadMessagesProvider('bob')).items;
    expect(items.single.key, clientId);
    expect(items.single.body, 'queued');
  });
}
