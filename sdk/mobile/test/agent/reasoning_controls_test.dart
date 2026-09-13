import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/agent/catalog.dart';
import 'package:kim_mobile/l10n/app_localizations.dart';
import 'package:kim_mobile/screens/agent/reasoning_controls.dart';
import 'package:kim_mobile/state/agent_profiles.dart';

Future<void> _pump(
  WidgetTester tester, {
  required ReasoningSurfaceDto surface,
  required ReasoningChoice choice,
  ValueChanged<ReasoningChoice>? onChanged,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      locale: const Locale('zh'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(
        body: ReasoningControls(
          surface: surface,
          choice: choice,
          onChanged: onChanged ?? (_) {},
        ),
      ),
    ),
  );
}

void main() {
  testWidgets('DeepSeek effort_enum shows none|low|high|max, not Off–Max', (
    tester,
  ) async {
    await _pump(
      tester,
      surface: const ReasoningSurfaceDto(
        kind: 'effort_enum',
        allowed: ['none', 'low', 'high', 'max'],
        defaultValue: 'high',
      ),
      choice: const ReasoningChoice(kind: 'effort_enum', value: 'high'),
    );
    expect(find.text('none'), findsOneWidget);
    expect(find.text('low'), findsOneWidget);
    expect(find.text('high'), findsOneWidget);
    expect(find.text('max'), findsOneWidget);
    expect(find.text('medium'), findsNothing);
    expect(find.byType(SegmentedButton<String>), findsOneWidget);
  });

  testWidgets('Claude effort_enum is low|high|max without Off', (tester) async {
    await _pump(
      tester,
      surface: const ReasoningSurfaceDto(
        kind: 'effort_enum',
        allowed: ['low', 'high', 'max'],
        defaultValue: 'high',
      ),
      choice: const ReasoningChoice(kind: 'effort_enum', value: 'high'),
    );
    expect(find.text('low'), findsOneWidget);
    expect(find.text('high'), findsOneWidget);
    expect(find.text('max'), findsOneWidget);
    expect(find.text('none'), findsNothing);
    expect(find.text('medium'), findsNothing);
  });

  testWidgets('toggle surface is a Switch, not an effort row', (tester) async {
    await _pump(
      tester,
      surface: const ReasoningSurfaceDto(kind: 'toggle', defaultOn: false),
      choice: const ReasoningChoice(kind: 'toggle', on: false),
    );
    expect(find.byType(SwitchListTile), findsOneWidget);
    expect(find.byType(SegmentedButton<String>), findsNothing);
  });

  testWidgets('always_on surface is copy, MiniMax M2 has no toggle', (
    tester,
  ) async {
    await _pump(
      tester,
      surface: const ReasoningSurfaceDto(
        kind: 'always_on',
        note: 'thinking stays on',
      ),
      choice: const ReasoningChoice(kind: 'always_on'),
    );
    expect(find.text('始终开启'), findsWidgets);
    expect(find.text('thinking stays on'), findsOneWidget);
    expect(find.byType(SwitchListTile), findsNothing);
    expect(find.byType(SegmentedButton<String>), findsNothing);
  });

  testWidgets('none surface hides the effort control', (tester) async {
    await _pump(
      tester,
      surface: const ReasoningSurfaceDto(kind: 'none'),
      choice: const ReasoningChoice(kind: 'none'),
    );
    expect(find.byType(SegmentedButton<String>), findsNothing);
    expect(find.byType(SwitchListTile), findsNothing);
  });
}
