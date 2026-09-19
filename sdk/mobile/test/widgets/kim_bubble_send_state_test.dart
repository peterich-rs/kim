import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/kim_bubble.dart';

KimChatMsg _bot(
  String body, {
  required KimSendStatus status,
  String key = 'bot-1',
}) {
  return KimChatMsg(
    key: key,
    dest: 'b_bot',
    sender: 'b_bot',
    body: body,
    at: 1,
    failed: status == KimSendStatus.failed,
    status: status,
  );
}

void main() {
  testWidgets('bot sending hides spinner until the busy delay', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: KimTheme.light(),
        home: Scaffold(
          body: KimMessageRow(
            message: _bot('pong', status: KimSendStatus.sending),
            isSentByMe: false,
          ),
        ),
      ),
    );
    expect(find.text('pong'), findsOneWidget);
    expect(find.byType(CircularProgressIndicator), findsNothing);
    await tester.pump(kPeerSendBusyDelay);
    expect(find.byType(CircularProgressIndicator), findsOneWidget);
  });

  testWidgets('failed bot line shows retry and taps it', (tester) async {
    var taps = 0;
    await tester.pumpWidget(
      MaterialApp(
        theme: KimTheme.light(),
        home: Scaffold(
          body: KimMessageRow(
            message: _bot('pong', status: KimSendStatus.failed),
            isSentByMe: false,
            onRetry: () => taps += 1,
          ),
        ),
      ),
    );
    expect(find.text('pong'), findsOneWidget);
    expect(find.byKey(const Key('retry-bot-1')), findsOneWidget);
    expect(find.text(Copy.retry), findsOneWidget);
    await tester.tap(find.byKey(const Key('retry-bot-1')));
    await tester.pump();
    expect(taps, 1);
  });
}
