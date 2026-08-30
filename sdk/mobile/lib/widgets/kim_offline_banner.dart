library;

import 'package:flutter/material.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../copy.dart';
import '../core/connectivity.dart';
import '../theme/motion.dart';

/// Slim offline strip. Driven by connectivity_plus, not a Dart socket.
class KimOfflineBanner extends StatelessWidget {
  const KimOfflineBanner({
    super.key,
    required this.connectivity,
    required this.child,
  });

  final KimConnectivity connectivity;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<bool>(
      valueListenable: connectivity.online,
      builder: (context, online, _) {
        final scheme = Theme.of(context).colorScheme;
        return Column(
          children: [
            AnimatedSize(
              duration: KimMotion.short,
              curve: KimMotion.standard,
              alignment: Alignment.topCenter,
              child: online
                  ? const SizedBox(width: double.infinity, height: 0)
                  : Material(
                      color: scheme.errorContainer,
                      child: SafeArea(
                        bottom: false,
                        child: Padding(
                          padding: const EdgeInsets.fromLTRB(16, 6, 16, 8),
                          child: Row(
                            children: [
                              Icon(
                                LucideIcons.wifiOff,
                                size: 16,
                                color: scheme.onErrorContainer,
                              ),
                              const Gap(8),
                              Expanded(
                                child: Text(
                                  Copy.offlineBanner,
                                  style: Theme.of(context).textTheme.bodySmall
                                      ?.copyWith(color: scheme.onErrorContainer),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
            ),
            Expanded(child: child),
          ],
        );
      },
    );
  }
}
