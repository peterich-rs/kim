import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/kim_tokens.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/design/kim_hairline.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  // PNGs were captured on macOS; Linux CI fonts/AA will not match.
  final skipGoldens = !Platform.isMacOS;

  Widget wrap({required ThemeData theme, required Widget child}) {
    return MaterialApp(
      theme: theme,
      locale: const Locale('zh'),
      home: Scaffold(body: Center(child: child)),
    );
  }

  testWidgets('KimTokens is attached on light and dark', (tester) async {
    await tester.pumpWidget(
      wrap(theme: KimTheme.light(), child: const SizedBox()),
    );
    final light = tester.element(find.byType(SizedBox));
    expect(Theme.of(light).extension<KimTokens>(), isNotNull);

    await tester.pumpWidget(
      wrap(theme: KimTheme.dark(), child: const SizedBox()),
    );
    final dark = tester.element(find.byType(SizedBox));
    expect(Theme.of(dark).extension<KimTokens>(), isNotNull);
  });

  testWidgets('group card golden light zh', (tester) async {
    await tester.pumpWidget(
      wrap(
        theme: KimTheme.light(),
        child: const SizedBox(
          width: 280,
          child: KimGroupCard(
            children: [
              ListTile(title: Text('会话')),
              KimHairline(indent: 16),
              ListTile(title: Text('通讯录')),
            ],
          ),
        ),
      ),
    );
    await expectLater(
      find.byType(KimGroupCard),
      matchesGoldenFile('goldens/kim_group_light_zh.png'),
    );
  }, skip: skipGoldens);

  testWidgets('group card golden dark zh', (tester) async {
    await tester.pumpWidget(
      wrap(
        theme: KimTheme.dark(),
        child: const SizedBox(
          width: 280,
          child: KimGroupCard(
            children: [
              ListTile(title: Text('会话')),
              KimHairline(indent: 16),
              ListTile(title: Text('通讯录')),
            ],
          ),
        ),
      ),
    );
    await expectLater(
      find.byType(KimGroupCard),
      matchesGoldenFile('goldens/kim_group_dark_zh.png'),
    );
  }, skip: skipGoldens);
}
