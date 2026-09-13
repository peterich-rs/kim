library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import '../../copy.dart';
import '../../state/agent_profiles.dart';
import '../../state/provider_accounts.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';
import 'provider_account_page.dart';

class ProviderAccountsPage extends ConsumerStatefulWidget {
  const ProviderAccountsPage({super.key});

  @override
  ConsumerState<ProviderAccountsPage> createState() =>
      _ProviderAccountsPageState();
}

class _ProviderAccountsPageState extends ConsumerState<ProviderAccountsPage> {
  @override
  void initState() {
    super.initState();
    unawaited(ref.read(providerAccountsProvider.notifier).ensureLoaded());
  }

  int _refs(String accountId) {
    var n = 0;
    for (final p in ref.read(agentProfilesProvider)) {
      if (p.accountId == accountId) {
        n++;
      }
    }
    return n;
  }

  Future<void> _edit(ProviderAccount account) async {
    await openProviderAccountEditor(context, accountId: account.id);
  }

  Future<void> _add() async {
    await openProviderAccountEditor(context);
  }

  Future<void> _remove(ProviderAccount account) async {
    if (_refs(account.id) > 0) {
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: Text(Copy.agentAccountInUse),
        autoCloseDuration: const Duration(seconds: 3),
      );
      return;
    }
    await ref.read(providerAccountsProvider.notifier).delete(account.id);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final accounts = ref.watch(providerAccountsProvider);
    return Scaffold(
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(
            title: l10n.agentAccounts,
            actions: [
              IconButton(
                tooltip: l10n.agentAddAccount,
                onPressed: _add,
                icon: const Icon(Icons.add),
              ),
            ],
          ),
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            sliver: SliverList.list(
              children: [
                if (accounts.isEmpty)
                  EmptyState(
                    icon: LucideIcons.key,
                    title: l10n.agentEmptyProviders,
                    subtitle: l10n.agentEmptyProvidersHint,
                    action: FilledButton(
                      key: const Key('provider-add'),
                      onPressed: _add,
                      child: Text(l10n.agentAddAccount),
                    ),
                  )
                else
                  KimGroupCard(
                    children: [
                      for (final account in accounts) ...[
                        if (account != accounts.first) const Divider(height: 1),
                        ListTile(
                          title: Text(
                            account.displayName.isEmpty
                                ? account.vendorId
                                : account.displayName,
                          ),
                          subtitle: Text(
                            '${account.vendorId} · ${_refs(account.id)}',
                          ),
                          onTap: () => unawaited(_edit(account)),
                          trailing: IconButton(
                            tooltip: l10n.agentDelete,
                            onPressed: () => unawaited(_remove(account)),
                            icon: const Icon(Icons.delete_outline, size: 18),
                          ),
                        ),
                      ],
                    ],
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
