library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../state/contacts.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/kim_hairline.dart';

class HomeShell extends ConsumerWidget {
  const HomeShell({super.key, required this.navigationShell});

  final StatefulNavigationShell navigationShell;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final incoming = ref.watch(contactsProvider.select((s) => s.incomingCount));
    final wide = MediaQuery.sizeOf(context).width >= 900;
    final destinations = <NavigationDestination>[
      const NavigationDestination(
        icon: Icon(LucideIcons.messageCircle),
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
      const NavigationDestination(
        icon: Icon(LucideIcons.bot),
        label: Copy.agent,
      ),
      const NavigationDestination(
        icon: Icon(LucideIcons.user),
        label: Copy.me,
      ),
    ];

    void go(int index) {
      KimHaptics.selection();
      navigationShell.goBranch(
        index,
        initialLocation: index == navigationShell.currentIndex,
      );
    }

    if (wide) {
      return Scaffold(
        backgroundColor: KimTheme.canvasOf(context),
        body: Row(
          children: [
            NavigationRail(
              selectedIndex: navigationShell.currentIndex,
              onDestinationSelected: go,
              labelType: NavigationRailLabelType.all,
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
