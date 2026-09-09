library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../core/layout.dart';
import '../screens/auth_page.dart';
import '../screens/home/chats_split.dart';
import '../screens/home/contacts_page.dart';
import '../screens/home/home_shell.dart';
import '../screens/agent/agent_settings_page.dart';
import '../screens/home/me_page.dart';
import '../screens/password_page.dart';
import '../state/auth.dart';
import '../state/location.dart';
import '../state/session.dart';
import 'kim_page.dart';

final routerProvider = Provider<GoRouter>((ref) {
  final refresh = ValueNotifier<int>(0);
  ref.onDispose(refresh.dispose);
  ref.listen(authProvider.select((s) => s.signedIn), (prev, next) {
    refresh.value++;
  });

  return GoRouter(
    initialLocation: '/',
    refreshListenable: refresh,
    redirect: (context, state) {
      final path = state.uri.path;
      Future<void>.microtask(() {
        ref.read(locationProvider.notifier).setPath(path);
      });
      final signedIn = ref.read(sessionProvider).signedIn;
      final loc = state.matchedLocation;
      final onAuth = loc == '/login' || loc == '/register';
      if (!signedIn && !onAuth) {
        return '/login';
      }
      if (signedIn && onAuth) {
        return '/';
      }
      return null;
    },
    errorBuilder: (context, state) => KimErrorPage(error: state.error),
    routes: [
      GoRoute(
        path: '/login',
        builder: (context, state) => const AuthPage(register: false),
      ),
      GoRoute(
        path: '/register',
        builder: (context, state) => const AuthPage(register: true),
      ),
      StatefulShellRoute.indexedStack(
        builder: (context, state, navigationShell) {
          return HomeShell(navigationShell: navigationShell);
        },
        branches: [
          StatefulShellBranch(
            routes: [
              GoRoute(
                path: '/',
                builder: (context, state) => const ChatsSplitView(),
                routes: [
                  GoRoute(
                    path: 'chat/:id',
                    name: 'chat',
                    pageBuilder: (context, state) {
                      final id = state.pathParameters['id'] ?? '';
                      final host = ChatRouteHost(id: id);
                      if (kimIsWide(context)) {
                        return NoTransitionPage<void>(
                          key: state.pageKey,
                          child: host,
                        );
                      }
                      return kimPushPage(
                        key: state.pageKey,
                        name: state.name,
                        child: host,
                      );
                    },
                  ),
                ],
              ),
            ],
          ),
          StatefulShellBranch(
            routes: [
              GoRoute(
                path: '/contacts',
                builder: (context, state) => const ContactsPage(),
              ),
            ],
          ),
          StatefulShellBranch(
            routes: [
              GoRoute(path: '/me', builder: (context, state) => const MePage()),
            ],
          ),
        ],
      ),
      GoRoute(
        path: '/password',
        pageBuilder: (context, state) => kimPushPage(
          key: state.pageKey,
          name: state.name,
          child: const PasswordPage(),
        ),
      ),
      GoRoute(
        path: '/agent/settings',
        pageBuilder: (context, state) => kimPushPage(
          key: state.pageKey,
          name: state.name,
          child: const AgentSettingsPage(),
        ),
      ),
    ],
  );
});
