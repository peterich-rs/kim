import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/state/chat_agent.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('stub enqueueTurn has no store or goose side effects', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    final agent = env.container.read(chatAgentProvider);
    await agent.enqueueTurn('bob', 'hi', 1);
    await agent.catchUpPending();
    await agent.sendDirect(dest: 'bob', text: 'x');
    expect(env.fake.enqueues, 0);
  });
}
