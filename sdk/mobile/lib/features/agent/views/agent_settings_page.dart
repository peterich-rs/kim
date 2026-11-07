library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:kim_mobile/features/agent/data/catalog.dart';
import 'package:kim_mobile/features/agent/data/mention.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/router/open_chat.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';
import 'package:kim_mobile/features/agent/providers/agent_settings_form.dart';
import 'package:kim_mobile/features/agent/providers/provider_accounts.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/design/kim_header.dart';
import 'package:kim_mobile/features/agent/providers/agent_editor_hydrator.dart';
import 'package:kim_mobile/features/agent/providers/agent_editor_save.dart';
import 'package:kim_mobile/features/agent/widgets/agent_editor_sections.dart';
import 'package:kim_mobile/features/agent/widgets/model_picker_dialogs.dart';
import 'package:kim_mobile/features/agent/widgets/agent_provider_sections.dart';
import 'package:kim_mobile/design/kim_pinned_footer.dart';
import 'package:kim_mobile/features/agent/widgets/agent_overview_status.dart';
import 'package:kim_mobile/features/agent/widgets/toast_helpers.dart';
import 'package:kim_mobile/features/agent/views/agent_create_page.dart';
import 'package:kim_mobile/features/agent/views/provider_account_page.dart';
import 'package:kim_mobile/features/agent/widgets/context_window.dart';
import 'package:kim_mobile/features/agent/widgets/context_window_controls.dart';
import 'package:kim_mobile/features/agent/widgets/reasoning_controls.dart';
import 'package:kim_mobile/router/app_routes.dart';

part 'agent_settings_page_logic.part.dart';

const _kNewProvider = '__new__';

class AgentEditorPage extends ConsumerStatefulWidget {
  const AgentEditorPage({super.key, this.profileId});

  final String? profileId;

  bool get isCreate => profileId == null || profileId!.isEmpty;

  @override
  ConsumerState<AgentEditorPage> createState() => _AgentEditorPageState();
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
  ReasoningSurface get _surface => _state.surface;
  int get _contextTokens => _state.contextTokens;
  String get _accountId => _state.accountId;
  List<VendorSummary> get _vendors => _state.vendors;
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

  VendorSummary? _vendorById(String id, [List<VendorSummary>? vendors]) {
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
    await _hydrator().bootstrap(isCreate: widget.isCreate);
  }

  AgentEditorHydrator _hydrator() => AgentEditorHydrator(
    context: context,
    form: _form,
    draft: () => _state,
    profiles: () => ref.read(agentProfilesProvider),
    accounts: () => ref.read(providerAccountsProvider),
    ensureVendors: () => ref.read(catalogRepositoryProvider).ensureVendors(),
    surfaceFor: ({required vendor, required model}) => ref
        .read(catalogRepositoryProvider)
        .surface(vendor: vendor, model: model),
    defaultModelFor: (account) =>
        defaultModelForAccount(account, _vendorById(account.vendorId)),
    displayName: _displayName,
    aliases: _aliases,
    prompt: _prompt,
    model: _model,
  );

  void _toastInfo(String message) {
    if (mounted) {
      showAgentToastInfo(context, message);
    }
  }

  void _toastError(String message) {
    if (mounted) {
      showAgentToastError(context, message);
    }
  }

  Future<void> _save() async {
    final saver = AgentEditorSave(
      context: context,
      form: _form,
      ensureStores: () async {
        await ref.read(agentProfilesProvider.notifier).ensureLoaded();
        await ref.read(providerAccountsProvider.notifier).ensureLoaded();
      },
      draftNew: ({required accountId, required model}) => ref
          .read(agentProfilesProvider.notifier)
          .draftNew(accountId: accountId, model: model),
      saveEditor: (profile) =>
          ref.read(agentProfilesProvider.notifier).saveEditor(profile),
      validateChoice:
          ({required vendor, required model, required choice}) async {
            try {
              return await ref
                  .read(catalogRepositoryProvider)
                  .validate(vendor: vendor, model: model, choice: choice);
            } catch (_) {
              return null;
            }
          },
      toastError: _toastError,
      toastInfo: _toastInfo,
      displayName: _displayName,
      aliases: _aliases,
      prompt: _prompt,
      model: _model,
    );
    final shouldPop = await saver.save(
      isCreate: widget.isCreate,
      existing: _profile,
      accountId: _accountId,
      account: _selectedAccount,
      vendorById: (vendorId) => _vendorById(vendorId),
      contextTokens: _contextTokens,
      choice: _choice,
      surface: _surface,
    );
    if (shouldPop && mounted) {
      await Navigator.of(context).maybePop();
    }
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
    final overview = _profile;
    final items = _providerItems(l10n, settings);

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
                AgentBasicsSection(
                  isCreate: widget.isCreate,
                  displayName: _displayName,
                  aliases: _aliases,
                  onNameChanged: () => _form.bump(),
                ),
                const Gap(18),
                AgentSectionLabel(label: l10n.agentProvider),
                const Gap(8),
                AgentProviderSection(
                  accountsEmpty: accounts.isEmpty,
                  accountId: settings.accountId,
                  providerItems: items,
                  selectedValue: providerValue,
                  onSelectAccount: _selectAccount,
                  onAddProvider: () => unawaited(_openNewProvider()),
                ),
                if (settings.accountId.isNotEmpty) ...[
                  const Gap(18),
                  AgentSectionLabel(label: Copy.agentModel),
                  const Gap(8),
                  AgentModelSection(
                    model: _model,
                    onPickModel: () => unawaited(_pickModel()),
                  ),
                  const Gap(18),
                  AgentSectionLabel(label: l10n.agentContextWindow),
                  const Gap(8),
                  ContextWindowControls(
                    tokens: settings.contextTokens,
                    model: _model.text.trim(),
                    onChanged: (next) => _form.setContextTokens(next),
                  ),
                  if (!widget.isCreate) ...[
                    const Gap(18),
                    AgentSectionLabel(label: l10n.agentReasoning),
                    const Gap(8),
                    ReasoningControls(
                      surface: settings.surface,
                      choice: settings.choice,
                      onChanged: (next) => _form.setChoice(next),
                      advancedController: _advanced,
                    ),
                  ],
                ],
                const Gap(18),
                AgentSectionLabel(label: l10n.agentPrompt),
                const Gap(8),
                AgentPromptSection(
                  prompt: _prompt,
                  maxLines: widget.isCreate ? 4 : 8,
                  defaultHint: kDefaultSystemPrompt,
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
                        onTap: () => context.push(
                          AppRoutes.agentCapabilities(overview.id),
                        ),
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
