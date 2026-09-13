library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:toastification/toastification.dart';

import '../../agent/catalog.dart';
import '../../agent_bridge.dart';
import '../../copy.dart';
import '../../core/settings.dart';
import '../../state/provider_accounts.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';

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
  late String _vendor;
  late List<String> _models;
  List<VendorSummaryDto> _vendors = const [];
  var _fetching = false;
  var _hydrated = false;
  final _secure = SettingsStore.productionSecureStorage();

  @override
  void initState() {
    super.initState();
    _name = TextEditingController();
    _url = TextEditingController();
    _key = TextEditingController();
    _vendor = 'openai';
    _models = const [];
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

  VendorSummaryDto? get _vendorSummary {
    for (final v in _vendors) {
      if (v.id == _vendor) {
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
        setState(() => _vendors = vendors);
      }
    } catch (_) {}
    if (!mounted || _hydrated) {
      return;
    }
    _hydrated = true;
    final existing = _existing;
    if (existing != null) {
      _name.text = existing.displayName;
      _url.text = existing.baseUrl;
      _vendor = existing.vendorId;
      _models = List<String>.from(existing.models);
      try {
        final v = await _secure.read(key: existing.keyRef);
        if (mounted && v != null && v.isNotEmpty) {
          _key.text = v;
        }
      } catch (_) {}
    } else {
      final vendor = _vendorSummary;
      if (vendor != null && vendor.defaultBaseUrl.isNotEmpty) {
        _url.text = vendor.defaultBaseUrl;
      }
      _name.text = vendor?.displayName ?? 'OpenAI';
    }
    if (mounted) {
      setState(() {});
    }
    await _seedModels();
  }

  Future<void> _seedModels() async {
    if (_models.isNotEmpty) {
      return;
    }
    final vendor = _vendorSummary;
    final models = await migrateAccountModelIds(
      vendorId: _vendor,
      existing: _models,
      catalogModels: vendor?.models ?? const [],
      defaultModel: vendor?.defaultModel ?? '',
    );
    if (!mounted || models.isEmpty) {
      return;
    }
    setState(() => _models = models);
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
    setState(() => _fetching = true);
    final original = List<String>.from(_models);
    try {
      final bridge = ref.read(agentBridgeProvider);
      await bridge.ensure();
      final list = await bridge.fetchModels(
        SessionOpenOpts(
          model: _models.isNotEmpty ? _models.first : '',
          llmBackend: canonicalizeVendorId(_vendor),
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
        ),
      );
      if (!mounted) {
        return;
      }
      final models = unionAccountModels(list, original);
      if (models.isEmpty) {
        throw StateError(Copy.agentFetchModelsFailed('empty'));
      }
      setState(() => _models = models);
      final existing = _existing;
      if (existing != null) {
        await ref
            .read(providerAccountsProvider.notifier)
            .upsert(
              existing.copyWith(
                vendorId: canonicalizeVendorId(_vendor),
                baseUrl: _url.text.trim(),
                displayName: _name.text.trim().isEmpty
                    ? _vendor
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
      setState(() => _models = original);
      _toast(Copy.agentFetchModelsFailed(err.toString()), error: true);
    } finally {
      if (mounted) {
        setState(() => _fetching = false);
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
    final vendorId = canonicalizeVendorId(_vendor);
    final vendor = _vendorSummary;
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
      models: _models,
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
    final l10n = AppLocalizations.of(context);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final vendor = _vendorSummary;
    final urlChoices = <String>{
      if (vendor != null && vendor.defaultBaseUrl.isNotEmpty)
        vendor.defaultBaseUrl,
      if (vendor != null) ...vendor.altBaseUrls,
    }.toList();
    final title = widget.isCreate
        ? l10n.agentAddAccount
        : (_name.text.isEmpty ? l10n.agentAccounts : _name.text);

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(title: title),
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
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
                          final ids = {for (final v in _vendors) v.id};
                          if (ids.contains(_vendor) || _vendors.isEmpty) {
                            return _vendor;
                          }
                          return _vendors.first.id;
                        }(),
                        isExpanded: true,
                        items: [
                          for (final v in sortVendors(_vendors))
                            DropdownMenuItem(
                              value: v.id,
                              child: Text(v.displayName),
                            ),
                          if (_vendors.every((v) => v.id != _vendor) &&
                              _vendor.isNotEmpty)
                            DropdownMenuItem(
                              value: _vendor,
                              child: Text(_vendor),
                            ),
                        ],
                        onChanged: (next) {
                          if (next == null) {
                            return;
                          }
                          VendorSummaryDto? hit;
                          for (final v in _vendors) {
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
                            if (_name.text.isEmpty ||
                                _name.text ==
                                    (_vendorSummary?.displayName ?? '')) {
                              _name.text = hit?.displayName ?? next;
                            }
                          });
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
                          hintText: vendor?.displayName ?? _vendor,
                        ),
                        onChanged: (_) => setState(() {}),
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
                            setState(() => _url.text = next);
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
                    if (_models.isEmpty)
                      ListTile(title: Text(l10n.agentModelHint))
                    else
                      for (final m in _models) ...[
                        if (m != _models.first) const Divider(height: 1),
                        ListTile(dense: true, title: Text(m)),
                      ],
                    const Divider(height: 1),
                    ListTile(
                      title: Text(l10n.agentFetchModels),
                      trailing: _fetching
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Icon(Icons.refresh, size: 18),
                      onTap: _fetching
                          ? null
                          : () => unawaited(_refreshModels()),
                    ),
                  ],
                ),
                const Gap(20),
                FilledButton(
                  key: const Key('provider-save'),
                  onPressed: _save,
                  child: Text(Copy.save),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
