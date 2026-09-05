import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/theme/kim_theme.dart';
import 'package:kim_mobile/widgets/kim_composer.dart';

void main() {
  testWidgets('plus opens album and camera actions', (tester) async {
    var album = 0;
    var camera = 0;
    var sent = '';
    await tester.pumpWidget(
      MaterialApp(
        theme: KimTheme.light(),
        home: Scaffold(
          body: KimComposer(
            onSend: (text) => sent = text,
            onPickAlbum: () => album += 1,
            onTakePhoto: () => camera += 1,
          ),
        ),
      ),
    );

    expect(find.text(Copy.album), findsNothing);
    await tester.tap(find.byKey(const Key('composer-plus')));
    await tester.pumpAndSettle();
    expect(find.text(Copy.album), findsOneWidget);
    expect(find.text(Copy.camera), findsOneWidget);

    await tester.tap(find.byKey(const Key('composer-album')));
    await tester.pumpAndSettle();
    expect(album, 1);
    expect(find.text(Copy.album), findsNothing);

    await tester.tap(find.byKey(const Key('composer-plus')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('composer-camera')));
    await tester.pumpAndSettle();
    expect(camera, 1);

    await tester.enterText(find.byType(TextField), 'hello');
    await tester.pumpAndSettle();
    // Floating chrome keeps + and send as separate controls.
    expect(find.byKey(const Key('composer-plus')), findsOneWidget);
    expect(find.byKey(const Key('composer-send')), findsOneWidget);
    await tester.tap(find.byKey(const Key('composer-send')));
    await tester.pumpAndSettle();
    expect(sent, 'hello');
  });
}
