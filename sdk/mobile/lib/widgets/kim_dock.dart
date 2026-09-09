/// Floating iOS-style pill tab bar. Icon-only; overlays tab content.
library;

import 'dart:ui';

import 'package:flutter/material.dart';

import '../theme/kim_theme.dart';

class KimDockItem {
  const KimDockItem({
    this.key,
    required this.icon,
    required this.label,
    this.badge = 0,
  });

  final Key? key;
  final IconData icon;
  final String label;
  final int badge;
}

class KimDock extends StatelessWidget {
  const KimDock({
    super.key,
    required this.selectedIndex,
    required this.onSelected,
    required this.items,
  });

  static const double pillHeight = 56;
  static const double minBottomGap = 12;
  static const double _itemExtent = 56;
  static const double _iconSize = 22;

  /// Space the floating pill occupies, including the home-indicator inset.
  /// Tab lists should pad by this so the last row can scroll clear of the dock.
  static double overlapOf(BuildContext context) {
    final bottom = MediaQuery.viewPaddingOf(context).bottom;
    return pillHeight + (bottom > minBottomGap ? bottom : minBottomGap);
  }

  final int selectedIndex;
  final ValueChanged<int> onSelected;
  final List<KimDockItem> items;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final dark = theme.brightness == Brightness.dark;
    final fill = (dark ? const Color(0xFF1C1C1E) : const Color(0xFFFFFFFF))
        .withValues(alpha: dark ? 0.82 : 0.94);
    final border = dark
        ? Colors.white.withValues(alpha: 0.10)
        : Colors.black.withValues(alpha: 0.06);
    final radius = BorderRadius.circular(999);

    return SafeArea(
      top: false,
      minimum: const EdgeInsets.only(bottom: minBottomGap),
      child: Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 24),
          child: DecoratedBox(
            decoration: BoxDecoration(
              borderRadius: radius,
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: dark ? 0.50 : 0.14),
                  blurRadius: 28,
                  offset: const Offset(0, 10),
                  spreadRadius: -4,
                ),
                BoxShadow(
                  color: Colors.black.withValues(alpha: dark ? 0.28 : 0.06),
                  blurRadius: 8,
                  offset: const Offset(0, 2),
                ),
              ],
            ),
            child: ClipRRect(
              borderRadius: radius,
              child: BackdropFilter(
                filter: ImageFilter.blur(sigmaX: 28, sigmaY: 28),
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    color: fill,
                    borderRadius: radius,
                    border: Border.all(color: border, width: 0.5),
                  ),
                  child: Material(
                    type: MaterialType.transparency,
                    child: SizedBox(
                      height: pillHeight,
                      child: Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 6),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            for (var i = 0; i < items.length; i++)
                              _DockButton(
                                item: items[i],
                                selected: i == selectedIndex,
                                onTap: () => onSelected(i),
                                color: i == selectedIndex
                                    ? scheme.onSurface
                                    : scheme.onSurface.withValues(alpha: 0.36),
                              ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _DockButton extends StatelessWidget {
  const _DockButton({
    required this.item,
    required this.selected,
    required this.onTap,
    required this.color,
  });

  final KimDockItem item;
  final bool selected;
  final VoidCallback onTap;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final icon = Icon(item.icon, size: KimDock._iconSize, color: color);
    return Semantics(
      button: true,
      selected: selected,
      label: item.label,
      child: Tooltip(
        message: item.label,
        child: SizedBox(
          key: item.key,
          width: KimDock._itemExtent,
          height: KimDock.pillHeight,
          child: InkWell(
            customBorder: const CircleBorder(),
            onTap: onTap,
            child: Center(
              child: AnimatedScale(
                scale: selected ? 1.06 : 1.0,
                duration: KimTheme.motionFast,
                curve: KimTheme.motionEmphasized,
                child: item.badge > 0
                    ? Badge(
                        label: Text(item.badge > 9 ? '9+' : '${item.badge}'),
                        child: icon,
                      )
                    : icon,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
