library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:toastification/toastification.dart';

import '../../copy.dart';
import '../../state/agent_settings.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';

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
  var _loaded = false;

  @override
  void initState() {
    super.initState();
    _baseUrl = TextEditingController();
    _model = TextEditingController();
    _apiKey = TextEditingController();
  }

  @override
  void dispose() {
    _baseUrl.dispose();
    _model.dispose();
    _apiKey.dispose();
    super.dispose();
  }

  void _hydrate(AgentSettings s) {
    if (!_loaded) {
      _apply(s);
      _loaded = true;
      return;
    }
    // First frame is empty defaults; fill in once reload lands.
    if (_apiKey.text.isEmpty && s.apiKey.isNotEmpty) {
      _apply(s);
    }
  }

  void _apply(AgentSettings s) {
    _backend = s.llmBackend == 'anthropic' ? 'anthropic' : 'openai';
    _baseUrl.text = s.baseUrl;
    _model.text = s.model;
    _apiKey.text = s.apiKey;
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
    final next = AgentSettings(
      llmBackend: _backend,
      baseUrl: _baseUrl.text.trim(),
      model: _model.text.trim().isEmpty
          ? (_backend == 'anthropic' ? 'claude-sonnet-4-5' : 'gpt-4o')
          : _model.text.trim(),
      apiKey: _apiKey.text.trim(),
      enableFsTools: false,
      bashEnabled: false,
    );
    await ref.read(agentSettingsProvider.notifier).save(next);
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
                      child: SegmentedButton<String>(
                        segments: [
                          ButtonSegment(
                            value: 'openai',
                            label: Text(Copy.agentProviderOpenAi),
                          ),
                          ButtonSegment(
                            value: 'anthropic',
                            label: Text(Copy.agentProviderAnthropic),
                          ),
                        ],
                        selected: {_backend},
                        onSelectionChanged: (next) {
                          setState(() {
                            _backend = next.first;
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
                      subtitle: TextField(
                        controller: _model,
                        decoration: const InputDecoration(
                          border: InputBorder.none,
                          hintText: 'gpt-4o / claude-sonnet-4-5',
                        ),
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
