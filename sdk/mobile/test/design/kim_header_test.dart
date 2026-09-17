import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:kim_mobile/design/kim_header.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('tab root header has no back button', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: CustomScrollView(slivers: const [KimSliverHeader(title: '我')]),
        ),
      ),
    );
    expect(find.byKey(const Key('kim-back')), findsNothing);
  });

  testWidgets('pushed page header pops', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) {
            return TextButton(
              onPressed: () {
                Navigator.of(context).push<void>(
                  MaterialPageRoute<void>(
                    builder: (_) => const Scaffold(
                      body: CustomScrollView(
                        slivers: [KimSliverHeader(title: 'Agent')],
                      ),
                    ),
                  ),
                );
              },
              child: const Text('open'),
            );
          },
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('kim-back')), findsOneWidget);
    await tester.tap(find.byKey(const Key('kim-back')));
    await tester.pumpAndSettle();
    expect(find.text('open'), findsOneWidget);
    expect(find.text('Agent'), findsNothing);
  });

  testWidgets('overlay without a stack still returns home', (tester) async {
    final router = GoRouter(
      initialLocation: '/agent',
      routes: [
        GoRoute(path: '/', builder: (context, state) => const Text('home')),
        GoRoute(
          path: '/agent',
          builder: (context, state) => const Scaffold(
            body: CustomScrollView(slivers: [KimSliverHeader(title: 'Agent')]),
          ),
        ),
      ],
    );
    addTearDown(router.dispose);
    await tester.pumpWidget(MaterialApp.router(routerConfig: router));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('kim-back')), findsOneWidget);
    await tester.tap(find.byKey(const Key('kim-back')));
    await tester.pumpAndSettle();
    expect(find.text('home'), findsOneWidget);
  });
}
