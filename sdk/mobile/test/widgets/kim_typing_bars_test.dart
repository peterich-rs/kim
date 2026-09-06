import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/widgets/kim_typing_bars.dart';

void main() {
  testWidgets('KimTypingBars paints staggered bars', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(body: Center(child: KimTypingBars())),
      ),
    );
    expect(find.byType(KimTypingBars), findsOneWidget);
    await tester.pump(const Duration(milliseconds: 200));
    expect(tester.takeException(), isNull);
  });
}
