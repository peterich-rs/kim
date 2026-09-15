import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/kim_bubble.dart';

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
}
