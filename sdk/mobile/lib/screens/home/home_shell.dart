library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../core/layout.dart';
import '../../state/contacts.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/kim_dock.dart';

class HomeShell extends ConsumerWidget {
  const HomeShell({super.key, required this.navigationShell});

  final StatefulNavigationShell navigationShell;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final incoming = ref.watch(contactsProvider.select((s) => s.incomingCount));
    final layout = kimLayoutSize(context);
    final split = layout != KimLayoutSize.narrow;
    final onChat = GoRouterState.of(context).uri.path.startsWith('/chat/');
    final destinations = <NavigationDestination>[
      NavigationDestination(
        icon: const Icon(LucideIcons.messageCircle),
        label: Copy.conversations,
      ),
      NavigationDestination(
        icon: Badge(
          isLabelVisible: incoming > 0,
          label: Text(incoming > 9 ? '9+' : '$incoming'),
          child: const Icon(LucideIcons.users),
        ),
        label: Copy.contacts,
      ),
      NavigationDestination(icon: const Icon(LucideIcons.user), label: Copy.me),
    ];

    void go(int index) {
      KimHaptics.selection();
      navigationShell.goBranch(
        index,
        initialLocation: index == navigationShell.currentIndex,
      );
    }

    if (split) {
      return Scaffold(
        backgroundColor: KimTheme.canvasOf(context),
        body: Row(
          children: [
            NavigationRail(
              selectedIndex: navigationShell.currentIndex,
              onDestinationSelected: go,
              labelType: switch (layout) {
                KimLayoutSize.wide => NavigationRailLabelType.all,
                KimLayoutSize.compact ||
                KimLayoutSize.narrow => NavigationRailLabelType.none,
              },
              destinations: [
                for (final d in destinations)
                  NavigationRailDestination(icon: d.icon, label: Text(d.label)),
              ],
            ),
            VerticalDivider(
              width: 1,
              thickness: 1,
              color: KimTheme.hairlineOf(context),
            ),
            Expanded(child: navigationShell),
          ],
        ),
      );
    }

    if (onChat) {
      return Scaffold(
        backgroundColor: KimTheme.canvasOf(context),
        body: navigationShell,
      );
    }

    return Scaffold(
      backgroundColor: KimTheme.canvasOf(context),
      resizeToAvoidBottomInset: false,
      body: Stack(
        children: [
          navigationShell,
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            child: KimDock(
              selectedIndex: navigationShell.currentIndex,
              onSelected: go,
              items: [
                KimDockItem(
                  key: const Key('nav-conversations'),
                  icon: LucideIcons.messageCircle,
                  label: Copy.conversations,
                ),
                KimDockItem(
                  key: const Key('nav-contacts'),
                  icon: LucideIcons.users,
                  label: Copy.contacts,
                  badge: incoming,
                ),
                KimDockItem(
                  key: const Key('nav-me'),
                  icon: LucideIcons.user,
                  label: Copy.me,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
