library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/core/layout.dart';

/// Back control for overlay pages. Null on tab roots.
Widget? kimHeaderLeading(BuildContext context) {
  if (!kimShowsBack(context)) {
    return null;
  }
  return IconButton(
    key: const Key('kim-back'),
    tooltip: MaterialLocalizations.of(context).backButtonTooltip,
    onPressed: () => kimLeaveOverlay(context),
    icon: const Icon(Icons.arrow_back),
  );
}

/// Compact pinned header. Replaces [SliverAppBar.large], which left a tall
/// empty band above the list on every tab.
class KimSliverHeader extends StatelessWidget {
  const KimSliverHeader({super.key, required this.title, this.actions});

  final String title;
  final List<Widget>? actions;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final leading = kimHeaderLeading(context);
    return SliverAppBar(
      pinned: true,
      automaticallyImplyLeading: false,
      leading: leading,
      titleSpacing: leading == null ? 16 : 0,
      toolbarHeight: 52,
      title: Text(
        title,
        style: theme.textTheme.titleLarge?.copyWith(
          fontWeight: FontWeight.w700,
          letterSpacing: -0.2,
        ),
      ),
      actions: actions,
    );
  }
}
