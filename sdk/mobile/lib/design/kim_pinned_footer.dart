library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/design/kim_theme.dart';

/// Always-visible primary action under a scrolling list.
class KimPinnedFooter extends StatelessWidget {
  const KimPinnedFooter({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: scheme.surface,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Divider(height: 1, color: KimTheme.hairlineOf(context)),
          SafeArea(
            top: false,
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 10, 16, 12),
              child: SizedBox(width: double.infinity, child: child),
            ),
          ),
        ],
      ),
    );
  }
}
