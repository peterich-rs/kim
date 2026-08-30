library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../models/models.dart';
import '../screens/auth_page.dart';
import '../screens/chat/chat_page.dart';
import '../screens/home/chats_page.dart';
import '../screens/home/contacts_page.dart';
import '../screens/home/home_shell.dart';
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
                builder: (context, state) => const ChatsPage(),
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
        path: '/chat/:id',
        pageBuilder: (context, state) {
          final id = state.pathParameters['id'] ?? '';
          final extra = state.extra;
          final thread = extra is KimThread ? extra : null;
          return kimPushPage(
            key: state.pageKey,
            name: state.name,
            arguments: extra,
            child: ChatPage(
              id: id,
              title: thread?.title ?? id,
              kind: thread?.kind ?? ThreadKind.user,
            ),
          );
        },
      ),
      GoRoute(
        path: '/password',
        pageBuilder: (context, state) => kimPushPage(
          key: state.pageKey,
          name: state.name,
          child: const PasswordPage(),
        ),
      ),
    ],
  );
});
