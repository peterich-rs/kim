import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/theme/kim_theme.dart';
import 'package:kim_mobile/widgets/chat/chat_list.dart';
import 'package:kim_mobile/widgets/kim_bubble.dart';

KimChatMsg _msg(String key, String body, {int at = 1, String sender = 'bob'}) {
  return KimChatMsg(key: key, dest: 'bob', sender: sender, body: body, at: at);
}

void main() {
  testWidgets('reverse list shows newest at the bottom', (tester) async {
    final items = [
      _msg('a', 'oldest', at: 1),
      _msg('b', 'mid', at: 2),
      _msg('c', 'newest', at: 3),
    ];
    await tester.pumpWidget(
      MaterialApp(
        theme: KimTheme.light(),
        home: Scaffold(
          body: ChatList(
            items: items,
            itemBuilder: (context, msg, index) {
              return SizedBox(
                height: 48,
                child: Text(msg.body, key: Key('row-${msg.key}')),
              );
            },
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final oldest = tester.getRect(find.text('oldest'));
    final newest = tester.getRect(find.text('newest'));
    expect(newest.bottom, greaterThan(oldest.bottom));
  });

  testWidgets('controller reports bottom edge and scrolls to bottom', (
    tester,
  ) async {
    final controller = ChatListController();
    final items = [for (var i = 0; i < 40; i++) _msg('$i', 'm$i', at: i)];
    await tester.pumpWidget(
      MaterialApp(
        theme: KimTheme.light(),
        home: Scaffold(
          body: SizedBox(
            height: 400,
            child: ChatList(
              items: items,
              controller: controller,
              itemBuilder: (context, msg, index) {
                return SizedBox(height: 48, child: Text(msg.body));
              },
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(controller.atBottomEdge, isTrue);
    await tester.drag(find.byKey(const Key('chat-list')), const Offset(0, 400));
    await tester.pumpAndSettle();
    expect(controller.atBottomEdge, isFalse);
    await controller.scrollToBottom(animated: false);
    await tester.pumpAndSettle();
    expect(controller.atBottomEdge, isTrue);
  });

  testWidgets('unread divider renders above the anchor', (tester) async {
    final items = [
      _msg('a', 'read', at: 1),
      _msg('b', 'first unread', at: 2),
      _msg('c', 'later', at: 3),
    ];
    await tester.pumpWidget(
      MaterialApp(
        theme: KimTheme.light(),
        home: Scaffold(
          body: ChatList(
            items: items,
            itemBuilder: (context, msg, index) {
              return KimMessageRow(
                message: msg,
                previous: index > 0 ? items[index - 1] : null,
                next: index + 1 < items.length ? items[index + 1] : null,
                isSentByMe: false,
                unreadAnchor: msg.key == 'b',
              );
            },
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text(Copy.unreadBelow), findsOneWidget);
    final divider = tester.getRect(find.byKey(const Key('unread-divider')));
    final firstUnread = tester.getRect(find.text('first unread'));
    expect(divider.bottom, lessThanOrEqualTo(firstUnread.top + 1));
  });

  test('groups consecutive messages using millisecond windows', () {
    const nano = 1788077118498491646;
    const twoSecondsLater = nano + 2 * 1000 * 1000 * 1000;
    final first = _msg('a', 'one', at: nano);
    final second = _msg('b', 'two', at: twoSecondsLater);
    expect(kimIsGroupStart(first, null), isTrue);
    expect(kimIsGroupStart(second, first), isFalse);
    expect(kimIsGroupEnd(first, second), isFalse);
    expect(kimIsGroupEnd(second, null), isTrue);

    final later = _msg(
      'c',
      'next batch',
      at: 1_700_000_000_000 + const Duration(minutes: 6).inMilliseconds,
    );
    final earlier = _msg('d', 'old', at: 1_700_000_000_000);
    expect(kimIsGroupStart(later, earlier), isTrue);
  });
}
