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
import '../../widgets/kim_hairline.dart';

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
      body: navigationShell,
      bottomNavigationBar: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const KimHairline(),
          NavigationBar(
            selectedIndex: navigationShell.currentIndex,
            onDestinationSelected: go,
            destinations: destinations,
          ),
        ],
      ),
    );
  }
}
