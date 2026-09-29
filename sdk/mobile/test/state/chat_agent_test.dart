import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/src/rust/api/types.dart';

import '../support/harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('fake injects an agent turn stop into the session stream', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final events = <SessionUpdate>[];
    final sub = env.fake.watchSessionEvents().listen(events.add);
    addTearDown(sub.cancel);
    env.fake.pushEvent(
      SessionUpdate.agentTurn(
        dest: 'b_bot',
        state: AgentTurnState.error,
        text: 'provider',
      ),
    );
    await Future<void>.delayed(Duration.zero);
    final turn = events.whereType<SessionUpdate_AgentTurn>().single;
    expect(turn.state, AgentTurnState.error);
    expect(turn.text, 'provider');
    expect(turn.dest, 'b_bot');
  });
}
