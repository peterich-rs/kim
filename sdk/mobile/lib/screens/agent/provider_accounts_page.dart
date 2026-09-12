library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:toastification/toastification.dart';

import '../../agent/catalog.dart';
import '../../copy.dart';
import '../../core/settings.dart';
import '../../state/agent_profiles.dart';
import '../../state/provider_accounts.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';

class ProviderAccountsPage extends ConsumerStatefulWidget {
  const ProviderAccountsPage({super.key});

  @override
  ConsumerState<ProviderAccountsPage> createState() =>
      _ProviderAccountsPageState();
}

class _ProviderAccountsPageState extends ConsumerState<ProviderAccountsPage> {
  List<VendorSummaryDto> _vendors = const [];

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    await ref.read(providerAccountsProvider.notifier).ensureLoaded();
    try {
      final vendors = await ref.read(catalogRepositoryProvider).ensureVendors();
      if (mounted) {
        setState(() => _vendors = vendors);
      }
    } catch (_) {}
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
    final result = await showModalBottomSheet<ProviderAccount>(
      context: context,
      isScrollControlled: true,
      builder: (ctx) => _AccountSheet(account: account, vendors: _vendors),
    );
    if (result == null) {
      return;
    }
    await ref.read(providerAccountsProvider.notifier).upsert(result);
  }

  Future<void> _add() async {
    final id = 'acct-${DateTime.now().millisecondsSinceEpoch}';
    final created = ProviderAccount(
      id: id,
      vendorId: 'openai',
      baseUrl: 'https://api.openai.com/v1',
      keyRef: '$kAccountKeyPrefix$id',
      displayName: 'OpenAI',
    );
    await _edit(created);
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
                KimGroupCard(
                  children: [
                    if (accounts.isEmpty)
                      ListTile(title: Text(l10n.agentAddAccount))
                    else
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

class _AccountSheet extends StatefulWidget {
  const _AccountSheet({required this.account, required this.vendors});

  final ProviderAccount account;
  final List<VendorSummaryDto> vendors;

  @override
  State<_AccountSheet> createState() => _AccountSheetState();
}

class _AccountSheetState extends State<_AccountSheet> {
  late final TextEditingController _name;
  late final TextEditingController _url;
  late final TextEditingController _key;
  late String _vendor;
  final _secure = SettingsStore.productionSecureStorage();

  @override
  void initState() {
    super.initState();
    _name = TextEditingController(text: widget.account.displayName);
    _url = TextEditingController(text: widget.account.baseUrl);
    _key = TextEditingController();
    _vendor = widget.account.vendorId;
    unawaited(_loadKey());
  }

  Future<void> _loadKey() async {
    try {
      final v = await _secure.read(key: widget.account.keyRef);
      if (mounted && v != null && v.isNotEmpty) {
        _key.text = v;
      }
    } catch (_) {}
  }

  @override
  void dispose() {
    _name.dispose();
    _url.dispose();
    _key.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    if (_vendor == 'openai_compatible' && !isAllowedAgentBaseUrl(_url.text)) {
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: Text(Copy.agentInvalidUrl),
        autoCloseDuration: const Duration(seconds: 3),
      );
      return;
    }
    final account = widget.account.copyWith(
      vendorId: canonicalizeVendorId(_vendor),
      baseUrl: _url.text.trim(),
      displayName: _name.text.trim().isEmpty ? _vendor : _name.text.trim(),
    );
    try {
      if (_key.text.trim().isEmpty) {
        await _secure.delete(key: account.keyRef);
      } else {
        await _secure.write(key: account.keyRef, value: _key.text.trim());
      }
    } catch (_) {}
    if (mounted) {
      Navigator.of(context).pop(account);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final inset = MediaQuery.viewInsetsOf(context).bottom;
    final vendorIds = {for (final v in widget.vendors) v.id};
    if (!vendorIds.contains(_vendor)) {
      vendorIds.add(_vendor);
    }
    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + inset),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          DropdownButton<String>(
            value: _vendor,
            isExpanded: true,
            items: [
              for (final v in sortVendors(widget.vendors))
                DropdownMenuItem(value: v.id, child: Text(v.displayName)),
              if (widget.vendors.every((v) => v.id != _vendor))
                DropdownMenuItem(value: _vendor, child: Text(_vendor)),
            ],
            onChanged: (next) {
              if (next == null) {
                return;
              }
              VendorSummaryDto? hit;
              for (final v in widget.vendors) {
                if (v.id == next) {
                  hit = v;
                  break;
                }
              }
              setState(() {
                _vendor = next;
                if (hit != null && hit.defaultBaseUrl.isNotEmpty) {
                  _url.text = hit.defaultBaseUrl;
                }
              });
            },
          ),
          TextField(
            controller: _name,
            decoration: InputDecoration(labelText: l10n.agentDisplayName),
          ),
          TextField(
            controller: _url,
            decoration: InputDecoration(labelText: Copy.agentBaseUrl),
          ),
          TextField(
            controller: _key,
            obscureText: true,
            decoration: InputDecoration(labelText: Copy.agentApiKey),
          ),
          const Gap(12),
          FilledButton(onPressed: _save, child: Text(Copy.save)),
        ],
      ),
    );
  }
}
