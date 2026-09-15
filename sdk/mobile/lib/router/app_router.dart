library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/features/auth/auth_page.dart';
import 'package:kim_mobile/features/chats/chats_split.dart';
import 'package:kim_mobile/features/contacts/contacts_page.dart';
import 'package:kim_mobile/features/chats/home_shell.dart';
import 'package:kim_mobile/features/agent/agent_list_page.dart';
import 'package:kim_mobile/features/agent/agent_plaza_page.dart';
import 'package:kim_mobile/features/agent/agent_settings_page.dart';
import 'package:kim_mobile/features/agent/agent_capabilities_page.dart';
import 'package:kim_mobile/features/agent/provider_account_page.dart';
import 'package:kim_mobile/features/agent/provider_accounts_page.dart';
import 'package:kim_mobile/core/env.dart';
import 'package:kim_mobile/features/profile/me_page.dart';
import 'package:kim_mobile/features/settings/dev_panel.dart';
import 'package:kim_mobile/features/contacts/peer_profile_page.dart';
import 'package:kim_mobile/features/auth/password_page.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/session/location.dart';
import 'package:kim_mobile/features/session/session.dart';
import 'package:kim_mobile/router/kim_page.dart';

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
        if (!ref.mounted) {
          return;
        }
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
      if (kimDevPanelEnabled)
        GoRoute(
          path: '/dev',
          pageBuilder: (context, state) => kimPushPage(
            key: state.pageKey,
            name: state.name,
            child: const DevPanelPage(),
          ),
        ),
      GoRoute(
        path: '/peer/:id',
        name: 'peer',
        pageBuilder: (context, state) {
          final id = state.pathParameters['id'] ?? '';
          final title = state.uri.queryParameters['title'] ?? '';
          return kimPushPage(
            key: state.pageKey,
            name: state.name,
            child: PeerProfilePage(account: id, seedTitle: title),
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
      GoRoute(
        path: '/agent',
        pageBuilder: (context, state) => kimPushPage(
          key: state.pageKey,
          name: state.name,
          child: const AgentListPage(),
        ),
        routes: [
          GoRoute(
            path: 'accounts/new',
            pageBuilder: (context, state) => kimPushPage(
              key: state.pageKey,
              name: state.name,
              child: const ProviderAccountPage(),
            ),
          ),
          GoRoute(
            path: 'accounts/:accountId',
            pageBuilder: (context, state) {
              final id = state.pathParameters['accountId'] ?? '';
              return kimPushPage(
                key: state.pageKey,
                name: state.name,
                child: ProviderAccountPage(accountId: id),
              );
            },
          ),
          GoRoute(
            path: 'accounts',
            pageBuilder: (context, state) => kimPushPage(
              key: state.pageKey,
              name: state.name,
              child: const ProviderAccountsPage(),
            ),
          ),
          GoRoute(path: 'settings', redirect: (context, state) => '/agent'),
          GoRoute(
            path: 'plaza',
            pageBuilder: (context, state) => kimPushPage(
              key: state.pageKey,
              name: state.name,
              child: AgentPlazaPage(
                assignTo: state.uri.queryParameters['assignTo'],
              ),
            ),
          ),
          GoRoute(
            path: 'new',
            pageBuilder: (context, state) => kimPushPage(
              key: state.pageKey,
              name: state.name,
              child: const AgentEditorPage(),
            ),
          ),
          GoRoute(
            path: ':id',
            pageBuilder: (context, state) {
              final id = state.pathParameters['id'] ?? kGooseAgentId;
              return kimPushPage(
                key: state.pageKey,
                name: state.name,
                child: AgentEditorPage(profileId: id),
              );
            },
            routes: [
              GoRoute(
                path: 'capabilities',
                pageBuilder: (context, state) {
                  final id = state.pathParameters['id'] ?? kGooseAgentId;
                  return kimPushPage(
                    key: state.pageKey,
                    name: state.name,
                    child: AgentCapabilitiesPage(
                      profileId: id,
                      section: state.uri.queryParameters['section'],
                    ),
                  );
                },
              ),
              GoRoute(
                path: 'workspace',
                redirect: (context, state) {
                  final id = state.pathParameters['id'] ?? kGooseAgentId;
                  return '/agent/$id/capabilities?section=fs';
                },
              ),
              GoRoute(
                path: 'skills',
                redirect: (context, state) {
                  final id = state.pathParameters['id'] ?? kGooseAgentId;
                  return '/agent/$id/capabilities?section=skills';
                },
              ),
              GoRoute(
                path: 'tools',
                redirect: (context, state) {
                  final id = state.pathParameters['id'] ?? kGooseAgentId;
                  return '/agent/$id/capabilities?section=mcp';
                },
              ),
            ],
          ),
        ],
      ),
    ],
  );
});
