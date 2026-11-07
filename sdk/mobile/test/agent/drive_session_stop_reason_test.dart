import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/session/link.dart';
import 'package:kim_mobile/features/session/typing.dart';
import 'package:kim_mobile/src/rust/api/types.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('agent turn error is the stop reason on the session stream', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final events = <SessionUpdate>[];
    final sub = env.fake.watchSessionEvents().listen(events.add);
    addTearDown(sub.cancel);
    env.fake.pushEvent(
      SessionUpdate.agentTurn(
        dest: 'b_bot',
        state: AgentTurnState.error,
        text: 'rate limited',
      ),
    );
    await Future<void>.delayed(Duration.zero);
    final turn = events.whereType<SessionUpdate_AgentTurn>().single;
    expect(turn.text, 'rate limited');
    expect(turn.state, AgentTurnState.error);
  });

  test('running agent turn lights typing and done clears it', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.listen(linkProvider, (_, _) {});
    await Future<void>.delayed(const Duration(milliseconds: 20));
    env.fake.pushEvent(
      SessionUpdate.agentTurn(
        dest: 'b_bot',
        state: AgentTurnState.running,
        text: '',
      ),
    );
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(env.container.read(peerTypingProvider('b_bot')), isTrue);
    env.fake.pushEvent(
      SessionUpdate.agentTurn(
        dest: 'b_bot',
        state: AgentTurnState.done,
        text: '',
      ),
    );
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(env.container.read(peerTypingProvider('b_bot')), isFalse);
  });
}
