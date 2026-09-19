import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/session/link.dart';
import 'package:kim_mobile/features/session/typing.dart';
import 'package:kim_mobile/src/rust/api/types.dart';

import '../support/harness.dart';
import '../support/jwt.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('local AgentTurn updates typing without a server push', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.listen(linkProvider, (_, _) {});
    await Future<void>.delayed(const Duration(milliseconds: 20));
    for (final state in [
      AgentTurnStateDto.queued,
      AgentTurnStateDto.running,
      AgentTurnStateDto.waitingPermission,
      AgentTurnStateDto.done,
      AgentTurnStateDto.error,
      AgentTurnStateDto.empty,
    ]) {
      env.fake.pushEvent(
        SessionUpdateDto.agentTurn(dest: 'b_bot', state: state, text: ''),
      );
      await Future<void>.delayed(const Duration(milliseconds: 20));
      expect(
        env.container.read(peerTypingProvider('b_bot')),
        state == AgentTurnStateDto.running,
        reason: '$state',
      );
      expect(env.container.read(peerTypingProvider('alice')), isFalse);
    }
  });

  test(
    'owner-desktop ignores bot Typing push; AgentTurn running lights it',
    () async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      addTearDown(() => debugDefaultTargetPlatformOverride = null);
      final env = await kimHarness(
        token: testJwt(acc: 'alice', exp: 4_000_000_000),
        account: 'alice',
      );
      env.container.listen(linkProvider, (_, _) {});
      await Future<void>.delayed(const Duration(milliseconds: 20));
      env.fake.pushEvent(
        const SessionUpdateDto.typing(
          typer: 'b_bot',
          dest: 'alice',
          kind: 0,
          active: true,
        ),
      );
      await Future<void>.delayed(const Duration(milliseconds: 20));
      expect(env.container.read(peerTypingProvider('b_bot')), isFalse);
      env.fake.pushEvent(
        SessionUpdateDto.agentTurn(
          dest: 'b_bot',
          state: AgentTurnStateDto.running,
          text: '',
        ),
      );
      await Future<void>.delayed(const Duration(milliseconds: 20));
      expect(env.container.read(peerTypingProvider('b_bot')), isTrue);
    },
  );

  test(
    'heartbeat Typing is ignored once a local AgentTurn owns the dest',
    () async {
      final env = await kimHarness(
        token: testJwt(acc: 'alice', exp: 4_000_000_000),
        account: 'alice',
      );
      env.container.listen(linkProvider, (_, _) {});
      await Future<void>.delayed(const Duration(milliseconds: 20));
      env.fake.pushEvent(
        SessionUpdateDto.agentTurn(
          dest: 'b_bot',
          state: AgentTurnStateDto.running,
          text: '',
        ),
      );
      await Future<void>.delayed(const Duration(milliseconds: 20));
      expect(env.container.read(peerTypingProvider('b_bot')), isTrue);
      env.fake.pushEvent(
        SessionUpdateDto.agentTurn(
          dest: 'b_bot',
          state: AgentTurnStateDto.done,
          text: 'pong',
        ),
      );
      await Future<void>.delayed(const Duration(milliseconds: 20));
      expect(env.container.read(peerTypingProvider('b_bot')), isFalse);
      env.fake.pushEvent(
        const SessionUpdateDto.typing(
          typer: 'b_bot',
          dest: 'alice',
          kind: 0,
          active: true,
        ),
      );
      await Future<void>.delayed(const Duration(milliseconds: 20));
      expect(env.container.read(peerTypingProvider('b_bot')), isFalse);
    },
  );

  test('next AgentTurn running lights typing after done', () async {
    final env = await kimHarness(
      token: testJwt(acc: 'alice', exp: 4_000_000_000),
      account: 'alice',
    );
    env.container.listen(linkProvider, (_, _) {});
    await Future<void>.delayed(const Duration(milliseconds: 20));
    env.fake.pushEvent(
      SessionUpdateDto.agentTurn(
        dest: 'b_bot',
        state: AgentTurnStateDto.done,
        text: 'pong',
      ),
    );
    await Future<void>.delayed(const Duration(milliseconds: 20));
    env.fake.pushEvent(
      const SessionUpdateDto.typing(
        typer: 'b_bot',
        dest: 'alice',
        kind: 0,
        active: true,
      ),
    );
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(env.container.read(peerTypingProvider('b_bot')), isFalse);
    env.fake.pushEvent(
      SessionUpdateDto.agentTurn(
        dest: 'b_bot',
        state: AgentTurnStateDto.running,
        text: '',
      ),
    );
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(env.container.read(peerTypingProvider('b_bot')), isTrue);
  });
}
