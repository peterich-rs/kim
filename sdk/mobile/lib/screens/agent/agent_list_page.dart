library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:toastification/toastification.dart';

import '../../agent/mention.dart';
import '../../copy.dart';
import '../../state/agent_profiles.dart';
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
                KimGroupCard(
                  children: [
                    SwitchListTile(
                      title: Text(l10n.agentServerIdentity),
                      subtitle: Text(() {
                        if (store.identityError != null) {
                          return store.identityError!;
                        }
                        final acc = store.goose?.serverAccount ?? '';
                        if (acc.isNotEmpty) {
                          return acc;
                        }
                        return l10n.agentServerIdentityHint;
                      }()),
                      value: store.serverIdentity,
                      onChanged: (next) =>
                          unawaited(store.setServerIdentity(next)),
                    ),
                    const Divider(height: 1),
                    SwitchListTile(
                      title: Text(l10n.agentMultiProfile),
                      value: store.multiProfile,
                      onChanged: (next) =>
                          unawaited(store.setMultiProfile(next)),
                    ),
                  ],
                ),
                const Gap(18),
                KimGroupCard(
                  children: [
                    for (final profile in profiles) ...[
                      if (profile != profiles.first) const Divider(height: 1),
                      ListTile(
                        title: Text(profile.displayName),
                        subtitle: Text(
                          profile.serverAccount.isEmpty
                              ? profile.id
                              : '${profile.id} · ${profile.serverAccount}',
                        ),
                        onTap: () => context.push('/agent/${profile.id}'),
                        trailing: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            if (profile.id != kGooseAgentId)
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
                            if (profile.id != kGooseAgentId)
                              IconButton(
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
                  Copy.agentGooseHint,
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
