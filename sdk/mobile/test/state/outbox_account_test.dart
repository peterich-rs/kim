import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/state/messages.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('enqueueMessage writes pending onto timeline', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.read(threadMessagesProvider('bob'));
    await env.fake.enqueueMessage(
      dest: 'bob',
      kind: ThreadKind.user,
      content: const KimOutgoingContent.text('queued'),
      clientId: 'q1',
    );
    await Future<void>.delayed(Duration.zero);
    final items = env.container.read(threadMessagesProvider('bob')).items;
    expect(items.single.key, 'q1');
    expect(items.single.body, 'queued');
    expect(env.fake.enqueues, 1);
  });
}
