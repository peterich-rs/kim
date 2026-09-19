import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/kim_bubble.dart';

KimChatMsg _msg(String body) {
  return KimChatMsg(key: '1', dest: 'bob', sender: 'bob', body: body, at: 1);
}

void main() {
  testWidgets('desktop bubble text is selectable', (tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    try {
      await tester.pumpWidget(
        MaterialApp(
          theme: KimTheme.light().copyWith(platform: TargetPlatform.macOS),
          home: Scaffold(
            body: KimMessageRow(
              message: _msg('hello from bob'),
              isSentByMe: false,
              onLongPress: (_) {},
            ),
          ),
        ),
      );
      expect(find.text('hello from bob'), findsOneWidget);
      expect(find.byType(SelectableText), findsOneWidget);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });

  testWidgets('phone bubble text is not a SelectableText', (tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
    try {
      await tester.pumpWidget(
        MaterialApp(
          theme: KimTheme.light().copyWith(platform: TargetPlatform.iOS),
          home: Scaffold(
            body: KimMessageRow(
              message: _msg('hello from bob'),
              isSentByMe: false,
              onLongPress: (_) {},
            ),
          ),
        ),
      );
      expect(find.text('hello from bob'), findsOneWidget);
      expect(find.byType(SelectableText), findsNothing);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });
}
