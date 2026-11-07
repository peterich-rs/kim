library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/features/agent/data/catalog.dart';
import 'package:kim_mobile/features/agent/providers/provider_account_form.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/core/secret_store_executor.dart';
import 'package:kim_mobile/features/agent/providers/provider_accounts.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/design/kim_header.dart';
import 'package:kim_mobile/design/kim_pinned_footer.dart';
import 'package:kim_mobile/features/agent/widgets/provider_account_sections.dart';
import 'package:kim_mobile/router/app_routes.dart';

/// Full-screen create/edit. Prefer this over a sheet on phones.
Future<String?> openProviderAccountEditor(
  BuildContext context, {
  String? accountId,
}) {
  final path = accountId == null || accountId.isEmpty
      ? AppRoutes.agentAccountNew
      : AppRoutes.agentAccount(accountId);
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
  final _secure = KimKeystore();

  /// Stable id + keyRef for a brand-new account, computed once so a staged
  /// key (fetch-models) and the saved row point at the same vault entry.
  late final String _pendingId =
      'acct-${DateTime.now().microsecondsSinceEpoch}';
  late final String _pendingKeyRef = '$kAccountKeyPrefix$_pendingId';

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

  VendorSummary? _vendorSummaryOf(ProviderAccountDraft draft) {
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
        final v = await _secure.read(existing.keyRef);
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
      client: ref.read(clientPortProvider),
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
    if (_key.text.trim().isEmpty && (_existing?.keyRef.isEmpty ?? true)) {
      _toast(Copy.agentKeyMissing, error: true);
      return;
    }
    _form.setFetching(true);
    final draft = _draft;
    final original = List<String>.from(draft.models);
    try {
      final client = ref.read(clientPortProvider);
      // keyRef the fetch will resolve through the Rust vault. For a new
      // account (not yet saved) stage the typed key under the would-be ref.
      final existing = _existing;
      final keyRef = existing?.keyRef.isNotEmpty == true
          ? existing!.keyRef
          : _pendingKeyRef;
      final typed = _key.text.trim();
      if (typed.isNotEmpty) {
        await client.storeAgentSecret(keyRef: keyRef, secret: typed);
      }
      final list = await client.fetchModels(
        vendorId: canonicalizeVendorId(draft.vendor),
        baseUrl: _url.text.trim(),
        keyRef: keyRef,
      );
      if (!mounted) {
        return;
      }
      final models = unionAccountModels(list, original);
      if (models.isEmpty) {
        throw StateError(Copy.agentFetchModelsFailed('empty'));
      }
      _form.setModels(models);
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
        ? _pendingId
        : existingId;
    final existingKeyRef = _existing?.keyRef ?? '';
    // A staged fetch (before save) may have written under _pendingKeyRef;
    // adopt it when the row is new. Otherwise reuse the row's ref.
    final keyRef = existingKeyRef.isNotEmpty ? existingKeyRef : _pendingKeyRef;
    final account = ProviderAccount(
      id: id,
      vendorId: vendorId,
      baseUrl: _url.text.trim(),
      keyRef: keyRef,
      displayName: _name.text.trim().isEmpty
          ? (vendor?.displayName ?? vendorId)
          : _name.text.trim(),
      models: draft.models,
    );
    await ref.read(providerAccountsProvider.notifier).upsert(account);
    final key = _key.text.trim();
    try {
      if (key.isNotEmpty) {
        // Rust-owned persistence: Keychain via the executor + vault mirror.
        await ref
            .read(clientPortProvider)
            .storeAgentSecret(keyRef: keyRef, secret: key);
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
                ProviderVendorCard(
                  isCreate: widget.isCreate,
                  vendor: vendor,
                  vendors: draft.vendors,
                  selectedVendor: draft.vendor,
                  name: _name,
                  url: _url,
                  keyController: _key,
                  onVendorChanged: (next, hit) {
                    _form.setVendor(next);
                    if (hit != null && hit.defaultBaseUrl.isNotEmpty) {
                      _url.text = hit.defaultBaseUrl;
                    }
                    if (_name.text.isEmpty ||
                        _name.text == (hit?.displayName ?? '')) {
                      _name.text = hit?.displayName ?? next;
                    }
                  },
                  onNameChanged: () => _form.bump(),
                ),
                const Gap(18),
                Text(
                  l10n.agentModel,
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                ProviderModelsCard(
                  models: draft.models,
                  fetching: draft.fetching,
                  onRefresh: () => unawaited(_refreshModels()),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
