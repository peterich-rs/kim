import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/layout.dart';

void main() {
  test('layout bands split at 600 and 900', () {
    expect(kimLayoutSizeForWidth(1100), KimLayoutSize.wide);
    expect(kimLayoutSizeForWidth(900), KimLayoutSize.wide);
    expect(kimLayoutSizeForWidth(899), KimLayoutSize.compact);
    expect(kimLayoutSizeForWidth(600), KimLayoutSize.compact);
    expect(kimLayoutSizeForWidth(599), KimLayoutSize.narrow);
    expect(kimLayoutSizeForWidth(390), KimLayoutSize.narrow);
  });

  testWidgets('kimIsWide covers compact and wide', (tester) async {
    Future<void> expectAt(double width, {required bool wide}) async {
      late bool actual;
      await tester.pumpWidget(
        MediaQuery(
          data: MediaQueryData(size: Size(width, 800)),
          child: Builder(
            builder: (context) {
              actual = kimIsWide(context);
              return const SizedBox();
            },
          ),
        ),
      );
      expect(actual, wide, reason: 'width $width');
    }

    await expectAt(1100, wide: true);
    await expectAt(720, wide: true);
    await expectAt(390, wide: false);
  });
}
