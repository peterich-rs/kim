library;

import 'package:flutter/material.dart';
import 'package:gap/gap.dart';

import '../theme/kim_theme.dart';

class EmptyState extends StatelessWidget {
  const EmptyState({
    super.key,
    required this.icon,
    required this.title,
    required this.subtitle,
    this.action,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 40),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 72,
              height: 72,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: scheme.primary.withValues(alpha: 0.12),
                borderRadius: BorderRadius.circular(KimTheme.radiusCard),
              ),
              child: Icon(icon, size: 32, color: scheme.primary),
            ),
            Gap(KimTheme.spaceUnit * 4),
            Text(
              title,
              textAlign: TextAlign.center,
              style: theme.textTheme.titleMedium?.copyWith(
                fontWeight: FontWeight.w600,
                fontSize: KimTheme.fontTitle,
              ),
            ),
            Gap(KimTheme.spaceUnit * 1.5),
            Text(
              subtitle,
              textAlign: TextAlign.center,
              style: theme.textTheme.bodyMedium?.copyWith(
                color: scheme.onSurfaceVariant,
                fontSize: KimTheme.fontBody,
                height: 1.4,
              ),
            ),
            if (action != null) ...[Gap(KimTheme.spaceUnit * 5), action!],
          ],
        ),
      ),
    );
  }
}
