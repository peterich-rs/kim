/// MaterialApp.router: M3 light/dark, connectivity banner, tap-outside dismiss.
library;

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:toastification/toastification.dart';

import 'copy.dart';
import 'core/runtime.dart';
import 'data/conversation_store.dart';
import 'kim_bridge.dart';
import 'router/app_router.dart';
import 'router/kim_page.dart';
import 'state/live.dart';
import 'state/profile.dart';
import 'state/providers.dart';
import 'state/retry.dart';
import 'theme/kim_theme.dart';
import 'widgets/kim_offline_banner.dart';

class KimApp extends ConsumerWidget {
  const KimApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Keep these on the root. IndexedStack tabs pause Riverpod 3 listeners.
    ref.watch(liveEventsProvider);
    ref.watch(sessionLinkProvider);
    ref.watch(profileProvider);
    final router = ref.watch(routerProvider);
    return ToastificationWrapper(
      child: MaterialApp.router(
        title: Copy.brand,
        debugShowCheckedModeBanner: false,
        theme: KimTheme.light(),
        darkTheme: KimTheme.dark(),
        themeMode: ThemeMode.system,
        locale: const Locale('zh', 'CN'),
        supportedLocales: const [Locale('zh', 'CN'), Locale('en', 'US')],
        localizationsDelegates: const [
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        routerConfig: router,
        builder: (context, child) {
          return Listener(
            onPointerDown: (event) {
              if (event.position.dx < kKimBackGestureWidth) {
                return;
              }
              FocusManager.instance.primaryFocus?.unfocus();
            },
            child: KimOfflineBanner(child: child ?? const SizedBox.shrink()),
          );
        },
      ),
    );
  }
}

/// Test / preview host that injects runtime + ports into Riverpod.
class KimAppHost extends StatelessWidget {
  const KimAppHost({
    super.key,
    required this.runtime,
    required this.auth,
    required this.client,
    required this.store,
  });

  final KimRuntime runtime;
  final KimAuthPort auth;
  final KimClientPort client;
  final ConversationStore store;

  @override
  Widget build(BuildContext context) {
    return ProviderScope(
      retry: kimRetry,
      overrides: kimProviderOverrides(
        runtime: runtime,
        auth: auth,
        client: client,
        store: store,
      ),
      child: const KimApp(),
    );
  }
}
