library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../copy.dart';
import '../models/models.dart';
import '../state/providers.dart';
import '../state/session.dart';
import '../theme/motion.dart';

/// App-wide offline strip. Radio from connectivity_plus; socket from session.
class KimOfflineBanner extends ConsumerWidget {
  const KimOfflineBanner({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final radio = ref.watch(runtimeProvider).connectivity;
    final session = ref.watch(sessionProvider);
    return ValueListenableBuilder<bool>(
      valueListenable: radio.online,
      builder: (context, linkUp, _) {
        final noRadio = !linkUp;
        final noSocket =
            session.signedIn && session.status == ConnStatus.offline;
        final show = noRadio || noSocket;
        final scheme = Theme.of(context).colorScheme;
        final label = noRadio ? Copy.offlineBanner : Copy.offline;
        return Column(
          children: [
            AnimatedSize(
              duration: KimMotion.short,
              curve: KimMotion.standard,
              alignment: Alignment.topCenter,
              child: show
                  ? Material(
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
                                  label,
                                  style: Theme.of(context).textTheme.bodySmall
                                      ?.copyWith(color: scheme.onErrorContainer),
                                ),
                              ),
                              if (noSocket && !noRadio)
                                TextButton(
                                  onPressed: () => ref
                                      .read(sessionProvider.notifier)
                                      .connect(),
                                  child: Text(
                                    Copy.retry,
                                    style: TextStyle(
                                      color: scheme.onErrorContainer,
                                    ),
                                  ),
                                ),
                            ],
                          ),
                        ),
                      ),
                    )
                  : const SizedBox(width: double.infinity, height: 0),
            ),
            Expanded(child: child),
          ],
        );
      },
    );
  }
}
