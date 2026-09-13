library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import '../../agent/mention.dart';
import '../../copy.dart';
import '../../core/haptics.dart';
import '../../state/agent_profiles.dart';
import '../../state/provider_accounts.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';

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
                key: const Key('agent-new'),
                tooltip: l10n.agentCreate,
                onPressed: () {
                  unawaited(KimHaptics.light());
                  context.push('/agent/new');
                },
                icon: const Icon(Icons.add),
              ),
              IconButton(
                tooltip: l10n.agentAccounts,
                onPressed: () => context.push('/agent/accounts'),
                icon: const Icon(Icons.key_outlined),
              ),
            ],
          ),
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            sliver: SliverList.list(
              children: [
                if (profiles.isEmpty)
                  EmptyState(
                    icon: LucideIcons.bot,
                    title: l10n.agentEmptyTitle,
                    subtitle: l10n.agentEmptyHint,
                    action: FilledButton(
                      key: const Key('agent-create'),
                      onPressed: () => context.push('/agent/new'),
                      child: Text(l10n.agentCreate),
                    ),
                  )
                else
                  KimGroupCard(
                    children: [
                      for (final profile in profiles) ...[
                        if (profile != profiles.first) const Divider(height: 1),
                        ListTile(
                          title: Text(profile.displayName),
                          subtitle: Text(_profileSubtitle(profile, accounts)),
                          onTap: () => context.push('/agent/${profile.id}'),
                          trailing: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Switch(
                                value: profile.enabled,
                                onChanged: (next) => unawaited(
                                  store.setEnabled(profile.id, next),
                                ),
                              ),
                              IconButton(
                                tooltip: l10n.agentDuplicate,
                                onPressed: () => unawaited(
                                  _duplicate(context, store, profile),
                                ),
                                icon: const Icon(Icons.copy, size: 18),
                              ),
                              IconButton(
                                key: Key('agent-delete-${profile.id}'),
                                tooltip: l10n.agentDelete,
                                onPressed: () => unawaited(
                                  _delete(context, store, profile.id),
                                ),
                                icon: const Icon(
                                  Icons.delete_outline,
                                  size: 18,
                                ),
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
