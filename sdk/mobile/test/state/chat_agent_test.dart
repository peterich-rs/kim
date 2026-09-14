import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent_bridge.dart';
import 'package:kim_mobile/bridge/agent_bridge.dart';
import 'package:kim_mobile/src/rust/api/types.dart';

import '../support/harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('watch_agent_run stub prompt submits result', () async {
    final env = await kimHarness(token: 'tok.jwt', account: 'alice');
    final loop = AgentRunLoop(env.fake, AgentBridge())
      ..promptOverride = (req) async => 'echo:${req.text}';
    final done = loop.start();
    env.fake.agentRunCtrl.add(
      AgentRunRequestDto(
        dest: 'b_bot',
        profileId: 'bot',
        text: 'hi',
        inReplyTo: 1,
        epoch: BigInt.one,
      ),
    );
    await Future<void>.delayed(const Duration(milliseconds: 50));
    expect(env.fake.submittedAgentRuns, isNotEmpty);
    expect(env.fake.submittedAgentRuns.single.output, 'echo:hi');
    await env.fake.agentRunCtrl.close();
    await done;
  });
}
