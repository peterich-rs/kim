library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/experimental/mutation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../models/models.dart';
import '../../state/auth.dart';
import '../../state/gateway.dart';
import '../../state/mutations.dart';
import '../../state/providers.dart';
import '../../state/session.dart';
import '../../widgets/kim_avatar.dart';
import '../../widgets/kim_header.dart';
import '../../widgets/status_chip.dart';

class MePage extends ConsumerWidget {
  const MePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final session = ref.watch(sessionProvider);
    final runtime = ref.watch(runtimeProvider);
    final logout = ref.watch(signOutMutation);
    final settings = runtime.settings;
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final local = settings.httpOrigin.contains('127.0.0.1');
    final loggingOut = logout is MutationPending;

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          const KimSliverHeader(title: Copy.me),
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: scheme.surfaceContainerHighest.withValues(alpha: 0.55),
                  borderRadius: BorderRadius.circular(24),
                ),
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(18, 20, 18, 20),
                  child: Row(
                    children: [
                      KimAvatar(
                        name: session.account.isEmpty ? '?' : session.account,
                        size: KimAvatarSize.lg,
                      ),
                      const Gap(16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              session.account,
                              style: theme.textTheme.headlineSmall?.copyWith(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            const Gap(6),
                            Row(
                              children: [
                                StatusDot(status: session.status),
                                const Gap(6),
                                Text(
                                  session.statusLabel,
                                  style: theme.textTheme.bodyMedium?.copyWith(
                                    color: scheme.onSurfaceVariant,
                                  ),
                                ),
                              ],
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
          SliverPadding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            sliver: SliverList.list(
              children: [
                _SectionLabel(Copy.accountSection),
                _Card(
                  children: [
                    ListTile(
                      leading: const Icon(LucideIcons.lock),
                      title: const Text(Copy.changePassword),
                      trailing: const Icon(LucideIcons.chevronRight, size: 18),
                      onTap: () => context.push('/password'),
                    ),
                    const Divider(indent: 56),
                    ListTile(
                      key: const Key('logout'),
                      leading: Icon(LucideIcons.logOut, color: scheme.error),
                      title: Text(
                        Copy.logout,
                        style: TextStyle(color: scheme.error),
                      ),
                      onTap: loggingOut
                          ? null
                          : () {
                              signOutMutation.run(ref, (tsx) async {
                                await tsx.get(authProvider.notifier).signOut();
                              });
                            },
                    ),
                  ],
                ),
                const Gap(18),
                _SectionLabel(Copy.generalSection),
                _Card(
                  children: [
                    ListTile(
                      leading: const Icon(LucideIcons.server),
                      title: const Text(Copy.environment),
                      subtitle: Text(
                        local ? Copy.localServer : Copy.prodServer,
                      ),
                    ),
                    Padding(
                      padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
                      child: SegmentedButton<bool>(
                        segments: const [
                          ButtonSegment(
                            value: false,
                            label: Text(Copy.prodServer),
                          ),
                          ButtonSegment(
                            value: true,
                            label: Text(Copy.localServer),
                          ),
                        ],
                        selected: {local},
                        onSelectionChanged: (next) async {
                          KimHaptics.selection();
                          if (next.first) {
                            await settings.useLocal();
                          } else {
                            await settings.useProd();
                          }
                          ref.invalidate(gatewayProvider);
                        },
                      ),
                    ),
                    const Divider(indent: 56),
                    ListTile(
                      leading: const Icon(LucideIcons.info),
                      title: const Text(Copy.about),
                      subtitle: Text('${Copy.brand} ${runtime.versionLabel}'),
                    ),
                    if (session.status != ConnStatus.online)
                      ListTile(
                        leading: const Icon(LucideIcons.refreshCw),
                        title: const Text(Copy.retry),
                        subtitle: Text(session.connectError ?? Copy.offline),
                        onTap: () => ref.invalidate(gatewayProvider),
                      ),
                  ],
                ),
                const Gap(48),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(4, 0, 4, 8),
      child: Text(
        text,
        style: Theme.of(context).textTheme.labelLarge
            ?.copyWith(color: Theme.of(context).colorScheme.onSurfaceVariant),
      ),
    );
  }
}

class _Card extends StatelessWidget {
  const _Card({required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Theme.of(context).colorScheme.surfaceContainerHighest
          .withValues(alpha: 0.45),
      borderRadius: BorderRadius.circular(20),
      clipBehavior: Clip.antiAlias,
      child: Column(children: children),
    );
  }
}
