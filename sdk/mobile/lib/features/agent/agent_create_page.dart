library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/features/agent/catalog.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/ask_before_switch.dart';
import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/agent/provider_account_page.dart';
import 'package:kim_mobile/features/agent/context_window.dart';
import 'package:kim_mobile/features/agent/context_window_controls.dart';
import 'package:kim_mobile/features/agent/reasoning_controls.dart';
import 'package:kim_mobile/features/agent/skill_picker.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/design/empty_state.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/design/kim_header.dart';
import 'package:kim_mobile/design/kim_pinned_footer.dart';

const _kNewProvider = '__new__';
const _kStepCount = 4;

class AgentCreatePage extends ConsumerStatefulWidget {
  const AgentCreatePage({super.key});

  @override
  ConsumerState<AgentCreatePage> createState() => _AgentCreatePageState();
}

class _AgentCreatePageState extends ConsumerState<AgentCreatePage> {
  late final TextEditingController _displayName;
  late final TextEditingController _model;
  late final TextEditingController _prompt;
  late final TextEditingController _mcp;
  ReasoningChoice _choice = const ReasoningChoice(kind: 'none');
  ReasoningSurfaceDto _surface = const ReasoningSurfaceDto(kind: 'none');
  var _contextTokens = kDefaultContextTokens;
  var _step = 0;
  var _saving = false;
  String _accountId = '';
  List<VendorSummaryDto> _vendors = const [];
  List<String> _pendingModels = const [];
  List<CapabilityRef> _caps = List<CapabilityRef>.from(
    kCreateDefaultCapabilities,
  );
  var _perms = <String, String>{};
  var _kindRepo = false;
  String _repoPath = '';
  String _bookmark = '';
  List<CatalogSkill> _app = const [];
  List<CatalogSkill> _portable = const [];
  final _appSelected = <String>{};
  final _portableSelected = <String>{};
  var _portableHydrated = false;

  @override
  void initState() {
    super.initState();
    _displayName = TextEditingController();
    _model = TextEditingController();
    _prompt = TextEditingController();
    _mcp = TextEditingController();
    unawaited(_bootstrap());
  }

  @override
  void dispose() {
    _displayName.dispose();
    _model.dispose();
    _prompt.dispose();
    _mcp.dispose();
    super.dispose();
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

  bool _capOn(String kind) => _caps.any((c) => c.kind == kind && c.enabled);

  bool get _fsOn => _capOn(CapabilityKinds.fs);

  bool get _fsWriteOn {
    for (final c in _caps) {
      if (c.kind == CapabilityKinds.fs && c.enabled) {
        return c.params['writable'] == true;
      }
    }
    return false;
  }

  bool get _bashOn => _capOn(CapabilityKinds.bash);

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
    final accounts = ref.read(providerAccountsProvider);
    if (accounts.isNotEmpty) {
      _accountId = accounts.first.id;
      final model = defaultModelForAccount(
        accounts.first,
        _vendorById(accounts.first.vendorId),
      );
      _model.text = model;
      _contextTokens = defaultContextTokens(model);
    }
    if (mounted) {
      setState(() {});
    }
    await _reloadSurface();
    await _reloadSkills();
  }

  Future<void> _reloadSurface() async {
    final account = _selectedAccount;
    final vendorId = account?.vendorId ?? '';
    if (vendorId.isEmpty) {
      return;
    }
    try {
      final surface = await ref
          .read(catalogRepositoryProvider)
          .surface(vendor: vendorId, model: _model.text.trim());
      if (!mounted) {
        return;
      }
      final aligned = alignChoice(surface, _choice);
      setState(() {
        _surface = surface;
        _choice = aligned.choice;
      });
    } catch (_) {}
  }

  Future<void> _reloadSkills() async {
    if (!agentHostSupported) {
      return;
    }
    try {
      final bridge = ref.read(agentBridgeProvider);
      final app = await loadAppSkillCatalog(bridge: bridge);
      final userRoot = await workspaceAccess.realUserAgentsSkills() ?? '';
      final portable = await loadPortableSkills(
        bridge: bridge,
        userRoot: userRoot,
        projectRoot: _kindRepo ? _repoPath : '',
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _app = app;
        _portable = portable;
        if (!_portableHydrated) {
          _portableSelected
            ..clear()
            ..addAll(portable.map((s) => s.id));
          _portableHydrated = true;
        } else {
          final keep = Set<String>.from(_portableSelected);
          _portableSelected
            ..clear()
            ..addAll(
              portable.where((s) => keep.contains(s.id)).map((s) => s.id),
            );
        }
      });
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
      _contextTokens = defaultContextTokens(model);
    });
    if (fell && mounted) {
      _toastInfo(AppLocalizations.of(context).agentModelFallback(model));
    }
    unawaited(_reloadSurface());
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
    setState(() {
      _model.text = picked;
      _contextTokens = defaultContextTokens(picked);
    });
    unawaited(_reloadSurface());
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
    setState(() {
      _model.text = raw;
      _contextTokens = defaultContextTokens(raw);
    });
    unawaited(_reloadSurface());
  }

  void _setAsk(String tool, bool ask) {
    setState(() {
      _perms = setPermissionAskBefore(_perms, tool, ask: ask);
    });
  }

  void _clearAsk(String tool) {
    if (!permissionAsksBefore(_perms, tool)) {
      return;
    }
    _setAsk(tool, false);
  }

  void _setCap(String kind, bool on, {Map<String, Object?> params = const {}}) {
    setState(() {
      _caps = upsertCapability(_caps, kind: kind, enabled: on, params: params);
      if (!on) {
        final tool = switch (kind) {
          CapabilityKinds.imSendMessage => kAskBeforeSendMessage,
          CapabilityKinds.imReadClipboard => kAskBeforeReadClipboard,
          CapabilityKinds.bash => kAskBeforeBash,
          CapabilityKinds.subagent => kAskBeforeDelegate,
          _ => '',
        };
        if (tool.isNotEmpty) {
          _perms = setPermissionAskBefore(_perms, tool, ask: false);
        }
      }
      if (kind == CapabilityKinds.fs && params['writable'] != true) {
        _perms = setPermissionAskBefore(
          _perms,
          kAskBeforeWriteFile,
          ask: false,
        );
      }
    });
  }

  void _setFs({required bool read, required bool write}) {
    if (!read && !write) {
      _setCap(CapabilityKinds.fs, false);
      _clearAsk(kAskBeforeWriteFile);
      return;
    }
    _setCap(CapabilityKinds.fs, true, params: {'writable': write});
    if (!write) {
      _clearAsk(kAskBeforeWriteFile);
    }
  }

  Future<void> _pickRepo() async {
    final picked = await workspaceAccess.pickDirectory();
    if (picked == null || !mounted) {
      return;
    }
    setState(() {
      _kindRepo = true;
      _repoPath = picked.path;
      _bookmark = picked.bookmarkBase64;
      _caps = upsertCapability(
        upsertCapability(
          _caps,
          kind: CapabilityKinds.fs,
          enabled: true,
          params: const {'writable': true},
        ),
        kind: CapabilityKinds.bash,
        enabled: true,
      );
    });
    await _reloadSkills();
  }

  Future<void> _clearRepo() async {
    setState(() {
      _kindRepo = false;
      _repoPath = '';
      _bookmark = '';
    });
    await _reloadSkills();
  }

  Future<bool> _confirmMissingTools(CatalogSkill skill) async {
    final l10n = AppLocalizations.of(context);
    final draft = AgentProfile(
      id: 'draft',
      displayName: _displayName.text.trim(),
      providerKind: _selectedAccount?.vendorId ?? '',
      baseUrl: _selectedAccount?.baseUrl ?? '',
      model: _model.text.trim(),
      keyRef: '',
      systemPrompt: _prompt.text,
      capabilities: mergeCapsWithMcpLines(_caps, _mcp.text),
    );
    final missing = missingToolsForAppSkill(draft, skill.id);
    if (missing.isEmpty) {
      return true;
    }
    final open = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(l10n.agentSkillsNeedsToolsTitle),
        content: Text(
          l10n.agentSkillsNeedsToolsBody(skill.id, missing.join(', ')),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: Text(l10n.agentSkillsNeedsToolsCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: Text(l10n.agentSkillsNeedsToolsEnable),
          ),
        ],
      ),
    );
    if (open != true || !mounted) {
      if (mounted) {
        _toastInfo(l10n.agentSkillsNeedsToolsToast);
      }
      return false;
    }
    setState(() {
      _caps = enableRequiredCapabilities(_caps, missing);
    });
    return true;
  }

  Future<void> _toggleSkill(CatalogSkill skill, bool selected) async {
    if (skill.isApp) {
      if (selected) {
        if (!await _confirmMissingTools(skill)) {
          return;
        }
        setState(() => _appSelected.add(skill.id));
      } else {
        setState(() => _appSelected.remove(skill.id));
      }
      return;
    }
    setState(() {
      if (selected) {
        _portableSelected.add(skill.id);
      } else {
        _portableSelected.remove(skill.id);
      }
    });
  }

  bool _basicsReady() {
    final name = _displayName.text.trim();
    if (name.isEmpty) {
      return false;
    }
    return _accountId.isNotEmpty &&
        ref.read(providerAccountsProvider.notifier).byId(_accountId) != null;
  }

  Future<void> _next() async {
    final l10n = AppLocalizations.of(context);
    if (_step == 0 && !_basicsReady()) {
      if (_displayName.text.trim().isEmpty) {
        _toastError(l10n.agentNameHint);
      } else {
        _toastError(l10n.agentNeedProvider);
      }
      return;
    }
    if (_step >= _kStepCount - 1) {
      await _finish();
      return;
    }
    setState(() => _step += 1);
    if (_step == 2) {
      await _reloadSkills();
    }
  }

  void _back() {
    if (_step == 0) {
      return;
    }
    setState(() => _step -= 1);
  }

  Future<void> _finish() async {
    if (_saving) {
      return;
    }
    final l10n = AppLocalizations.of(context);
    if (!_basicsReady()) {
      setState(() => _step = 0);
      _toastError(
        _displayName.text.trim().isEmpty
            ? l10n.agentNameHint
            : l10n.agentNeedProvider,
      );
      return;
    }
    final account = _selectedAccount;
    if (account == null) {
      _toastError(l10n.agentNeedProvider);
      return;
    }
    _saving = true;
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
    } catch (_) {
      choice = alignChoice(_surface, choice).choice;
    }
    final model = _model.text.trim().isEmpty
        ? defaultModelForAccount(account, _vendorById(account.vendorId))
        : _model.text.trim();
    final store = ref.read(agentProfilesProvider.notifier);
    await store.ensureLoaded();
    final caps = mergeCapsWithMcpLines(_caps, _mcp.text);
    final skills = [
      for (final skill in _app)
        if (_appSelected.contains(skill.id)) appSkillRef(skill),
    ];
    final denylist = [
      for (final skill in _portable)
        if (!_portableSelected.contains(skill.id)) skill.id,
    ];
    final workspace = _kindRepo
        ? WorkspaceSpec(
            kind: WorkspaceSpec.kindRepo,
            path: _repoPath,
            bookmarkRef: _bookmark,
          )
        : WorkspaceSpec.sandbox;
    var next = store
        .draftNew(accountId: account.id, model: model)
        .copyWith(
          displayName: _displayName.text.trim(),
          systemPrompt: _prompt.text,
          reasoning: choice,
          thinkingEffort: choice.value ?? '',
          contextTokens: _contextTokens,
          workspace: workspace,
          skills: skills,
          portableDenylist: denylist,
          permissionOverrides: Map<String, String>.from(_perms),
        )
        .withCapabilities(caps);
    try {
      await store.saveEditor(next);
      if (_bookmark.isNotEmpty) {
        await workspaceAccess.saveBookmark(next.id, _bookmark);
      }
    } catch (err) {
      _saving = false;
      if (mounted) {
        _toastError(err.toString());
      }
      return;
    }
    if (!mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: Text(Copy.agentSaved),
      autoCloseDuration: const Duration(seconds: 2),
    );
    await Navigator.of(context).maybePop();
  }

  List<DropdownMenuItem<String>> _providerItems(AppLocalizations l10n) {
    final accounts = ref.watch(providerAccountsProvider);
    return [
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
  }

  String _stepTitle(AppLocalizations l10n) {
    return switch (_step) {
      0 => l10n.agentCreateStepBasics,
      1 => l10n.agentCreateStepTools,
      2 => l10n.agentCreateStepSkills,
      _ => l10n.agentCreateStepPrompt,
    };
  }

  @override
  Widget build(BuildContext context) {
    ref.watch(providerAccountsProvider);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final last = _step >= _kStepCount - 1;
    return Scaffold(
      bottomNavigationBar: KimPinnedFooter(
        child: Row(
          children: [
            if (_step > 0) ...[
              OutlinedButton(
                key: const Key('agent-back'),
                onPressed: _saving ? null : _back,
                child: Text(l10n.back),
              ),
              const Gap(12),
            ],
            Expanded(
              child: FilledButton(
                key: Key(last ? 'agent-save' : 'agent-next'),
                onPressed: _saving ? null : () => unawaited(_next()),
                child: Text(
                  last ? l10n.agentCreateFinish : l10n.agentCreateNext,
                ),
              ),
            ),
          ],
        ),
      ),
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(title: l10n.agentCreate),
          KimBodySliver(
            sliver: SliverList.list(
              children: [
                Text(
                  '${l10n.agentCreateProgress(_step + 1, _kStepCount)}  ${_stepTitle(l10n)}',
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(16),
                ...switch (_step) {
                  0 => _basics(theme, scheme, l10n),
                  1 => _tools(theme, scheme, l10n),
                  2 => _skills(l10n),
                  _ => _promptStep(l10n),
                },
              ],
            ),
          ),
        ],
      ),
    );
  }

  List<Widget> _basics(
    ThemeData theme,
    ColorScheme scheme,
    AppLocalizations l10n,
  ) {
    final accounts = ref.watch(providerAccountsProvider);
    final providerValue = _accountId.isNotEmpty ? _accountId : null;
    return [
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
                    icon: const Icon(LucideIcons.chevronsUpDown, size: 18),
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
          tokens: _contextTokens,
          model: _model.text.trim(),
          onChanged: (next) => setState(() => _contextTokens = next),
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
        ),
      ],
    ];
  }

  List<Widget> _tools(
    ThemeData theme,
    ColorScheme scheme,
    AppLocalizations l10n,
  ) {
    return [
      Text(
        l10n.agentCapabilitiesImSection,
        style: theme.textTheme.titleSmall?.copyWith(
          color: scheme.onSurfaceVariant,
        ),
      ),
      const Gap(8),
      KimGroupCard(
        children: [
          for (final entry in [
            (CapabilityKinds.imSendMessage, l10n.agentToolSendMessage),
            (CapabilityKinds.imReadClipboard, l10n.agentToolClipboard),
            (CapabilityKinds.imSearchContacts, l10n.agentToolSearchContacts),
            (CapabilityKinds.imSearchMessages, l10n.agentToolSearchMessages),
            (
              CapabilityKinds.imGetConversationContext,
              l10n.agentToolGetConversationContext,
            ),
            (CapabilityKinds.imListProfiles, l10n.agentToolListProfiles),
          ]) ...[
            if (entry.$1 != CapabilityKinds.imSendMessage)
              const Divider(height: 1),
            SwitchListTile(
              key: Key('agent-cap-${entry.$1}'),
              title: Text(entry.$2),
              value: _capOn(entry.$1),
              onChanged: (v) => _setCap(entry.$1, v),
            ),
            if (entry.$1 == CapabilityKinds.imSendMessage &&
                _capOn(CapabilityKinds.imSendMessage)) ...[
              const Divider(height: 1),
              AskBeforeSwitch(
                tool: kAskBeforeSendMessage,
                value: permissionAsksBefore(_perms, kAskBeforeSendMessage),
                onChanged: (v) => _setAsk(kAskBeforeSendMessage, v),
              ),
            ],
            if (entry.$1 == CapabilityKinds.imReadClipboard &&
                _capOn(CapabilityKinds.imReadClipboard)) ...[
              const Divider(height: 1),
              AskBeforeSwitch(
                tool: kAskBeforeReadClipboard,
                value: permissionAsksBefore(_perms, kAskBeforeReadClipboard),
                onChanged: (v) => _setAsk(kAskBeforeReadClipboard, v),
              ),
            ],
          ],
        ],
      ),
      const Gap(18),
      Text(
        l10n.agentCapabilitiesFsSection,
        style: theme.textTheme.titleSmall?.copyWith(
          color: scheme.onSurfaceVariant,
        ),
      ),
      const Gap(8),
      Text(
        l10n.agentWorkspacePresetHint,
        style: theme.textTheme.bodySmall?.copyWith(
          color: scheme.onSurfaceVariant,
        ),
      ),
      const Gap(8),
      Wrap(
        spacing: 8,
        children: [
          OutlinedButton(
            onPressed: () {
              setState(() => _kindRepo = false);
              _setFs(read: true, write: true);
              unawaited(_reloadSkills());
            },
            child: Text(l10n.agentWorkspacePresetKnowledge),
          ),
          OutlinedButton(
            onPressed: () {
              if (_repoPath.isEmpty) {
                unawaited(_pickRepo());
              } else {
                setState(() => _kindRepo = true);
                _setFs(read: true, write: true);
                _setCap(CapabilityKinds.bash, true);
                unawaited(_reloadSkills());
              }
            },
            child: Text(l10n.agentWorkspacePresetCoding),
          ),
        ],
      ),
      const Gap(8),
      KimGroupCard(
        children: [
          SwitchListTile(
            key: const Key('agent-workspace-kind-repo'),
            title: Text(l10n.agentWorkspaceUseRepo),
            subtitle: Text(l10n.agentWorkspaceUseRepoHint),
            value: _kindRepo,
            onChanged: (next) {
              if (next) {
                unawaited(_pickRepo());
              } else {
                unawaited(_clearRepo());
              }
            },
          ),
          if (_kindRepo) ...[
            const Divider(height: 1),
            ListTile(
              key: const Key('agent-workspace-pick-repo'),
              title: Text(
                _repoPath.isEmpty ? l10n.agentWorkspacePickRepo : _repoPath,
              ),
              trailing: const Icon(Icons.folder_open),
              onTap: () => unawaited(_pickRepo()),
            ),
          ],
          const Divider(height: 1),
          SwitchListTile(
            key: const Key('agent-workspace-fs'),
            title: Text(l10n.agentFsReadonly),
            value: _fsOn,
            onChanged: (next) =>
                _setFs(read: next, write: next ? _fsWriteOn : false),
          ),
          const Divider(height: 1),
          SwitchListTile(
            key: const Key('agent-workspace-fs-write'),
            title: Text(l10n.agentFsWrite),
            subtitle: Text(l10n.agentFsWriteHint),
            value: _fsWriteOn,
            onChanged: (next) => _setFs(read: next || _fsOn, write: next),
          ),
          if (_fsWriteOn) ...[
            const Divider(height: 1),
            AskBeforeSwitch(
              tool: kAskBeforeWriteFile,
              value: permissionAsksBefore(_perms, kAskBeforeWriteFile),
              onChanged: (v) => _setAsk(kAskBeforeWriteFile, v),
            ),
          ],
          const Divider(height: 1),
          SwitchListTile(
            key: const Key('agent-workspace-bash'),
            title: Text(l10n.agentBashDanger),
            subtitle: Text(l10n.agentBashLater),
            value: _bashOn,
            onChanged: (next) => _setCap(CapabilityKinds.bash, next),
          ),
          if (_bashOn) ...[
            const Divider(height: 1),
            AskBeforeSwitch(
              tool: kAskBeforeBash,
              value: permissionAsksBefore(_perms, kAskBeforeBash),
              onChanged: (v) => _setAsk(kAskBeforeBash, v),
            ),
          ],
          const Divider(height: 1),
          SwitchListTile(
            key: const Key('agent-cap-subagent'),
            title: Text(l10n.agentToolDelegate),
            value: _capOn(CapabilityKinds.subagent),
            onChanged: (v) => _setCap(CapabilityKinds.subagent, v),
          ),
          if (_capOn(CapabilityKinds.subagent)) ...[
            const Divider(height: 1),
            AskBeforeSwitch(
              tool: kAskBeforeDelegate,
              value: permissionAsksBefore(_perms, kAskBeforeDelegate),
              onChanged: (v) => _setAsk(kAskBeforeDelegate, v),
            ),
          ],
        ],
      ),
      const Gap(18),
      Text(
        l10n.agentCapabilitiesMcpSection,
        style: theme.textTheme.titleSmall?.copyWith(
          color: scheme.onSurfaceVariant,
        ),
      ),
      const Gap(8),
      KimGroupCard(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
            child: TextField(
              key: const Key('agent-tools-mcp'),
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
    ];
  }

  List<Widget> _skills(AppLocalizations l10n) {
    final merged = mergeSkillCatalogs(app: _app, portable: _portable);
    final selected = <String>{..._appSelected, ..._portableSelected};
    return [
      SkillPickerList(
        skills: merged,
        selectedIds: selected,
        onChanged: (skill, next) => unawaited(_toggleSkill(skill, next)),
      ),
    ];
  }

  List<Widget> _promptStep(AppLocalizations l10n) {
    return [
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
    ];
  }
}
