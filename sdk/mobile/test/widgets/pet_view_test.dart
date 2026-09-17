import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/design/pet_atlas_painter.dart';
import 'package:kim_mobile/design/pet_view.dart';
import 'package:kim_mobile/features/agent/agent_presence.dart';
import 'package:kim_mobile/features/agent/pet_pack.dart';

/// 16×18 indexed PNG, 8×9 cells of 2×2. Generated with zlib; not the
/// production atlas.
const _kAtlasPng = <int>[
  137,
  80,
  78,
  71,
  13,
  10,
  26,
  10,
  0,
  0,
  0,
  13,
  73,
  72,
  68,
  82,
  0,
  0,
  0,
  16,
  0,
  0,
  0,
  18,
  8,
  2,
  0,
  0,
  0,
  221,
  89,
  201,
  61,
  0,
  0,
  0,
  76,
  73,
  68,
  65,
  84,
  120,
  218,
  99,
  136,
  74,
  202,
  35,
  9,
  49,
  144,
  172,
  193,
  192,
  200,
  130,
  36,
  68,
  186,
  6,
  51,
  11,
  7,
  146,
  16,
  233,
  26,
  254,
  175,
  208,
  32,
  9,
  145,
  174,
  193,
  206,
  193,
  131,
  36,
  68,
  186,
  134,
  35,
  22,
  38,
  36,
  33,
  210,
  53,
  184,
  121,
  4,
  144,
  132,
  72,
  215,
  240,
  245,
  251,
  31,
  146,
  16,
  233,
  26,
  76,
  86,
  196,
  144,
  132,
  72,
  214,
  0,
  0,
  175,
  77,
  102,
  176,
  145,
  114,
  194,
  148,
  0,
  0,
  0,
  0,
  73,
  69,
  78,
  68,
  174,
  66,
  96,
  130,
];

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late PetPack pack;

  setUp(() {
    pack = PetPack.parseJson(<dynamic, dynamic>{
      'id': 'test',
      'frame': {'width': 2, 'height': 2},
      'columns': 8,
      'rows': 9,
    }, Uint8List.fromList(_kAtlasPng));
  });

  testWidgets('injected tiny atlas pumps one frame without exception', (
    tester,
  ) async {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(
            body: PetView(dest: 'goose', debugPack: pack),
          ),
        ),
      ),
    );
    await _pumpUntilPainter(tester);
    expect(tester.takeException(), isNull);
    expect(find.byType(PetView), findsOneWidget);
  });

  testWidgets('presence running uses src rect top matching running row', (
    tester,
  ) async {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    container.read(agentRunStatusProvider.notifier).begin('goose');
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(
            body: PetView(dest: 'goose', debugPack: pack),
          ),
        ),
      ),
    );
    await _pumpUntilPainter(tester);
    expect(
      container.read(agentPresenceProvider('goose')).phase,
      PetPhase.running,
    );
    final painter = tester
        .widgetList<CustomPaint>(find.byType(CustomPaint))
        .map((w) => w.painter)
        .whereType<PetAtlasPainter>()
        .single;
    expect(painter.src.top, 7 * 2);
    expect(tester.takeException(), isNull);
  });

  testWidgets('dest change applies the new dest phase', (tester) async {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    container.read(agentRunStatusProvider.notifier).begin('other');
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(
            body: PetView(dest: 'goose', debugPack: pack),
          ),
        ),
      ),
    );
    await _pumpUntilPainter(tester);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(
            body: PetView(dest: 'other', debugPack: pack),
          ),
        ),
      ),
    );
    await tester.pump();
    final painter = tester
        .widgetList<CustomPaint>(find.byType(CustomPaint))
        .map((w) => w.painter)
        .whereType<PetAtlasPainter>()
        .single;
    expect(painter.src.top, 7 * 2);
    expect(tester.takeException(), isNull);
  });
}

Future<void> _pumpUntilPainter(WidgetTester tester) async {
  await tester.pump();
  await tester.runAsync(() async {
    await Future<void>.delayed(const Duration(milliseconds: 20));
  });
  for (var i = 0; i < 20; i++) {
    await tester.pump();
    final found = tester
        .widgetList<CustomPaint>(find.byType(CustomPaint))
        .any((w) => w.painter is PetAtlasPainter);
    if (found) {
      return;
    }
    await tester.runAsync(() async {
      await Future<void>.delayed(const Duration(milliseconds: 10));
    });
  }
  fail('PetAtlasPainter did not appear');
}
