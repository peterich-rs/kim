import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/kim_bubble.dart';
import 'package:kim_mobile/design/agent_action_bubble.dart';

void main() {
  testWidgets('hides tool progress cards but shows permission prompts', (
    tester,
  ) async {
    final tool = KimChatMsg(
      key: 'agent-card-t1',
      dest: 'agent:goose',
      sender: '助手',
      body: jsonEncode({
        'v': 1,
        'type': 'tool',
        'call_id': 't1',
        'name': 'tool',
        'state': 'ok',
        'preview': '{"ok":true}',
        'ok': true,
      }),
      at: 1,
      kind: KimMsgKind.agentCard,
    );
    final ask = KimChatMsg(
      key: 'agent-card-c1',
      dest: 'agent:goose',
      sender: '助手',
      body: jsonEncode({
        'v': 1,
        'type': 'action_required',
        'call_id': 'c1',
        'name': 'bash',
        'state': 'pending',
        'preview': 'Allow bash?',
        'ok': false,
      }),
      at: 2,
      kind: KimMsgKind.agentCard,
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

  testWidgets('allow button routes to the permission hub session', (
    tester,
  ) async {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final session = _BubbleSession();
    container
        .read(agentPermissionHubProvider.notifier)
        .attach('b_bot', session);
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
    expect(session.callId, 'c1');
    expect(session.permission, 'allow_once');
  });
}

class _BubbleSession implements AgentSessionPort {
  String? callId;
  String? permission;

  @override
  Stream<AgentUiEvent> listen() => const Stream.empty();

  @override
  Future<String> prompt({required String text}) async => '';

  @override
  Future<String> promptWithContext({
    required String text,
    required String contextJson,
  }) async => '';

  @override
  Future<String> completeTool({
    required String callId,
    required String outputJson,
  }) async => '';

  @override
  Future<String> respondPermission({
    required String callId,
    required String permission,
  }) async {
    this.callId = callId;
    this.permission = permission;
    return '';
  }

  @override
  Future<void> close() async {}

  @override
  Future<void> abort() async {}

  @override
  Future<void> steer({required String text}) async {}

  @override
  Future<void> reconfigure({required SessionOpenOpts opts}) async {}

  @override
  Future<ResumeReportDto> resume() async =>
      const ResumeReportDto(resumedOps: [], statuses: []);

  @override
  Future<SessionSnapshotDto> snapshot() async => const SessionSnapshotDto(
    busy: false,
    lastOperationId: '',
    phase: '',
    pendingCallIds: [],
  );
}
