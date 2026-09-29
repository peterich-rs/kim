import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/design/agent_action_bubble.dart';
import 'package:kim_mobile/design/kim_bubble.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/models/models.dart';

import '../support/fake_kim.dart';

void main() {
  testWidgets('hides tool cards and shows permission prompts', (tester) async {
    final tool = KimChatMsg(
      key: 'agent-card-t1',
      dest: 'agent:goose',
      sender: '助手',
      body: '',
      at: 1,
      kind: KimMsgKind.agentCard,
      card: const KimAgentCard(
        callId: 't1',
        name: 'tool',
        actionRequired: false,
        pending: false,
        preview: '{"ok":true}',
        ok: true,
      ),
    );
    final ask = KimChatMsg(
      key: 'agent-card-c1',
      dest: 'agent:goose',
      sender: '助手',
      body: '',
      at: 2,
      kind: KimMsgKind.agentCard,
      card: const KimAgentCard(
        callId: 'c1',
        name: 'bash',
        actionRequired: true,
        pending: true,
        preview: 'Allow bash?',
        ok: false,
      ),
    );

    await tester.pumpWidget(
      ProviderScope(
        child: MaterialApp(
          theme: KimTheme.light(),
          home: Scaffold(
            body: ListView(
              children: [
                KimMessageRow(message: tool, isSentByMe: false),
                KimMessageRow(message: ask, isSentByMe: false),
              ],
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('tool'), findsNothing);
    expect(find.text('{"ok":true}'), findsNothing);
    expect(find.text('bash'), findsOneWidget);
    expect(find.text('Allow bash?'), findsOneWidget);
  });

  testWidgets('allow button answers through the client port', (tester) async {
    final client = _RecordingClient();
    final container = ProviderContainer(
      overrides: [clientPortProvider.overrideWithValue(client)],
    );
    addTearDown(container.dispose);
    container.read(agentPermissionHubProvider.notifier).bindClient(client);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          theme: KimTheme.light(),
          home: const Scaffold(
            body: AgentActionBubble(
              dest: 'b_bot',
              callId: 'c1',
              name: 'bash',
              preview: 'Allow bash?',
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('允许'));
    await tester.pump();
    expect(client.callId, 'c1');
    expect(client.allow, isTrue);
  });
}

class _RecordingClient extends FakeKim {
  String? callId;
  bool? allow;

  @override
  Future<void> respondAgentPermission({
    required String callId,
    required bool allow,
  }) async {
    this.callId = callId;
    this.allow = allow;
  }
}
