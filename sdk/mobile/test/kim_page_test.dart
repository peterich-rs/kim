import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/router/kim_page.dart';

void main() {
  test('kimPushPage is a swipe-back page', () {
    final page = kimPushPage(
      key: const ValueKey('chat'),
      child: const SizedBox(),
    );
    expect(page, isA<KimSwipePage>());
  });

  testWidgets('left-edge swipe pops the page', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) {
            return TextButton(
              onPressed: () {
                Navigator.of(context).push<void>(
                  kimPushPage(
                    key: const ValueKey('chat'),
                    child: const Scaffold(body: SizedBox.expand()),
                  ).createRoute(context),
                );
              },
              child: const Text('open'),
            );
          },
        ),
      ),
    );

    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byType(Scaffold), findsOneWidget);

    // 40px is past Cupertino's 20px edge and inside kKimBackGestureWidth.
    final gesture = await tester.startGesture(const Offset(40, 300));
    await gesture.moveBy(const Offset(500, 0));
    await tester.pump();
    await gesture.up();
    await tester.pumpAndSettle();

    expect(find.byType(Scaffold), findsNothing);
  });
}
