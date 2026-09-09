import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/theme/kim_theme.dart';
import 'package:kim_mobile/widgets/conversation_tile.dart';

void main() {
  KimThread thread({int unread = 0}) => KimThread(
    id: 'bob',
    kind: ThreadKind.user,
    title: 'Bobby',
    lastBody: 'hello from bob',
    unread: unread,
  );

  Future<void> pumpRail(
    WidgetTester tester, {
    required KimThread thread,
    bool selected = false,
    VoidCallback? onOpen,
  }) {
    return tester.pumpWidget(
      MaterialApp(
        theme: KimTheme.light(),
        home: Scaffold(
          body: SizedBox(
            width: 80,
            child: ConversationRailAvatar(
              thread: thread,
              selected: selected,
              onOpen: onOpen ?? () {},
            ),
          ),
        ),
      ),
    );
  }

  testWidgets('shows title as tooltip, not as list text', (tester) async {
    await pumpRail(tester, thread: thread());
    expect(find.text('Bobby'), findsNothing);
    expect(find.byTooltip('Bobby'), findsOneWidget);
    expect(find.text('hello from bob'), findsNothing);
  });

  testWidgets('unread count renders on the avatar', (tester) async {
    await pumpRail(tester, thread: thread(unread: 3));
    expect(find.text('3'), findsOneWidget);
  });

  testWidgets('tap opens the conversation', (tester) async {
    var opened = false;
    await pumpRail(tester, thread: thread(), onOpen: () => opened = true);
    await tester.tap(find.byType(ConversationRailAvatar));
    expect(opened, isTrue);
  });
}
