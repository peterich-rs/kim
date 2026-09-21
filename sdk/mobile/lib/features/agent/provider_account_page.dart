library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/features/agent/catalog.dart';
import 'package:kim_mobile/features/agent/provider_account_form.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/core/settings.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/design/kim_header.dart';
import 'package:kim_mobile/design/kim_pinned_footer.dart';

/// Full-screen create/edit. Prefer this over a sheet on phones.
Future<String?> openProviderAccountEditor(
  BuildContext context, {
  String? accountId,
}) {
  final path = accountId == null || accountId.isEmpty
      ? '/agent/accounts/new'
      : '/agent/accounts/$accountId';
  final router = GoRouter.maybeOf(context);
  if (router != null) {
    return router.push<String>(path);
  }
  return Navigator.of(context).push<String>(
    MaterialPageRoute(
      builder: (_) => ProviderAccountPage(accountId: accountId),
    ),
  );
}

class ProviderAccountPage extends ConsumerStatefulWidget {
  const ProviderAccountPage({super.key, this.accountId});

  final String? accountId;

  bool get isCreate => accountId == null || accountId!.isEmpty;

  @override
  ConsumerState<ProviderAccountPage> createState() =>
      _ProviderAccountPageState();
}

class _ProviderAccountPageState extends ConsumerState<ProviderAccountPage> {
  late final TextEditingController _name;
  late final TextEditingController _url;
  late final TextEditingController _key;
  final _secure = SettingsStore.productionSecureStorage();

  ProviderAccountDraft get _draft => ref.read(providerAccountFormProvider);

  ProviderAccountForm get _form =>
      ref.read(providerAccountFormProvider.notifier);

  @override
  void initState() {
    super.initState();
    _name = TextEditingController();
    _url = TextEditingController();
    _key = TextEditingController();
    unawaited(_load());
  }

  @override
  void dispose() {
    _name.dispose();
    _url.dispose();
    _key.dispose();
    super.dispose();
  }

  ProviderAccount? get _existing {
    final id = widget.accountId;
    if (id == null || id.isEmpty) {
      return null;
    }
    return ref.read(providerAccountsProvider.notifier).byId(id);
  }

  VendorSummaryDto? _vendorSummaryOf(ProviderAccountDraft draft) {
    for (final v in draft.vendors) {
      if (v.id == draft.vendor) {
        return v;
      }
    }
    return null;
  }

  Future<void> _load() async {
    await ref.read(providerAccountsProvider.notifier).ensureLoaded();
    try {
      final vendors = await ref.read(catalogRepositoryProvider).ensureVendors();
      if (mounted) {
        _form.setVendors(vendors);
      }
    } catch (_) {}
    if (!mounted || _draft.hydrated) {
      return;
    }
    final existing = _existing;
    var vendor = _draft.vendor;
    var models = _draft.models;
    if (existing != null) {
      _name.text = existing.displayName;
      _url.text = existing.baseUrl;
      vendor = existing.vendorId;
      models = List<String>.from(existing.models);
      try {
        final v = await _secure.read(key: existing.keyRef);
        if (mounted && v != null && v.isNotEmpty) {
          _key.text = v;
        }
      } catch (_) {}
    } else {
      final summary = _vendorSummaryOf(_draft);
      if (summary != null && summary.defaultBaseUrl.isNotEmpty) {
        _url.text = summary.defaultBaseUrl;
      }
      _name.text = summary?.displayName ?? 'OpenAI';
    }
    if (mounted) {
      _form.applyHydrated(vendor: vendor, models: models);
    }
    await _seedModels();
  }

  Future<void> _seedModels() async {
    final draft = _draft;
    if (draft.models.isNotEmpty) {
      return;
    }
    final vendor = _vendorSummaryOf(draft);
    final models = await migrateAccountModelIds(
      vendorId: draft.vendor,
      existing: draft.models,
      catalogModels: vendor?.models ?? const [],
      defaultModel: vendor?.defaultModel ?? '',
    );
    if (!mounted || models.isEmpty) {
      return;
    }
    _form.setModels(models);
    final existing = _existing;
    if (existing == null) {
      return;
    }
    await ref
        .read(providerAccountsProvider.notifier)
        .upsert(existing.copyWith(models: models));
  }

  Future<void> _refreshModels() async {
    if (!isAllowedAgentBaseUrl(_url.text)) {
      _toast(Copy.agentInvalidUrl, error: true);
      return;
    }
    if (_key.text.trim().isEmpty) {
      _toast(Copy.agentKeyMissing, error: true);
      return;
    }
    _form.setFetching(true);
    final draft = _draft;
    final original = List<String>.from(draft.models);
    try {
      final bridge = ref.read(agentBridgeProvider);
      await bridge.ensure();
      final list = await bridge.fetchModels(
        SessionOpenOpts(
          model: draft.models.isNotEmpty ? draft.models.first : '',
          llmBackend: canonicalizeVendorId(draft.vendor),
          resumeOnOpen: false,
          baseUrl: _url.text.trim(),
          apiKey: _key.text.trim(),
          enableFsTools: false,
          bashEnabled: false,
          profileId: '',
          profileJson: '',
          thinkingEffort: '',
          gooseMode: '',
          enableKimTools: false,
          enableApprovals: false,
          sessionId: '',
          harnessJson: '',
        ),
      );
      if (!mounted) {
        return;
      }
      final models = unionAccountModels(list, original);
      if (models.isEmpty) {
        throw StateError(Copy.agentFetchModelsFailed('empty'));
      }
      _form.setModels(models);
      final existing = _existing;
      if (existing != null) {
        await ref
            .read(providerAccountsProvider.notifier)
            .upsert(
              existing.copyWith(
                vendorId: canonicalizeVendorId(draft.vendor),
                baseUrl: _url.text.trim(),
                displayName: _name.text.trim().isEmpty
                    ? draft.vendor
                    : _name.text.trim(),
                models: models,
              ),
            );
      }
      if (!mounted) {
        return;
      }
      toastification.show(
        context: context,
        type: ToastificationType.success,
        title: Text(Copy.agentFetchModelsOk(models.length)),
        autoCloseDuration: const Duration(seconds: 2),
      );
    } catch (err) {
      if (!mounted) {
        return;
      }
      _form.setModels(original);
      _toast(Copy.agentFetchModelsFailed(err.toString()), error: true);
    } finally {
      if (mounted) {
        _form.setFetching(false);
      }
    }
  }

  void _toast(String message, {required bool error}) {
    toastification.show(
      context: context,
      type: error ? ToastificationType.error : ToastificationType.success,
      title: Text(message),
      autoCloseDuration: const Duration(seconds: 4),
    );
  }

  Future<void> _save() async {
    if (widget.isCreate && _key.text.trim().isEmpty) {
      _toast(Copy.agentKeyMissing, error: true);
      return;
    }
    if (!isAllowedAgentBaseUrl(_url.text)) {
      _toast(Copy.agentInvalidUrl, error: true);
      return;
    }
    final draft = _draft;
    final vendorId = canonicalizeVendorId(draft.vendor);
    final vendor = _vendorSummaryOf(draft);
    final existingId = widget.accountId;
    final id = widget.isCreate || existingId == null || existingId.isEmpty
        ? 'acct-${DateTime.now().microsecondsSinceEpoch}'
        : existingId;
    final account = ProviderAccount(
      id: id,
      vendorId: vendorId,
      baseUrl: _url.text.trim(),
      keyRef: widget.isCreate
          ? '$kAccountKeyPrefix$id'
          : (_existing?.keyRef ?? '$kAccountKeyPrefix$id'),
      displayName: _name.text.trim().isEmpty
          ? (vendor?.displayName ?? vendorId)
          : _name.text.trim(),
      models: draft.models,
    );
    await ref.read(providerAccountsProvider.notifier).upsert(account);
    final key = _key.text.trim();
    try {
      if (key.isNotEmpty) {
        await _secure.write(key: account.keyRef, value: key);
      }
    } catch (_) {}
    if (!mounted) {
      return;
    }
    if (Navigator.of(context).canPop()) {
      Navigator.of(context).pop(account.id);
    }
  }

  @override
  Widget build(BuildContext context) {
    final draft = ref.watch(providerAccountFormProvider);
    final l10n = AppLocalizations.of(context);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final vendor = _vendorSummaryOf(draft);
    final urlChoices = <String>{
      if (vendor != null && vendor.defaultBaseUrl.isNotEmpty)
        vendor.defaultBaseUrl,
      if (vendor != null) ...vendor.altBaseUrls,
    }.toList();
    final title = widget.isCreate
        ? l10n.agentAddAccount
        : (_name.text.isEmpty ? l10n.agentAccounts : _name.text);

    return Scaffold(
      bottomNavigationBar: KimPinnedFooter(
        child: FilledButton(
          key: const Key('provider-save'),
          onPressed: _save,
          child: Text(Copy.save),
        ),
      ),
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(title: title),
          KimBodySliver(
            sliver: SliverList.list(
              children: [
                Text(
                  l10n.agentProvider,
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    Padding(
                      padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
                      child: DropdownButton<String>(
                        key: const Key('provider-vendor'),
                        value: () {
                          final ids = {for (final v in draft.vendors) v.id};
                          if (ids.contains(draft.vendor) ||
                              draft.vendors.isEmpty) {
                            return draft.vendor;
                          }
                          return draft.vendors.first.id;
                        }(),
                        isExpanded: true,
                        items: [
                          for (final v in sortVendors(draft.vendors))
                            DropdownMenuItem(
                              value: v.id,
                              child: Text(v.displayName),
                            ),
                          if (draft.vendors.every(
                                (v) => v.id != draft.vendor,
                              ) &&
                              draft.vendor.isNotEmpty)
                            DropdownMenuItem(
                              value: draft.vendor,
                              child: Text(draft.vendor),
                            ),
                        ],
                        onChanged: (next) {
                          if (next == null) {
                            return;
                          }
                          VendorSummaryDto? hit;
                          for (final v in draft.vendors) {
                            if (v.id == next) {
                              hit = v;
                              break;
                            }
                          }
                          _form.setVendor(next);
                          if (hit != null && hit.defaultBaseUrl.isNotEmpty) {
                            _url.text = hit.defaultBaseUrl;
                          }
                          if (_name.text.isEmpty ||
                              _name.text == (hit?.displayName ?? '')) {
                            _name.text = hit?.displayName ?? next;
                          }
                        },
                      ),
                    ),
                    const Divider(height: 1),
                    ListTile(
                      title: Text(l10n.agentDisplayName),
                      subtitle: TextField(
                        key: const Key('provider-name'),
                        controller: _name,
                        decoration: InputDecoration(
                          border: InputBorder.none,
                          hintText: vendor?.displayName ?? draft.vendor,
                        ),
                        onChanged: (_) => _form.bump(),
                      ),
                    ),
                    const Divider(height: 1),
                    ListTile(
                      title: Text(Copy.agentBaseUrl),
                      subtitle: TextField(
                        key: const Key('provider-url'),
                        controller: _url,
                        decoration: InputDecoration(
                          border: InputBorder.none,
                          hintText: vendor?.defaultBaseUrl.isNotEmpty == true
                              ? vendor!.defaultBaseUrl
                              : 'https://api.openai.com/v1',
                        ),
                      ),
                    ),
                    if (urlChoices.length > 1) ...[
                      const Divider(height: 1),
                      Padding(
                        padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
                        child: DropdownButton<String>(
                          value: urlChoices.contains(_url.text)
                              ? _url.text
                              : null,
                          hint: Text(l10n.agentAltUrl),
                          isExpanded: true,
                          items: [
                            for (final u in urlChoices)
                              DropdownMenuItem(value: u, child: Text(u)),
                          ],
                          onChanged: (next) {
                            if (next == null) {
                              return;
                            }
                            _url.text = next;
                            _form.bump();
                          },
                        ),
                      ),
                    ],
                    const Divider(height: 1),
                    ListTile(
                      title: Text(Copy.agentApiKey),
                      subtitle: TextField(
                        key: const Key('provider-key'),
                        controller: _key,
                        obscureText: true,
                        decoration: InputDecoration(
                          border: InputBorder.none,
                          hintText: widget.isCreate
                              ? Copy.agentApiKeyHint
                              : l10n.agentKeepKeyHint,
                        ),
                      ),
                    ),
                  ],
                ),
                const Gap(18),
                Text(
                  l10n.agentModel,
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    if (draft.models.isEmpty)
                      ListTile(title: Text(l10n.agentModelHint))
                    else
                      for (final m in draft.models) ...[
                        if (m != draft.models.first) const Divider(height: 1),
                        ListTile(dense: true, title: Text(m)),
                      ],
                    const Divider(height: 1),
                    ListTile(
                      title: Text(l10n.agentFetchModels),
                      trailing: draft.fetching
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Icon(Icons.refresh, size: 18),
                      onTap: draft.fetching
                          ? null
                          : () => unawaited(_refreshModels()),
                    ),
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
