library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/features/agent/catalog.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/router/open_chat.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/agent_settings_form.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/design/empty_state.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/design/kim_header.dart';
import 'package:kim_mobile/design/kim_pinned_footer.dart';
import 'package:kim_mobile/features/agent/agent_overview_status.dart';
import 'package:kim_mobile/features/agent/agent_create_page.dart';
import 'package:kim_mobile/features/agent/provider_account_page.dart';
import 'package:kim_mobile/features/agent/context_window.dart';
import 'package:kim_mobile/features/agent/context_window_controls.dart';
import 'package:kim_mobile/features/agent/reasoning_controls.dart';

const _kNewProvider = '__new__';

class AgentEditorPage extends ConsumerStatefulWidget {
  const AgentEditorPage({super.key, this.profileId});

  final String? profileId;

  bool get isCreate => profileId == null || profileId!.isEmpty;

  @override
  ConsumerState<AgentEditorPage> createState() => _AgentEditorPageState();
}

class AgentSettingsPage extends AgentEditorPage {
  const AgentSettingsPage({super.key}) : super(profileId: kGooseAgentId);
}

class _AgentEditorPageState extends ConsumerState<AgentEditorPage> {
  late final TextEditingController _displayName;
  late final TextEditingController _aliases;
  late final TextEditingController _prompt;
  late final TextEditingController _model;
  late final TextEditingController _advanced;

  AgentSettingsDraft get _state => ref.read(agentSettingsFormProvider);

  AgentSettingsForm get _form => ref.read(agentSettingsFormProvider.notifier);

  ReasoningChoice get _choice => _state.choice;
  ReasoningSurfaceDto get _surface => _state.surface;
  int get _contextTokens => _state.contextTokens;
  bool get _loaded => _state.loaded;
  String get _accountId => _state.accountId;
  List<VendorSummaryDto> get _vendors => _state.vendors;
  List<String> get _pendingModels => _state.pendingModels;
  AgentProfile? get _editorDraft => _state.profile;

  @override
  void initState() {
    super.initState();
    _displayName = TextEditingController();
    _aliases = TextEditingController();
    _prompt = TextEditingController();
    _model = TextEditingController();
    _advanced = TextEditingController();
    unawaited(_bootstrap());
  }

  @override
  void dispose() {
    _displayName.dispose();
    _aliases.dispose();
    _prompt.dispose();
    _model.dispose();
    _advanced.dispose();
    super.dispose();
  }

  AgentProfile? get _profile {
    final stored = _editorDraft;
    if (stored != null) {
      for (final p in ref.read(agentProfilesProvider)) {
        if (p.id == stored.id) {
          return p;
        }
      }
      return stored;
    }
    final id = widget.profileId;
    if (id == null || id.isEmpty) {
      return null;
    }
    for (final p in ref.read(agentProfilesProvider)) {
      if (p.id == id) {
        return p;
      }
    }
    return null;
  }

  ProviderAccount? get _selectedAccount {
    if (_accountId.isEmpty) {
      return null;
    }
    return ref.read(providerAccountsProvider.notifier).byId(_accountId);
  }

  VendorSummaryDto? _vendorById(String id, [List<VendorSummaryDto>? vendors]) {
    for (final v in vendors ?? _vendors) {
      if (v.id == id) {
        return v;
      }
    }
    return null;
  }

  List<String> get _modelOptions {
    final account = _selectedAccount;
    if (account != null) {
      return selectableModelIds([
        ...account.models,
        ..._pendingModels,
        _model.text,
      ]);
    }
    return selectableModelIds([..._pendingModels, _model.text]);
  }

  Future<void> _bootstrap() async {
    if (widget.isCreate) {
      return;
    }
    await ref.read(agentProfilesProvider.notifier).ensureLoaded();
    await ref.read(providerAccountsProvider.notifier).ensureLoaded();
    if (!mounted) {
      return;
    }
    try {
      final vendors = await ref.read(catalogRepositoryProvider).ensureVendors();
      if (mounted) {
        _form.setVendors(vendors);
      }
    } catch (_) {}
    if (!mounted) {
      return;
    }
    await _hydrate();
  }

  Future<void> _hydrate() async {
    if (_loaded) {
      return;
    }
    final accounts = ref.read(providerAccountsProvider);
    if (widget.isCreate) {
      if (accounts.isNotEmpty) {
        final model = defaultModelForAccount(
          accounts.first,
          _vendorById(accounts.first.vendorId),
        );
        _model.text = model;
        _form.seedCreate(
          accountId: accounts.first.id,
          contextTokens: defaultContextTokens(model),
        );
      } else {
        _form.markLoaded();
      }
      await _reloadSurface(toastDropped: false);
      return;
    }
    final profile = _profile;
    if (profile == null) {
      _form.markLoaded();
      return;
    }
    _displayName.text = profile.displayName;
    _aliases.text = profile.aliases.join(', ');
    _prompt.text = profile.systemPrompt;
    _model.text = profile.model;
    _form.hydrateEditor(
      accountId: profile.accountId,
      contextTokens:
          profile.contextTokens ?? defaultContextTokens(profile.model),
      choice:
          profile.reasoning ??
          ReasoningChoice.fromThinkingEffort(profile.thinkingEffort) ??
          const ReasoningChoice(kind: 'none'),
    );
    await _reloadSurface(toastDropped: false);
  }

  Future<void> _reloadSurface({required bool toastDropped}) async {
    final account = _selectedAccount;
    final vendorId = account?.vendorId ?? '';
    if (vendorId.isEmpty) {
      return;
    }
    try {
      final catalog = ref.read(catalogRepositoryProvider);
      final surface = await catalog.surface(
        vendor: vendorId,
        model: _model.text.trim(),
      );
      if (!mounted) {
        return;
      }
      final aligned = alignChoice(surface, _choice);
      _form.setSurface(surface: surface, choice: aligned.choice);
      if (toastDropped && aligned.dropped) {
        _toastInfo(Copy.agentReasoningDropped);
      }
    } catch (_) {}
  }

  void _toastInfo(String message) {
    if (!mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.info,
      title: Text(message),
      autoCloseDuration: const Duration(seconds: 3),
    );
  }

  void _toastError(String message) {
    if (!mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.error,
      title: Text(message),
      autoCloseDuration: const Duration(seconds: 4),
    );
  }

  Future<void> _openNewProvider() async {
    final id = await openProviderAccountEditor(context);
    if (!mounted || id == null || id.isEmpty) {
      return;
    }
    _selectAccount(id);
  }

  void _selectAccount(String id) {
    if (id == _kNewProvider) {
      unawaited(_openNewProvider());
      return;
    }
    final account = ref.read(providerAccountsProvider.notifier).byId(id);
    if (account == null) {
      return;
    }
    var model = _model.text.trim();
    var fell = false;
    if (account.models.isNotEmpty && !account.models.contains(model)) {
      model = defaultModelForAccount(account, _vendorById(account.vendorId));
      fell = true;
    }
    _model.text = model;
    _form.bindAccount(id: id, contextTokens: defaultContextTokens(model));
    if (fell && mounted) {
      _toastInfo(AppLocalizations.of(context).agentModelFallback(model));
    }
    unawaited(_reloadSurface(toastDropped: true));
  }

  Future<void> _pickModel() async {
    final models = _modelOptions;
    if (!mounted) {
      return;
    }
    final l10n = AppLocalizations.of(context);
    final selected = _model.text.trim();
    final picked = await showModalBottomSheet<String>(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      builder: (ctx) {
        final height = MediaQuery.sizeOf(ctx).height * 0.55;
        return SafeArea(
          child: SizedBox(
            height: height,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
                  child: Text(
                    l10n.agentModel,
                    style: Theme.of(ctx).textTheme.titleMedium,
                  ),
                ),
                Expanded(
                  child: ListView(
                    children: [
                      for (final id in models)
                        ListTile(
                          title: Text(id),
                          selected: id == selected,
                          onTap: () => Navigator.pop(ctx, id),
                        ),
                      ListTile(
                        title: Text(l10n.agentModelOther),
                        onTap: () => Navigator.pop(ctx, _kNewProvider),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
    if (picked == null || !mounted) {
      return;
    }
    if (picked == _kNewProvider) {
      await _otherModel();
      return;
    }
    _model.text = picked;
    _form.setContextTokens(defaultContextTokens(picked));
    unawaited(_reloadSurface(toastDropped: true));
  }

  Future<void> _otherModel() async {
    final l10n = AppLocalizations.of(context);
    final controller = TextEditingController();
    final raw = await showDialog<String>(
      context: context,
      builder: (ctx) {
        return AlertDialog(
          title: Text(l10n.agentModelOther),
          content: TextField(
            controller: controller,
            autofocus: true,
            decoration: InputDecoration(hintText: l10n.agentModel),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(ctx),
              child: Text(Copy.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(ctx, controller.text.trim()),
              child: Text(Copy.save),
            ),
          ],
        );
      },
    );
    controller.dispose();
    if (raw == null || !isSelectableModelId(raw) || !mounted) {
      return;
    }
    final account = _selectedAccount;
    if (account != null) {
      final models = selectableModelIds([...account.models, raw]);
      await ref
          .read(providerAccountsProvider.notifier)
          .upsert(account.copyWith(models: models));
      _model.text = raw;
      _form.setContextTokens(defaultContextTokens(raw));
    } else {
      _model.text = raw;
      _form.rememberCustomModel(
        pendingModels: selectableModelIds([..._pendingModels, raw]),
        contextTokens: defaultContextTokens(raw),
      );
    }
    unawaited(_reloadSurface(toastDropped: true));
  }

  Future<void> _save() async {
    final l10n = AppLocalizations.of(context);
    final name = _displayName.text.trim();
    if (name.isEmpty) {
      _toastError(l10n.agentNameHint);
      return;
    }
    final accountId = _accountId;
    if (accountId.isEmpty ||
        ref.read(providerAccountsProvider.notifier).byId(accountId) == null) {
      _toastError(l10n.agentNeedProvider);
      return;
    }
    final account = ref.read(providerAccountsProvider.notifier).byId(accountId);
    if (account == null) {
      _toastError(l10n.agentProvider);
      return;
    }
    var choice = _choice;
    try {
      final result = await ref
          .read(catalogRepositoryProvider)
          .validate(
            vendor: account.vendorId,
            model: _model.text.trim(),
            choice: choice,
          );
      choice = result.choice;
      if (result.dropped.isNotEmpty) {
        _toastInfo(Copy.agentReasoningDropped);
      }
    } catch (_) {
      final aligned = alignChoice(_surface, choice);
      choice = aligned.choice;
      if (aligned.dropped) {
        _toastInfo(Copy.agentReasoningDropped);
      }
    }
    final model = _model.text.trim().isEmpty
        ? defaultModelForAccount(account, _vendorById(account.vendorId))
        : _model.text.trim();
    final store = ref.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    final existing = _profile;
    final AgentProfile next;
    final creating = widget.isCreate && existing == null;
    if (creating) {
      final draft = store.draftNew(accountId: account.id, model: model);
      next = draft.copyWith(
        displayName: name,
        systemPrompt: _prompt.text,
        reasoning: choice,
        thinkingEffort: choice.value ?? '',
        contextTokens: _contextTokens,
      );
      _form.setProfile(next);
    } else {
      if (existing == null) {
        if (mounted) {
          Navigator.of(context).maybePop();
        }
        return;
      }
      final aliases = [
        for (final part in _aliases.text.split(RegExp(r'[,，\s]+')))
          if (part.trim().isNotEmpty) part.trim(),
      ];
      next = existing.copyWith(
        displayName: name,
        aliases: aliases,
        accountId: account.id,
        model: model,
        thinkingEffort: choice.value ?? '',
        reasoning: choice,
        contextTokens: _contextTokens,
        systemPrompt: _prompt.text,
      );
    }
    await store.saveEditor(next);
    if (!mounted) {
      return;
    }
    _form.setChoice(choice);
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: Text(Copy.agentSaved),
      autoCloseDuration: const Duration(seconds: 2),
    );
    if (creating) {
      await Navigator.of(context).maybePop();
    }
  }

  void _openChat(AgentProfile profile) {
    final person = personForProfile(profile);
    openKimChat(
      context,
      ref,
      id: person.account,
      kind: ThreadKind.user,
      title: profile.displayName,
    );
  }

  List<DropdownMenuItem<String>> _providerItems(
    AppLocalizations l10n,
    AgentSettingsDraft settings,
  ) {
    final accounts = ref.watch(providerAccountsProvider);
    final items = <DropdownMenuItem<String>>[
      for (final a in accounts)
        DropdownMenuItem(
          value: a.id,
          child: Text(
            a.displayName.isNotEmpty
                ? a.displayName
                : (_vendorById(a.vendorId, settings.vendors)?.displayName ??
                      a.vendorId),
          ),
        ),
      DropdownMenuItem(
        value: _kNewProvider,
        child: Text(l10n.agentNewProvider),
      ),
    ];
    return items;
  }

  @override
  Widget build(BuildContext context) {
    if (widget.isCreate) {
      return const AgentCreatePage();
    }
    final settings = ref.watch(agentSettingsFormProvider);
    ref.watch(agentProfilesProvider);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final accounts = ref.watch(providerAccountsProvider);
    final providerValue = settings.accountId.isNotEmpty
        ? settings.accountId
        : null;
    final overview = !widget.isCreate ? _profile : null;

    return Scaffold(
      bottomNavigationBar: KimPinnedFooter(
        child: FilledButton(
          key: const Key('agent-save'),
          onPressed: _save,
          child: Text(Copy.save),
        ),
      ),
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(
            title: widget.isCreate
                ? l10n.agentCreate
                : (_displayName.text.isEmpty
                      ? Copy.agentSettings
                      : _displayName.text),
            actions: [
              if (overview != null)
                IconButton(
                  key: const Key('agent-open-chat'),
                  tooltip: l10n.agentOpenChat,
                  onPressed: () => _openChat(overview),
                  icon: Icon(
                    LucideIcons.messageCircle,
                    color: scheme.onSurfaceVariant,
                  ),
                ),
            ],
          ),
          KimBodySliver(
            sliver: SliverList.list(
              children: [
                KimGroupCard(
                  children: [
                    ListTile(
                      title: Text(l10n.agentDisplayName),
                      subtitle: TextField(
                        key: const Key('agent-name'),
                        controller: _displayName,
                        decoration: InputDecoration(
                          border: InputBorder.none,
                          hintText: l10n.agentNameHint,
                        ),
                        onChanged: (_) => _form.bump(),
                      ),
                    ),
                    if (!widget.isCreate) ...[
                      const Divider(height: 1),
                      ListTile(
                        title: Text(l10n.agentAliases),
                        subtitle: TextField(
                          controller: _aliases,
                          decoration: InputDecoration(
                            border: InputBorder.none,
                            hintText: l10n.agentAliasesHint,
                          ),
                        ),
                      ),
                    ],
                  ],
                ),
                const Gap(18),
                Text(
                  l10n.agentProvider,
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                if (accounts.isEmpty)
                  EmptyState(
                    icon: LucideIcons.key,
                    title: l10n.agentEmptyProviders,
                    subtitle: l10n.agentEmptyProvidersHint,
                    action: FilledButton(
                      key: const Key('agent-add-provider'),
                      onPressed: () => unawaited(_openNewProvider()),
                      child: Text(l10n.agentAddAccount),
                    ),
                  )
                else
                  KimGroupCard(
                    children: [
                      Padding(
                        padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
                        child: DropdownButton<String>(
                          key: const Key('agent-provider'),
                          value: () {
                            final items = _providerItems(l10n, settings);
                            if (providerValue != null &&
                                items.any((i) => i.value == providerValue)) {
                              return providerValue;
                            }
                            return items.first.value;
                          }(),
                          isExpanded: true,
                          items: _providerItems(l10n, settings),
                          onChanged: (next) {
                            if (next == null) {
                              return;
                            }
                            _selectAccount(next);
                          },
                        ),
                      ),
                    ],
                  ),
                if (settings.accountId.isNotEmpty) ...[
                  const Gap(18),
                  Text(
                    Copy.agentModel,
                    style: theme.textTheme.labelLarge?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                  const Gap(8),
                  KimGroupCard(
                    children: [
                      ListTile(
                        title: Text(Copy.agentModel),
                        subtitle: TextField(
                          key: const Key('agent-model'),
                          controller: _model,
                          readOnly: true,
                          onTap: () => unawaited(_pickModel()),
                          decoration: InputDecoration(
                            border: InputBorder.none,
                            hintText: l10n.agentModelOther,
                            suffixIcon: IconButton(
                              tooltip: l10n.agentPickModel,
                              onPressed: () => unawaited(_pickModel()),
                              icon: const Icon(
                                LucideIcons.chevronsUpDown,
                                size: 18,
                              ),
                            ),
                          ),
                        ),
                      ),
                    ],
                  ),
                  const Gap(18),
                  Text(
                    l10n.agentContextWindow,
                    style: theme.textTheme.labelLarge?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                  const Gap(8),
                  ContextWindowControls(
                    tokens: settings.contextTokens,
                    model: _model.text.trim(),
                    onChanged: (next) => _form.setContextTokens(next),
                  ),
                  if (!widget.isCreate) ...[
                    const Gap(18),
                    Text(
                      l10n.agentReasoning,
                      style: theme.textTheme.labelLarge?.copyWith(
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                    const Gap(8),
                    ReasoningControls(
                      surface: settings.surface,
                      choice: settings.choice,
                      onChanged: (next) => _form.setChoice(next),
                      advancedController: _advanced,
                    ),
                  ],
                  const Gap(18),
                ] else
                  const Gap(18),
                Text(
                  l10n.agentPrompt,
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    Padding(
                      padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
                      child: TextField(
                        key: const Key('agent-prompt'),
                        controller: _prompt,
                        maxLines: widget.isCreate ? 4 : 8,
                        decoration: InputDecoration(
                          border: InputBorder.none,
                          hintText: kDefaultSystemPrompt,
                          helperText: l10n.agentPromptHint,
                        ),
                      ),
                    ),
                  ],
                ),
                if (overview != null) ...[
                  const Gap(18),
                  KimGroupCard(
                    children: [
                      ListTile(
                        key: const Key('agent-entry-capabilities'),
                        title: Text(l10n.agentCapabilitiesEntry),
                        subtitle: Text(
                          agentCapabilitiesSubtitle(l10n, overview),
                        ),
                        trailing: const Icon(Icons.chevron_right),
                        onTap: () =>
                            context.push('/agent/${overview.id}/capabilities'),
                      ),
                    ],
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}
