library;

import 'package:flutter/material.dart';

import '../theme/motion.dart';

/// Blocks pointer events and fades in a scrim + spinner while [busy].
class KimBusyBarrier extends StatelessWidget {
  const KimBusyBarrier({super.key, required this.busy, required this.child});

  final bool busy;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        IgnorePointer(ignoring: busy, child: child),
        Positioned.fill(
          child: AnimatedSwitcher(
            duration: KimMotion.short,
            switchInCurve: KimMotion.enter,
            switchOutCurve: KimMotion.exit,
            child: busy
                ? ColoredBox(
                    key: const ValueKey('busy'),
                    color: Theme.of(context).colorScheme.scrim
                        .withValues(alpha: 0.24),
                    child: const Center(child: CircularProgressIndicator()),
                  )
                : const SizedBox.shrink(key: ValueKey('idle')),
          ),
        ),
      ],
    );
  }
}
