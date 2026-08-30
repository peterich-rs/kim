library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../state/contacts.dart';

class HomeShell extends ConsumerWidget {
  const HomeShell({super.key, required this.navigationShell});

  final StatefulNavigationShell navigationShell;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final incoming = ref.watch(contactsProvider.select((s) => s.incomingCount));
    return Scaffold(
      body: navigationShell,
      bottomNavigationBar: NavigationBar(
        selectedIndex: navigationShell.currentIndex,
        onDestinationSelected: (index) {
          KimHaptics.selection();
          navigationShell.goBranch(
            index,
            initialLocation: index == navigationShell.currentIndex,
          );
        },
        destinations: [
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
            icon: Icon(LucideIcons.user),
            label: Copy.me,
          ),
        ],
      ),
    );
  }
}
