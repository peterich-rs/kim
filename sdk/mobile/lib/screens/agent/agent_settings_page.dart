library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import '../../agent/catalog.dart';
import '../../agent/mention.dart';
import '../../copy.dart';
import '../../state/agent_profiles.dart';
import '../../state/provider_accounts.dart';
import '../../widgets/empty_state.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';
import 'provider_account_page.dart';
import 'reasoning_controls.dart';

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
  late final TextEditingController _mcp;
  late final TextEditingController _advanced;
  ReasoningChoice _choice = const ReasoningChoice(kind: 'none');
  ReasoningSurfaceDto _surface = const ReasoningSurfaceDto(kind: 'none');
  var _fs = false;
  var _bash = false;
  var _loaded = false;
  String _accountId = '';
  Map<String, String> _permissions = {};
  List<VendorSummaryDto> _vendors = const [];
  List<String> _pendingModels = const [];
  AgentProfile? _draft;

  @override
  void initState() {
    super.initState();
    _displayName = TextEditingController();
    _aliases = TextEditingController();
    _prompt = TextEditingController();
    _model = TextEditingController();
    _mcp = TextEditingController();
    _advanced = TextEditingController();
    unawaited(_bootstrap());
  }

  @override
  void dispose() {
    _displayName.dispose();
    _aliases.dispose();
    _prompt.dispose();
    _model.dispose();
    _mcp.dispose();
    _advanced.dispose();
    super.dispose();
  }

  AgentProfile? get _profile {
    final draft = _draft;
    if (draft != null) {
      for (final p in ref.read(agentProfilesProvider)) {
        if (p.id == draft.id) {
          return p;
        }
      }
      return draft;
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

  VendorSummaryDto? _vendorById(String id) {
    for (final v in _vendors) {
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
    await ref.read(agentProfilesProvider.notifier).ensureLoaded();
    await ref.read(providerAccountsProvider.notifier).ensureLoaded();
    if (!mounted) {
      return;
    }
    try {
      final vendors = await ref.read(catalogRepositoryProvider).ensureVendors();
      if (mounted) {
        setState(() => _vendors = vendors);
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
    _loaded = true;
    final accounts = ref.read(providerAccountsProvider);
    if (widget.isCreate) {
      if (accounts.isNotEmpty) {
        _accountId = accounts.first.id;
        _model.text = defaultModelForAccount(
          accounts.first,
          _vendorById(accounts.first.vendorId),
        );
      }
      if (mounted) {
        setState(() {});
      }
      await _reloadSurface(toastDropped: false);
      return;
    }
    final profile = _profile;
    if (profile == null) {
      return;
    }
    _displayName.text = profile.displayName;
    _aliases.text = profile.aliases.join(', ');
    _prompt.text = profile.systemPrompt;
    _accountId = profile.accountId;
    _model.text = profile.model;
    _fs = profile.tools.fs;
    _bash = profile.tools.bash;
    _permissions = Map<String, String>.from(profile.permissionOverrides);
    _mcp.text = [
      for (final e in profile.extensions)
        if (e.name.isNotEmpty && e.command.isNotEmpty)
          '${e.name} ${e.command.join(' ')}',
    ].join('\n');
    _choice =
        profile.reasoning ??
        ReasoningChoice.fromThinkingEffort(profile.thinkingEffort) ??
        const ReasoningChoice(kind: 'none');
    if (mounted) {
      setState(() {});
    }
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
      setState(() {
        _surface = surface;
        _choice = aligned.choice;
      });
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
    setState(() {
      _accountId = id;
      _pendingModels = const [];
      _model.text = model;
    });
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
    setState(() => _model.text = picked);
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
    } else {
      _pendingModels = selectableModelIds([..._pendingModels, raw]);
    }
    setState(() => _model.text = raw);
    unawaited(_reloadSurface(toastDropped: true));
  }

  List<AgentExtension> _parseMcp(String raw) {
    final out = <AgentExtension>[];
    for (final line in raw.split('\n')) {
      final parts = line
          .trim()
          .split(RegExp(r'\s+'))
          .where((p) => p.isNotEmpty)
          .toList();
      if (parts.length < 2) {
        continue;
      }
      out.add(AgentExtension(name: parts.first, command: parts.sublist(1)));
    }
    return out;
  }

  String _permissionValue(String tool) {
    final raw = _permissions[tool];
    if (tool == 'bash') {
      if (raw == 'never_allow') {
        return raw!;
      }
      return 'ask_before';
    }
    if (raw == 'always_allow' || raw == 'ask_before' || raw == 'never_allow') {
      return raw!;
    }
    if (tool == 'send_message' || tool == 'read_clipboard') {
      return 'ask_before';
    }
    return 'always_allow';
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
    if (widget.isCreate && existing == null) {
      final draft = store.draftNew(accountId: account.id, model: model);
      next = draft.copyWith(
        displayName: name,
        systemPrompt: _prompt.text,
        reasoning: choice,
        thinkingEffort: choice.value ?? '',
      );
      _draft = next;
    } else {
      if (existing == null) {
        if (mounted) {
          Navigator.of(context).maybePop();
        }
        return;
      }
      final base = existing;
      final aliases = [
        for (final part in _aliases.text.split(RegExp(r'[,，\s]+')))
          if (part.trim().isNotEmpty) part.trim(),
      ];
      next = base.copyWith(
        displayName: name,
        aliases: aliases,
        accountId: account.id,
        model: model,
        thinkingEffort: choice.value ?? '',
        reasoning: choice,
        systemPrompt: _prompt.text,
        tools: base.tools.copyWith(fs: _fs, bash: _bash),
        permissionOverrides: _permissions,
        extensions: _parseMcp(_mcp.text),
      );
    }
    await store.saveEditor(next);
    if (!mounted) {
      return;
    }
    setState(() => _choice = choice);
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: Text(Copy.agentSaved),
      autoCloseDuration: const Duration(seconds: 2),
    );
  }

  List<DropdownMenuItem<String>> _providerItems(AppLocalizations l10n) {
    final accounts = ref.watch(providerAccountsProvider);
    final items = <DropdownMenuItem<String>>[
      for (final a in accounts)
        DropdownMenuItem(
          value: a.id,
          child: Text(
            a.displayName.isNotEmpty
                ? a.displayName
                : (_vendorById(a.vendorId)?.displayName ?? a.vendorId),
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
    ref.watch(providerAccountsProvider);
    ref.watch(agentProfilesProvider);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final accounts = ref.watch(providerAccountsProvider);
    final providerValue = _accountId.isNotEmpty ? _accountId : null;

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(
            title: widget.isCreate
                ? l10n.agentCreate
                : (_displayName.text.isEmpty
                      ? Copy.agentSettings
                      : _displayName.text),
          ),
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
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
                        onChanged: (_) => setState(() {}),
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
                            final items = _providerItems(l10n);
                            if (providerValue != null &&
                                items.any((i) => i.value == providerValue)) {
                              return providerValue;
                            }
                            return items.first.value;
                          }(),
                          isExpanded: true,
                          items: _providerItems(l10n),
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
                if (_accountId.isNotEmpty) ...[
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
                    l10n.agentReasoning,
                    style: theme.textTheme.labelLarge?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                  const Gap(8),
                  ReasoningControls(
                    surface: _surface,
                    choice: _choice,
                    onChanged: (next) => setState(() => _choice = next),
                    advancedController: widget.isCreate ? null : _advanced,
                  ),
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
                        maxLines: 8,
                        decoration: InputDecoration(
                          border: InputBorder.none,
                          hintText: kDefaultSystemPrompt,
                          helperText: l10n.agentPromptHint,
                        ),
                      ),
                    ),
                  ],
                ),
                if (!widget.isCreate) ...[
                  const Gap(18),
                  ExpansionTile(
                    title: Text(l10n.agentAdvanced),
                    children: [
                      KimGroupCard(
                        children: [
                          SwitchListTile(
                            title: Text(l10n.agentFsReadonly),
                            value: _fs,
                            onChanged: (next) => setState(() => _fs = next),
                          ),
                          const Divider(height: 1),
                          SwitchListTile(
                            title: Text(l10n.agentBashDanger),
                            subtitle: Text(l10n.agentBashLater),
                            value: _bash,
                            onChanged: (next) => setState(() {
                              _bash = next;
                              if (next) {
                                _permissions['bash'] = 'ask_before';
                              }
                            }),
                          ),
                        ],
                      ),
                      const Gap(18),
                      Text(
                        l10n.agentMcp,
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
                              controller: _mcp,
                              maxLines: 4,
                              decoration: InputDecoration(
                                border: InputBorder.none,
                                hintText: l10n.agentMcpHint,
                              ),
                            ),
                          ),
                        ],
                      ),
                      const Gap(18),
                      Text(
                        l10n.agentPermissions,
                        style: theme.textTheme.labelLarge?.copyWith(
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                      const Gap(8),
                      KimGroupCard(
                        children: [
                          for (final entry in [
                            ('send_message', l10n.agentToolSendMessage),
                            ('read_clipboard', l10n.agentToolClipboard),
                            ('search_contacts', l10n.agentToolSearchContacts),
                            ('search_messages', l10n.agentToolSearchMessages),
                            ('bash', l10n.agentBashDanger),
                          ]) ...[
                            if (entry.$1 != 'send_message')
                              const Divider(height: 1),
                            ListTile(
                              title: Text(entry.$2),
                              trailing: DropdownButton<String>(
                                value: _permissionValue(entry.$1),
                                items: [
                                  if (entry.$1 != 'bash')
                                    DropdownMenuItem(
                                      value: 'always_allow',
                                      child: Text(l10n.agentPermissionAlways),
                                    ),
                                  DropdownMenuItem(
                                    value: 'ask_before',
                                    child: Text(l10n.agentPermissionAsk),
                                  ),
                                  DropdownMenuItem(
                                    value: 'never_allow',
                                    child: Text(l10n.agentPermissionNever),
                                  ),
                                ],
                                onChanged: (next) {
                                  if (next == null) {
                                    return;
                                  }
                                  if (entry.$1 == 'bash' &&
                                      next == 'always_allow') {
                                    return;
                                  }
                                  setState(() => _permissions[entry.$1] = next);
                                },
                              ),
                            ),
                          ],
                        ],
                      ),
                    ],
                  ),
                ],
                const Gap(20),
                FilledButton(
                  key: const Key('agent-save'),
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
