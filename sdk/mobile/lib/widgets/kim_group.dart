library;

import 'package:flutter/material.dart';

import '../theme/kim_theme.dart';

/// Raised, opaque group with a hairline. Replaces alpha-washed cards.
class KimGroupCard extends StatelessWidget {
  const KimGroupCard({super.key, required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: KimTheme.raisedOf(context),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(KimTheme.radiusCard),
        side: BorderSide(color: scheme.outlineVariant.withValues(alpha: 0.7)),
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(children: children),
    );
  }
}
