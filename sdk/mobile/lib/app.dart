/// MaterialApp.router: M3 light/dark, connectivity banner, tap-outside dismiss.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/core/runtime.dart';
import 'package:kim_mobile/bridge/kim_bridge.dart';
import 'package:kim_mobile/router/app_router.dart';
import 'package:kim_mobile/router/kim_page.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/chats/chats_search.dart';
import 'package:kim_mobile/features/session/link.dart';
import 'package:kim_mobile/features/profile/profile.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/features/session/retry.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/kim_offline_banner.dart';
import 'package:kim_mobile/design/new_chat_sheet.dart';

/// Phone: tap outside an input to drop the software keyboard.
/// Desktop: that same pointer-down also hits the focused field and IME
/// candidate window, so composition dies and typing looks stuck.
bool kimDismissesKeyboardOnTapOutside(TargetPlatform platform) {
  return switch (platform) {
    TargetPlatform.iOS ||
    TargetPlatform.android ||
    TargetPlatform.fuchsia => true,
    TargetPlatform.macOS ||
    TargetPlatform.windows ||
    TargetPlatform.linux => false,
  };
}

Map<ShortcutActivator, VoidCallback> _desktopShortcuts(
  BuildContext context,
  WidgetRef ref,
) {
  final meta = defaultTargetPlatform == TargetPlatform.macOS;
  SingleActivator chord(LogicalKeyboardKey key) =>
      SingleActivator(key, meta: meta, control: !meta);

  void goTab(String path) {
    if (!ref.read(authProvider).signedIn) {
      return;
    }
    context.go(path);
  }

  return {
    chord(LogicalKeyboardKey.digit1): () => goTab('/'),
    chord(LogicalKeyboardKey.digit2): () => goTab('/contacts'),
    chord(LogicalKeyboardKey.digit3): () => goTab('/me'),
    chord(LogicalKeyboardKey.keyN): () {
      if (!ref.read(authProvider).signedIn) {
        return;
      }
      unawaited(openNewChatSheet(context));
    },
    chord(LogicalKeyboardKey.keyF): () {
      if (!ref.read(authProvider).signedIn) {
        return;
      }
      context.go('/');
      ref.read(chatsSearchTickProvider.notifier).request();
    },
    const SingleActivator(LogicalKeyboardKey.escape): () {
      final path = GoRouterState.of(context).uri.path;
      if (path.startsWith('/chat/')) {
        if (kimIsWide(context) || !context.canPop()) {
          context.go('/');
        } else {
          context.pop();
        }
      }
    },
  };
}

class KimApp extends ConsumerWidget {
  const KimApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Keep these on the root. IndexedStack tabs pause Riverpod 3 listeners.
    ref.watch(linkProvider);
    ref.watch(profileProvider);
    final router = ref.watch(routerProvider);
    return ToastificationWrapper(
      child: MaterialApp.router(
        title: Copy.brand,
        debugShowCheckedModeBanner: false,
        theme: KimTheme.light(),
        darkTheme: KimTheme.dark(),
        themeMode: ref.watch(themeModeProvider),
        locale: const Locale('zh'),
        supportedLocales: AppLocalizations.supportedLocales,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        routerConfig: router,
        builder: (context, child) {
          return CallbackShortcuts(
            bindings: _desktopShortcuts(context, ref),
            child: Focus(
              autofocus: true,
              child: Listener(
                onPointerDown: (event) {
                  if (!kimDismissesKeyboardOnTapOutside(
                    defaultTargetPlatform,
                  )) {
                    return;
                  }
                  if (event.position.dx < kKimBackGestureWidth) {
                    return;
                  }
                  FocusManager.instance.primaryFocus?.unfocus();
                },
                child: KimOfflineBanner(
                  child: child ?? const SizedBox.shrink(),
                ),
              ),
            ),
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
  });

  final KimRuntime runtime;
  final KimAuthPort auth;
  final KimClientPort client;

  @override
  Widget build(BuildContext context) {
    return ProviderScope(
      retry: kimRetry,
      overrides: kimProviderOverrides(
        runtime: runtime,
        auth: auth,
        client: client,
      ),
      child: const KimApp(),
    );
  }
}
