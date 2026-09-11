library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:toastification/toastification.dart';

import '../../agent_bridge.dart';
import '../../copy.dart';
import '../../state/agent_profiles.dart';
import '../../state/agent_settings.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';

const _kEfforts = ['off', 'low', 'medium', 'high', 'max'];

class AgentSettingsPage extends ConsumerStatefulWidget {
  const AgentSettingsPage({super.key});

  @override
  ConsumerState<AgentSettingsPage> createState() => _AgentSettingsPageState();
}

class _AgentSettingsPageState extends ConsumerState<AgentSettingsPage> {
  late final TextEditingController _baseUrl;
  late final TextEditingController _model;
  late final TextEditingController _apiKey;
  late String _backend;
  var _thinking = 'off';
  var _loaded = false;
  var _fetching = false;
  List<String> _models = const [];
  List<_Bundled> _bundled = const [];

  @override
  void initState() {
    super.initState();
    _baseUrl = TextEditingController();
    _model = TextEditingController();
    _apiKey = TextEditingController();
    _backend = 'openai';
    unawaited(_loadBundled());
  }

  @override
  void dispose() {
    _baseUrl.dispose();
    _model.dispose();
    _apiKey.dispose();
    super.dispose();
  }

  Future<void> _loadBundled() async {
    try {
      final bridge = ref.read(agentBridgeProvider);
      await bridge.ensure();
      final raw = await bridge.bundledProviders();
      final parsed = <_Bundled>[];
      for (final s in raw) {
        try {
          final m = jsonDecode(s);
          if (m is Map) {
            parsed.add(
              _Bundled(
                name: '${m['name'] ?? ''}',
                displayName: '${m['display_name'] ?? m['name'] ?? ''}',
                mobile: m['mobile'] == true,
              ),
            );
          }
        } catch (_) {}
      }
      if (!mounted) {
        return;
      }
      setState(() => _bundled = parsed);
    } catch (_) {}
  }

  void _hydrate(AgentSettings s) {
    if (!_loaded) {
      _apply(s);
      _loaded = true;
      return;
    }
    if (_apiKey.text.isEmpty && s.apiKey.isNotEmpty) {
      _apply(s);
    }
  }

  void _apply(AgentSettings s) {
    _backend = s.llmBackend.isEmpty ? 'openai' : s.llmBackend;
    _baseUrl.text = s.baseUrl;
    _model.text = s.model;
    _apiKey.text = s.apiKey;
    _thinking = _kEfforts.contains(s.thinkingEffort)
        ? s.thinkingEffort
        : 'off';
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
          profileId: 'goose',
          profileJson: '',
          thinkingEffort: _thinking == 'off' ? '' : _thinking,
          gooseMode: '',
          enableKimTools: false,
          enableApprovals: false,
        ),
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _models = list;
        if (_model.text.trim().isEmpty && list.isNotEmpty) {
          _model.text = list.first;
        }
      });
    } catch (_) {
      if (!mounted) {
        return;
      }
      setState(() {
        _models = _backend == 'anthropic'
            ? const ['claude-sonnet-4-5', 'claude-opus-4-5', 'claude-haiku-4-5']
            : const ['gpt-4o', 'gpt-4o-mini', 'gpt-4.1'];
      });
    } finally {
      if (mounted) {
        setState(() => _fetching = false);
      }
    }
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
    await ref.read(agentProfilesProvider.notifier).ensureLoaded();
    final existing =
        ref.read(agentProfilesProvider.notifier).goose ??
        AgentProfile.gooseFromSettings(ref.read(agentSettingsProvider));
    final goose = existing.copyWith(
      providerKind: _backend,
      baseUrl: _baseUrl.text.trim(),
      model: _model.text.trim().isEmpty
          ? (_backend == 'anthropic' ? 'claude-sonnet-4-5' : 'gpt-4o')
          : _model.text.trim(),
      thinkingEffort: _thinking == 'off' ? '' : _thinking,
    );
    await ref
        .read(agentProfilesProvider.notifier)
        .saveGoose(goose, apiKey: _apiKey.text.trim());
    if (!mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: Text(Copy.agentSaved),
      autoCloseDuration: const Duration(seconds: 2),
    );
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(agentSettingsProvider);
    _hydrate(settings);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);

    final engines = <String>{
      'openai',
      'anthropic',
      'openai_compatible',
      for (final b in _bundled.where((b) => b.mobile)) b.name,
    };
    if (!engines.contains(_backend)) {
      engines.add(_backend);
    }

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(title: Copy.agentSettings),
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            sliver: SliverList.list(
              children: [
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
                        value: engines.contains(_backend) ? _backend : 'openai',
                        isExpanded: true,
                        items: [
                          DropdownMenuItem(
                            value: 'openai',
                            child: Text(Copy.agentProviderOpenAi),
                          ),
                          DropdownMenuItem(
                            value: 'anthropic',
                            child: Text(Copy.agentProviderAnthropic),
                          ),
                          DropdownMenuItem(
                            value: 'openai_compatible',
                            child: Text(l10n.agentProviderCompatible),
                          ),
                          for (final b in _bundled)
                            DropdownMenuItem(
                              value: b.name,
                              enabled: b.mobile,
                              child: Text(
                                b.mobile
                                    ? b.displayName
                                    : '${b.displayName} (${l10n.agentNeedsEnv})',
                              ),
                            ),
                        ],
                        onChanged: (next) {
                          if (next == null) {
                            return;
                          }
                          setState(() {
                            _backend = next;
                            if (_backend == 'anthropic' &&
                                _baseUrl.text.contains('openai.com')) {
                              _baseUrl.text = 'https://api.anthropic.com';
                            }
                            if (_backend == 'openai' &&
                                _baseUrl.text.contains('anthropic.com')) {
                              _baseUrl.text = 'https://api.openai.com/v1';
                            }
                          });
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
                        decoration: const InputDecoration(
                          border: InputBorder.none,
                          hintText: 'https://api.openai.com/v1',
                        ),
                      ),
                    ),
                    const Divider(height: 1),
                    ListTile(
                      title: Text(Copy.agentModel),
                      subtitle: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          TextField(
                            controller: _model,
                            decoration: const InputDecoration(
                              border: InputBorder.none,
                              hintText: 'gpt-4o / claude-sonnet-4-5',
                            ),
                          ),
                          Align(
                            alignment: Alignment.centerLeft,
                            child: TextButton(
                              onPressed: _fetching ? null : _fetchModels,
                              child: Text(
                                _fetching
                                    ? '…'
                                    : l10n.agentFetchModels,
                              ),
                            ),
                          ),
                          if (_models.isNotEmpty)
                            DropdownButton<String>(
                              value: _models.contains(_model.text)
                                  ? _model.text
                                  : null,
                              hint: Text(l10n.agentFetchModels),
                              isExpanded: true,
                              items: [
                                for (final m in _models)
                                  DropdownMenuItem(value: m, child: Text(m)),
                              ],
                              onChanged: (next) {
                                if (next == null) {
                                  return;
                                }
                                setState(() => _model.text = next);
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
                KimGroupCard(
                  children: [
                    Padding(
                      padding: const EdgeInsets.fromLTRB(12, 12, 12, 12),
                      child: SegmentedButton<String>(
                        segments: [
                          ButtonSegment(
                            value: 'off',
                            label: Text(l10n.agentReasoningOff),
                          ),
                          ButtonSegment(
                            value: 'low',
                            label: Text(l10n.agentReasoningLow),
                          ),
                          ButtonSegment(
                            value: 'medium',
                            label: Text(l10n.agentReasoningMedium),
                          ),
                          ButtonSegment(
                            value: 'high',
                            label: Text(l10n.agentReasoningHigh),
                          ),
                          ButtonSegment(
                            value: 'max',
                            label: Text(l10n.agentReasoningMax),
                          ),
                        ],
                        selected: {_thinking},
                        onSelectionChanged: (next) {
                          setState(() => _thinking = next.first);
                        },
                      ),
                    ),
                  ],
                ),
                const Gap(18),
                KimGroupCard(
                  children: [
                    SwitchListTile(
                      title: Text(l10n.agentFsLater),
                      value: false,
                      onChanged: null,
                    ),
                    const Divider(height: 1),
                    SwitchListTile(
                      title: Text(l10n.agentBashLater),
                      value: false,
                      onChanged: null,
                    ),
                  ],
                ),
                const Gap(12),
                Text(
                  Copy.agentGooseHint,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                Text(
                  l10n.agentMoreComing,
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

class _Bundled {
  const _Bundled({
    required this.name,
    required this.displayName,
    required this.mobile,
  });

  final String name;
  final String displayName;
  final bool mobile;
}
