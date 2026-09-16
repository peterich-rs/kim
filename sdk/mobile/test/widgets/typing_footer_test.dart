import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/design/chat/chat_list.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/kim_typing_bars.dart';
import 'package:kim_mobile/features/session/typing.dart';
import 'package:kim_mobile/models/models.dart';

void main() {
  testWidgets('typing push shows animated KimTypingRow footer', (tester) async {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          theme: KimTheme.light(),
          home: Consumer(
            builder: (context, ref, _) {
              final typing = ref.watch(peerTypingProvider('alice'));
              return Scaffold(
                body: ChatList(
                  items: const <KimChatMsg>[],
                  footer: typing
                      ? const KimTypingRow(
                          key: Key('typing-row'),
                          name: 'alice',
                          avatar: SizedBox(width: 28, height: 28),
                        )
                      : null,
                  itemBuilder: (context, msg, index) {
                    return Text(msg.body);
                  },
                ),
              );
            },
          ),
        ),
      ),
    );
    expect(find.byKey(const Key('typing-row')), findsNothing);
    container
        .read(typingProvider.notifier)
        .applyPush(typer: 'alice', dest: 'bob', active: true);
    await tester.pump();
    expect(find.byKey(const Key('typing-row')), findsOneWidget);
    expect(find.byType(KimTypingBars), findsOneWidget);
    await tester.pump(const Duration(milliseconds: 200));
    expect(tester.takeException(), isNull);
    container
        .read(typingProvider.notifier)
        .applyPush(typer: 'alice', dest: 'bob', active: false);
    await tester.pump();
  });
}
