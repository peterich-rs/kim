import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/widgets/kim_dock.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

void main() {
  testWidgets('KimDock renders icon-only items and reports taps', (
    tester,
  ) async {
    var selected = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: KimDock(
            selectedIndex: selected,
            onSelected: (index) => selected = index,
            items: const [
              KimDockItem(
                key: Key('nav-conversations'),
                icon: LucideIcons.messageCircle,
                label: '消息',
              ),
              KimDockItem(
                key: Key('nav-contacts'),
                icon: LucideIcons.users,
                label: '通讯录',
                badge: 3,
              ),
              KimDockItem(
                key: Key('nav-me'),
                icon: LucideIcons.user,
                label: '我',
              ),
            ],
          ),
        ),
      ),
    );

    expect(find.byType(KimDock), findsOneWidget);
    expect(find.text('消息'), findsNothing);
    expect(find.byTooltip('消息'), findsOneWidget);
    expect(find.byIcon(LucideIcons.messageCircle), findsOneWidget);
    expect(find.text('3'), findsOneWidget);

    await tester.tap(find.byKey(const Key('nav-contacts')));
    await tester.pump();
    expect(selected, 1);
  });

  testWidgets('KimDock overlap includes a floor for devices without insets', (
    tester,
  ) async {
    late double overlap;
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) {
            overlap = KimDock.overlapOf(context);
            return const SizedBox.shrink();
          },
        ),
      ),
    );
    expect(overlap, KimDock.pillHeight + KimDock.minBottomGap);
  });
}
