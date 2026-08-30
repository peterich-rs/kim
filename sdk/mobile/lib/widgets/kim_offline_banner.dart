library;

import 'package:flutter/material.dart';

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
                      child: Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 12,
                          vertical: 8,
                        ),
                        child: Row(
                          children: [
                            Icon(
                              Icons.cloud_off,
                              size: 16,
                              color: scheme.onErrorContainer,
                            ),
                            const SizedBox(width: 8),
                            Expanded(
                              child: Text(
                                'Offline — WGateway traffic waits until a network is back.',
                                style: Theme.of(context).textTheme.bodySmall
                                    ?.copyWith(color: scheme.onErrorContainer),
                              ),
                            ),
                          ],
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
