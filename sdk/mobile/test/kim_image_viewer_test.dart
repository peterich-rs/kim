import 'package:dismissible_page/dismissible_page.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/theme/kim_theme.dart';
import 'package:kim_mobile/widgets/kim_image_viewer.dart';
import 'package:photo_view/photo_view.dart';

void main() {
  testWidgets('showKimImageViewer pushes photo_view on a dismissible page', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: KimTheme.light(),
        home: Builder(
          builder: (context) {
            return TextButton(
              onPressed: () {
                showKimImageViewer(
                  context,
                  src: 'https://media.kim.ainexc.com/alice/a.png',
                  heroTag: 'img-test',
                );
              },
              child: const Text('open'),
            );
          },
        ),
      ),
    );

    await tester.tap(find.text('open'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));

    expect(find.byType(DismissiblePage), findsOneWidget);
    expect(find.byType(PhotoView), findsOneWidget);
    expect(find.byTooltip(Copy.closeViewer), findsOneWidget);

    await tester.tap(find.byTooltip(Copy.closeViewer));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(PhotoView), findsNothing);
  });
}
