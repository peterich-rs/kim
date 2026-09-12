library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:toastification/toastification.dart';

import '../../agent/catalog.dart';
import '../../agent/mention.dart';
import '../../agent_bridge.dart';
import '../../copy.dart';
import '../../state/agent_profiles.dart';
import '../../state/agent_settings.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';
import 'reasoning_controls.dart';

class AgentEditorPage extends ConsumerStatefulWidget {
  const AgentEditorPage({super.key, this.profileId = kGooseAgentId});

  final String profileId;

  @override
  ConsumerState<AgentEditorPage> createState() => _AgentEditorPageState();
}

class AgentSettingsPage extends AgentEditorPage {
  const AgentSettingsPage({super.key}) : super(profileId: kGooseAgentId);
}

class _AgentEditorPageState extends ConsumerState<AgentEditorPage> {
  late final TextEditingController _displayName;
  late final TextEditingController _aliases;
  late final TextEditingController _baseUrl;
  late final TextEditingController _model;
  late final TextEditingController _apiKey;
  late final TextEditingController _mcp;
  late final TextEditingController _advanced;
  late String _backend;
  ReasoningChoice _choice = const ReasoningChoice(kind: 'none');
  ReasoningSurfaceDto _surface = const ReasoningSurfaceDto(kind: 'none');
  var _fs = false;
  var _bash = false;
  var _loaded = false;
  var _fetching = false;
  List<String> _fetchedModels = const [];
  List<VendorSummaryDto> _vendors = const [];
  Map<String, String> _permissions = {};

  @override
  void initState() {
    super.initState();
    _displayName = TextEditingController();
    _aliases = TextEditingController();
    _baseUrl = TextEditingController();
    _model = TextEditingController();
    _apiKey = TextEditingController();
    _mcp = TextEditingController();
    _advanced = TextEditingController();
    _backend = 'openai';
    unawaited(_loadCatalog());
  }

  @override
  void dispose() {
    _displayName.dispose();
    _aliases.dispose();
    _baseUrl.dispose();
    _model.dispose();
    _apiKey.dispose();
    _mcp.dispose();
    _advanced.dispose();
    super.dispose();
  }

  AgentProfile? get _profile {
    final id = widget.profileId;
    for (final p in ref.read(agentProfilesProvider)) {
      if (p.id == id) {
        return p;
      }
    }
    return ref.read(agentProfilesProvider.notifier).goose;
  }

  VendorSummaryDto? get _vendor {
    for (final v in _vendors) {
      if (v.id == _backend) {
        return v;
      }
    }
    return null;
  }

  List<String> get _modelOptions {
    final seen = <String>{};
    final out = <String>[];
    void add(String m) {
      final id = m.trim();
      if (id.isEmpty || !seen.add(id)) {
        return;
      }
      out.add(id);
    }

    final vendor = _vendor;
    if (vendor != null) {
      for (final m in vendor.models) {
        add(m);
      }
      add(vendor.defaultModel);
    }
    for (final m in _fetchedModels) {
      add(m);
    }
    add(_model.text);
    return out;
  }

  Future<void> _loadCatalog() async {
    try {
      final catalog = ref.read(catalogRepositoryProvider);
      final vendors = await catalog.ensureVendors();
      if (!mounted) {
        return;
      }
      setState(() => _vendors = vendors);
      await _loadModelCache();
      await _reloadSurface(toastDropped: false);
    } catch (_) {}
  }

  void _hydrate(AgentSettings s) {
    if (!_loaded) {
      unawaited(_applyFromStore(s));
      _loaded = true;
      return;
    }
    if (_apiKey.text.isEmpty &&
        s.apiKey.isNotEmpty &&
        widget.profileId == kGooseAgentId) {
      _apiKey.text = s.apiKey;
    }
  }

  Future<void> _applyFromStore(AgentSettings s) async {
    await ref.read(agentProfilesProvider.notifier).ensureLoaded();
    if (!mounted) {
      return;
    }
    final profile = _profile;
    if (profile == null) {
      _backend = s.llmBackend.isEmpty ? 'openai' : s.llmBackend;
      _baseUrl.text = s.baseUrl;
      _model.text = s.model;
      _apiKey.text = s.apiKey;
      _displayName.text = kGooseAgentName;
      _fs = s.enableFsTools;
      _bash = s.bashEnabled;
      _choice =
          ReasoningChoice.fromThinkingEffort(s.thinkingEffort) ??
          const ReasoningChoice(kind: 'none');
      await _reloadSurface(toastDropped: false);
      return;
    }
    _displayName.text = profile.displayName;
    _aliases.text = profile.aliases.join(', ');
    _backend = profile.providerKind.isEmpty ? 'openai' : profile.providerKind;
    _baseUrl.text = profile.baseUrl.isNotEmpty ? profile.baseUrl : s.baseUrl;
    _model.text = profile.model.isNotEmpty ? profile.model : s.model;
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
    try {
      final key = await ref
          .read(agentProfilesProvider.notifier)
          .readApiKey(profile);
      if (mounted && key.isNotEmpty) {
        _apiKey.text = key;
      }
    } catch (_) {
      if (widget.profileId == kGooseAgentId && s.apiKey.isNotEmpty) {
        _apiKey.text = s.apiKey;
      }
    }
    if (mounted) {
      setState(() {});
    }
    await _reloadSurface(toastDropped: false);
  }

  Future<void> _reloadSurface({required bool toastDropped}) async {
    if (_vendors.isEmpty) {
      return;
    }
    try {
      final catalog = ref.read(catalogRepositoryProvider);
      final surface = await catalog.surface(
        vendor: _backend,
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
        _toastDropped();
      }
    } catch (_) {}
  }

  void _toastDropped() {
    if (!mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.info,
      title: Text(Copy.agentReasoningDropped),
      autoCloseDuration: const Duration(seconds: 3),
    );
  }

  void _selectVendor(String id) {
    VendorSummaryDto? next;
    for (final v in _vendors) {
      if (v.id == id) {
        next = v;
        break;
      }
    }
    setState(() {
      _backend = id;
      if (next != null) {
        if (next.defaultBaseUrl.isNotEmpty) {
          _baseUrl.text = next.defaultBaseUrl;
        }
        final model = _model.text.trim();
        final known = {...next.models, next.defaultModel};
        if (model.isEmpty ||
            (next.defaultModel.isNotEmpty && !known.contains(model))) {
          _model.text = next.defaultModel;
        }
      }
    });
    unawaited(_loadModelCache());
    unawaited(_reloadSurface(toastDropped: true));
  }

  Future<void> _loadModelCache() async {
    final cached = await loadCatalogModelCache(_backend);
    if (!mounted || cached.isEmpty) {
      return;
    }
    setState(() => _fetchedModels = cached);
  }

  Future<void> _fetchModels() async {
    setState(() => _fetching = true);
    try {
      final bridge = ref.read(agentBridgeProvider);
      await bridge.ensure();
      final list = await bridge.fetchModels(
        SessionOpenOpts(
          model: _model.text.trim(),
          llmBackend: _backend,
          resumeOnOpen: false,
          baseUrl: _baseUrl.text.trim(),
          apiKey: _apiKey.text.trim(),
          enableFsTools: false,
          bashEnabled: false,
          profileId: widget.profileId,
          profileJson: '',
          thinkingEffort: _choice.value ?? '',
          gooseMode: '',
          enableKimTools: false,
          enableApprovals: false,
          sessionId: '',
        ),
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _fetchedModels = list;
        if (_model.text.trim().isEmpty && list.isNotEmpty) {
          _model.text = list.first;
        }
      });
      await saveCatalogModelCache(_backend, list);
      await _reloadSurface(toastDropped: true);
    } catch (_) {
      if (!mounted) {
        return;
      }
      final fallback = _vendor?.models ?? const <String>[];
      setState(() => _fetchedModels = fallback);
    } finally {
      if (mounted) {
        setState(() => _fetching = false);
      }
    }
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
    if (_apiKey.text.trim().isEmpty) {
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: Text(Copy.agentKeyMissing),
        autoCloseDuration: const Duration(seconds: 3),
      );
      return;
    }
    if (_backend == 'openai_compatible' &&
        !isAllowedAgentBaseUrl(_baseUrl.text)) {
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: Text(Copy.agentInvalidUrl),
        autoCloseDuration: const Duration(seconds: 3),
      );
      return;
    }
    var choice = _choice;
    try {
      await ref
          .read(catalogRepositoryProvider)
          .validate(
            vendor: _backend,
            model: _model.text.trim(),
            choice: choice,
          );
    } catch (_) {
      final aligned = alignChoice(_surface, choice);
      choice = aligned.choice;
      if (aligned.dropped) {
        _toastDropped();
      }
    }
    await ref.read(agentProfilesProvider.notifier).ensureLoaded();
    final existing =
        _profile ??
        AgentProfile.gooseFromSettings(ref.read(agentSettingsProvider));
    final model = _model.text.trim().isEmpty
        ? (_vendor?.defaultModel.isNotEmpty == true
              ? _vendor!.defaultModel
              : 'gpt-4o')
        : _model.text.trim();
    final aliases = [
      for (final part in _aliases.text.split(RegExp(r'[,，\s]+')))
        if (part.trim().isNotEmpty) part.trim(),
    ];
    final name = _displayName.text.trim().isEmpty
        ? existing.displayName
        : _displayName.text.trim();
    final next = existing.copyWith(
      displayName: name,
      aliases: aliases,
      providerKind: _backend,
      baseUrl: _baseUrl.text.trim(),
      model: model,
      thinkingEffort: choice.value ?? '',
      reasoning: choice,
      tools: AgentToolSet(
        sendMessage: true,
        searchContacts: existing.tools.searchContacts,
        searchMessages: existing.tools.searchMessages,
        getConversationContext: existing.tools.getConversationContext,
        readClipboard: true,
        listProfiles: existing.tools.listProfiles,
        fs: _fs,
        fsWrite: existing.tools.fsWrite,
        bash: _bash,
      ),
      permissionOverrides: _permissions,
      extensions: _parseMcp(_mcp.text),
    );
    await ref
        .read(agentProfilesProvider.notifier)
        .saveEditor(next, apiKey: _apiKey.text.trim());
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

  List<DropdownMenuItem<String>> _vendorItems(AppLocalizations l10n) {
    final items = <DropdownMenuItem<String>>[];
    void section(String group, String label) {
      final rows = vendorsInGroup(_vendors, group);
      if (rows.isEmpty) {
        return;
      }
      items.add(
        DropdownMenuItem(
          value: '__hdr_$group',
          enabled: false,
          child: Text(label),
        ),
      );
      for (final v in rows) {
        items.add(DropdownMenuItem(value: v.id, child: Text(v.displayName)));
      }
    }

    section('primary', l10n.agentVendorPrimary);
    section('gateway', l10n.agentVendorGateway);
    section('other', l10n.agentVendorOther);
    if (_vendors.every((v) => v.id != _backend) && _backend.isNotEmpty) {
      items.add(DropdownMenuItem(value: _backend, child: Text(_backend)));
    }
    return items;
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(agentSettingsProvider);
    _hydrate(settings);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final vendor = _vendor;
    final urlChoices = <String>{
      if (vendor != null && vendor.defaultBaseUrl.isNotEmpty)
        vendor.defaultBaseUrl,
      if (vendor != null) ...vendor.altBaseUrls,
    }.toList();

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(
            title: _displayName.text.isEmpty
                ? Copy.agentSettings
                : _displayName.text,
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
                        controller: _displayName,
                        decoration: InputDecoration(
                          border: InputBorder.none,
                          hintText: kGooseAgentName,
                        ),
                        onChanged: (_) => setState(() {}),
                      ),
                    ),
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
                ),
                const Gap(18),
                Text(
                  Copy.agentMode,
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
                        value: _backend,
                        isExpanded: true,
                        items: () {
                          final items = _vendorItems(l10n);
                          if (items.any((i) => i.value == _backend)) {
                            return items;
                          }
                          return [
                            DropdownMenuItem(
                              value: _backend,
                              child: Text(_backend),
                            ),
                            ...items,
                          ];
                        }(),
                        onChanged: (next) {
                          if (next == null || next.startsWith('__hdr_')) {
                            return;
                          }
                          _selectVendor(next);
                        },
                      ),
                    ),
                  ],
                ),
                const Gap(18),
                Text(
                  Copy.agentProvider,
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    ListTile(
                      title: Text(Copy.agentBaseUrl),
                      subtitle: TextField(
                        controller: _baseUrl,
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
                          value: urlChoices.contains(_baseUrl.text)
                              ? _baseUrl.text
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
                            setState(() => _baseUrl.text = next);
                          },
                        ),
                      ),
                    ],
                    const Divider(height: 1),
                    ListTile(
                      title: Text(Copy.agentModel),
                      subtitle: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          TextField(
                            controller: _model,
                            decoration: InputDecoration(
                              border: InputBorder.none,
                              hintText: vendor?.defaultModel.isNotEmpty == true
                                  ? vendor!.defaultModel
                                  : 'gpt-4o / claude-sonnet-4-5',
                            ),
                            onEditingComplete: () =>
                                unawaited(_reloadSurface(toastDropped: true)),
                          ),
                          Align(
                            alignment: Alignment.centerLeft,
                            child: TextButton(
                              onPressed: _fetching ? null : _fetchModels,
                              child: Text(
                                _fetching ? '…' : l10n.agentFetchModels,
                              ),
                            ),
                          ),
                          if (_modelOptions.isNotEmpty)
                            DropdownButton<String>(
                              value: _modelOptions.contains(_model.text.trim())
                                  ? _model.text.trim()
                                  : null,
                              hint: Text(l10n.agentFetchModels),
                              isExpanded: true,
                              items: [
                                for (final m in _modelOptions)
                                  DropdownMenuItem(value: m, child: Text(m)),
                              ],
                              onChanged: (next) {
                                if (next == null) {
                                  return;
                                }
                                setState(() => _model.text = next);
                                unawaited(_reloadSurface(toastDropped: true));
                              },
                            ),
                        ],
                      ),
                    ),
                    const Divider(height: 1),
                    ListTile(
                      title: Text(Copy.agentApiKey),
                      subtitle: TextField(
                        controller: _apiKey,
                        obscureText: true,
                        decoration: InputDecoration(
                          border: InputBorder.none,
                          hintText: Copy.agentApiKeyHint,
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
                  advancedController: _advanced,
                ),
                const Gap(18),
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
                      if (entry.$1 != 'send_message') const Divider(height: 1),
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
                            if (entry.$1 == 'bash' && next == 'always_allow') {
                              return;
                            }
                            setState(() => _permissions[entry.$1] = next);
                          },
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
                const Gap(20),
                FilledButton(onPressed: _save, child: Text(Copy.save)),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
