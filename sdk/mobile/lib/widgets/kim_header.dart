library;

import 'package:flutter/material.dart';

/// Compact pinned header. Replaces [SliverAppBar.large], which left a tall
/// empty band above the list on every tab.
class KimSliverHeader extends StatelessWidget {
  const KimSliverHeader({super.key, required this.title, this.actions});

  final String title;
  final List<Widget>? actions;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SliverAppBar(
      pinned: true,
      titleSpacing: 16,
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
