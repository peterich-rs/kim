library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../state/contacts.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/kim_dock.dart';

class HomeShell extends ConsumerWidget {
  const HomeShell({super.key, required this.navigationShell});

  final StatefulNavigationShell navigationShell;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final incoming = ref.watch(contactsProvider.select((s) => s.incomingCount));
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
              onSelected: (index) {
                KimHaptics.selection();
                navigationShell.goBranch(
                  index,
                  initialLocation: index == navigationShell.currentIndex,
                );
              },
              items: [
                const KimDockItem(
                  key: Key('nav-conversations'),
                  icon: LucideIcons.messageCircle,
                  label: Copy.conversations,
                ),
                KimDockItem(
                  key: const Key('nav-contacts'),
                  icon: LucideIcons.users,
                  label: Copy.contacts,
                  badge: incoming,
                ),
                const KimDockItem(
                  key: Key('nav-me'),
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
