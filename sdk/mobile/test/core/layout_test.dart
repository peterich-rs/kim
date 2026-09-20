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

  test('body inset keeps a max width on wide panes', () {
    expect(kimBodyInset(390), 16);
    expect(kimBodyInset(640), 16);
    expect(kimBodyInset(672), 16);
    expect(kimBodyInset(1280, maxWidth: 640), 320);
    expect(kimBodyInset(800, maxWidth: 420), (800 - 420) / 2);
  });

  test('overlay paths are agent, password, dev, peer', () {
    expect(kimIsOverlayPath('/agent'), isTrue);
    expect(kimIsOverlayPath('/agent/new'), isTrue);
    expect(kimIsOverlayPath('/password'), isTrue);
    expect(kimIsOverlayPath('/dev'), isTrue);
    expect(kimIsOverlayPath('/peer/bob'), isTrue);
    expect(kimIsOverlayPath('/'), isFalse);
    expect(kimIsOverlayPath('/me'), isFalse);
    expect(kimIsOverlayPath('/contacts'), isFalse);
    expect(kimIsOverlayPath('/chat/bob'), isFalse);
  });

  test('desktop selects message text; phones keep long-press copy', () {
    expect(kimSelectsMessageText(TargetPlatform.macOS), isTrue);
    expect(kimSelectsMessageText(TargetPlatform.windows), isTrue);
    expect(kimSelectsMessageText(TargetPlatform.linux), isTrue);
    expect(kimSelectsMessageText(TargetPlatform.iOS), isFalse);
    expect(kimSelectsMessageText(TargetPlatform.android), isFalse);
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
