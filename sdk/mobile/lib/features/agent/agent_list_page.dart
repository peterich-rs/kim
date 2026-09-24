library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/haptics.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/design/empty_state.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/design/kim_header.dart';
import 'package:kim_mobile/router/app_routes.dart';

class AgentListPage extends ConsumerWidget {
  const AgentListPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final store = ref.watch(agentProfilesProvider.notifier);
    ref.watch(agentProfilesProvider);
    final accounts = ref.watch(providerAccountsProvider);
    final profiles = store.multiProfile
        ? ref.watch(agentProfilesProvider)
        : [
            for (final p in ref.watch(agentProfilesProvider))
              if (p.id == kGooseAgentId) p,
          ];

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(
            title: l10n.agentListTitle,
            actions: [
              IconButton(
                key: const Key('agent-plaza'),
                tooltip: l10n.agentPlazaTitle,
                onPressed: () {
                  unawaited(KimHaptics.light());
                  context.push(AppRoutes.agentPlaza);
                },
                icon: Icon(LucideIcons.store, color: scheme.onSurfaceVariant),
              ),
              IconButton(
                tooltip: l10n.agentAccounts,
                onPressed: () => context.push(AppRoutes.agentAccounts),
                icon: Icon(LucideIcons.key, color: scheme.onSurfaceVariant),
              ),
              IconButton(
                key: const Key('agent-new'),
                tooltip: l10n.agentCreate,
                onPressed: () {
                  unawaited(KimHaptics.light());
                  context.push(AppRoutes.agentNew);
                },
                icon: const Icon(Icons.add),
              ),
            ],
          ),
          KimBodySliver(
            sliver: SliverList.list(
              children: [
                if (profiles.isEmpty)
                  EmptyState(
                    icon: LucideIcons.bot,
                    title: l10n.agentEmptyTitle,
                    subtitle: l10n.agentEmptyHint,
                    action: FilledButton(
                      key: const Key('agent-create'),
                      onPressed: () => context.push(AppRoutes.agentNew),
                      child: Text(l10n.agentCreate),
                    ),
                  )
                else
                  KimGroupCard(
                    children: [
                      for (final profile in profiles) ...[
                        if (profile != profiles.first) const Divider(height: 1),
                        ListTile(
                          contentPadding: const EdgeInsets.fromLTRB(
                            16,
                            6,
                            8,
                            6,
                          ),
                          leading: CircleAvatar(
                            backgroundColor: scheme.primary.withValues(
                              alpha: 0.12,
                            ),
                            foregroundColor: scheme.primary,
                            child: const Icon(LucideIcons.bot, size: 18),
                          ),
                          title: Text(profile.displayName),
                          subtitle: Text(
                            _profileSubtitle(profile, accounts),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          onTap: () =>
                              context.push(AppRoutes.agentProfile(profile.id)),
                          trailing: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Switch.adaptive(
                                value: profile.enabled,
                                onChanged: (next) => unawaited(
                                  store.setEnabled(profile.id, next),
                                ),
                              ),
                              PopupMenuButton<String>(
                                key: Key('agent-row-menu-${profile.id}'),
                                tooltip: MaterialLocalizations.of(context)
                                    .moreButtonTooltip,
                                onSelected: (value) {
                                  switch (value) {
                                    case 'duplicate':
                                      unawaited(
                                        _duplicate(context, store, profile),
                                      );
                                    case 'delete':
                                      unawaited(
                                        _delete(context, store, profile.id),
                                      );
                                  }
                                },
                                itemBuilder: (ctx) => [
                                  PopupMenuItem(
                                    value: 'duplicate',
                                    child: Text(l10n.agentDuplicate),
                                  ),
                                  PopupMenuItem(
                                    key: Key('agent-delete-${profile.id}'),
                                    value: 'delete',
                                    child: Text(
                                      l10n.agentDelete,
                                      style: TextStyle(color: scheme.error),
                                    ),
                                  ),
                                ],
                              ),
                            ],
                          ),
                        ),
                      ],
                    ],
                  ),
                const Gap(12),
                Text(
                  l10n.agentListHint,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

String _profileSubtitle(AgentProfile profile, List<ProviderAccount> accounts) {
  ProviderAccount? account;
  for (final a in accounts) {
    if (a.id == profile.accountId) {
      account = a;
      break;
    }
  }
  final provider = (account != null && account.displayName.isNotEmpty)
      ? account.displayName
      : (account != null && account.vendorId.isNotEmpty)
      ? account.vendorId
      : profile.providerKind;
  final bits = <String>[
    if (provider.isNotEmpty) provider,
    if (profile.model.isNotEmpty) profile.model,
  ];
  return bits.join(' · ');
}

Future<void> _duplicate(
  BuildContext context,
  AgentProfileStore store,
  AgentProfile profile,
) async {
  try {
    await store.duplicate(profile);
  } catch (err) {
    if (!context.mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.error,
      title: Text(err.toString()),
      autoCloseDuration: const Duration(seconds: 3),
    );
  }
}

Future<void> _delete(
  BuildContext context,
  AgentProfileStore store,
  String id,
) async {
  try {
    await store.delete(id);
  } catch (err) {
    if (!context.mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.error,
      title: Text(agentRegisterError(err)),
      autoCloseDuration: const Duration(seconds: 3),
    );
  }
}
