library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:toastification/toastification.dart';

import '../../copy.dart';
import '../../state/agent.dart';
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
  late bool _fs;
  late bool _bash;
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
    if (_loaded) {
      return;
    }
    _backend = s.llmBackend;
    _baseUrl.text = s.baseUrl;
    _model.text = s.model;
    _apiKey.text = s.apiKey;
    _fs = s.enableFsTools;
    _bash = s.bashEnabled;
    _loaded = true;
  }

  Future<void> _save() async {
    final live =
        _backend == 'responses_http' ||
        _backend == 'live' ||
        _backend == 'responses';
    if (live && _apiKey.text.trim().isEmpty) {
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: const Text(Copy.agentKeyMissing),
        autoCloseDuration: const Duration(seconds: 3),
      );
      return;
    }
    final next = AgentSettings(
      llmBackend: _backend,
      baseUrl: _baseUrl.text.trim(),
      model: _model.text.trim().isEmpty
          ? (live ? 'gpt-4.1' : 'scripted')
          : _model.text.trim(),
      apiKey: _apiKey.text.trim(),
      enableFsTools: _fs,
      bashEnabled: _bash,
    );
    await ref.read(agentSettingsProvider.notifier).save(next);
    await ref.read(agentProvider.notifier).applySettings();
    if (!mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: const Text(Copy.agentSaved),
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
          const KimSliverHeader(title: Copy.agentSettings),
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
                        segments: const [
                          ButtonSegment(
                            value: 'scripted',
                            label: Text(Copy.agentModeScripted),
                          ),
                          ButtonSegment(
                            value: 'responses_http',
                            label: Text(Copy.agentModeLive),
                          ),
                        ],
                        selected: {_backend},
                        onSelectionChanged: (next) {
                          setState(() => _backend = next.first);
                        },
                      ),
                    ),
                  ],
                ),
                const Gap(18),
                Text(
                  'Provider',
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    ListTile(
                      title: const Text(Copy.agentBaseUrl),
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
                      title: const Text(Copy.agentModel),
                      subtitle: TextField(
                        controller: _model,
                        decoration: const InputDecoration(
                          border: InputBorder.none,
                          hintText: 'scripted / gpt-4.1',
                        ),
                      ),
                    ),
                    const Divider(height: 1),
                    ListTile(
                      title: const Text(Copy.agentApiKey),
                      subtitle: TextField(
                        controller: _apiKey,
                        obscureText: true,
                        decoration: const InputDecoration(
                          border: InputBorder.none,
                          hintText: Copy.agentApiKeyHint,
                        ),
                      ),
                    ),
                  ],
                ),
                const Gap(18),
                Text(
                  'Tools',
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    SwitchListTile(
                      title: const Text('启用 read/write/edit/bash'),
                      subtitle: Text(
                        'cwd = Application Support/agent/workspace',
                        style: theme.textTheme.bodySmall,
                      ),
                      value: _fs,
                      onChanged: (v) => setState(() => _fs = v),
                    ),
                    const Divider(height: 1),
                    SwitchListTile(
                      title: const Text('允许 bash'),
                      value: _bash,
                      onChanged: _fs ? (v) => setState(() => _bash = v) : null,
                    ),
                  ],
                ),
                const Gap(20),
                FilledButton(onPressed: _save, child: const Text(Copy.save)),
                const Gap(8),
                Text(
                  '保存只会重建 in-memory Harness / LLM client，不会删除 sessions/*.sqlite。',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
