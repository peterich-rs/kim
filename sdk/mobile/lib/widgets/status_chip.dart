library;

import 'package:flutter/material.dart';
import 'package:gap/gap.dart';

import '../copy.dart';
import '../models/models.dart';

class StatusDot extends StatelessWidget {
  const StatusDot({super.key, required this.status, this.size = 8});

  final ConnStatus status;
  final double size;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final color = switch (status) {
      ConnStatus.online => const Color(0xFF34C759),
      ConnStatus.connecting || ConnStatus.reconnecting => scheme.tertiary,
      ConnStatus.offline => scheme.outline,
    };
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(color: color, shape: BoxShape.circle),
    );
  }
}

class ConnectionBanner extends StatelessWidget {
  const ConnectionBanner({
    super.key,
    required this.status,
    this.error,
    this.onRetry,
  });

  final ConnStatus status;
  final String? error;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    if (status == ConnStatus.online) {
      return const SizedBox.shrink();
    }
    final theme = Theme.of(context);
    final label =
        error ??
        switch (status) {
          ConnStatus.connecting => Copy.connecting,
          ConnStatus.reconnecting => Copy.reconnecting,
          ConnStatus.offline => Copy.offline,
          ConnStatus.online => Copy.online,
        };
    return Material(
      color: theme.colorScheme.surfaceContainerHighest,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        child: Row(
          children: [
            StatusDot(status: status),
            const Gap(8),
            Expanded(
              child: Text(
                label,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ),
            if (status == ConnStatus.offline && onRetry != null)
              TextButton(onPressed: onRetry, child: const Text(Copy.retry)),
          ],
        ),
      ),
    );
  }
}
