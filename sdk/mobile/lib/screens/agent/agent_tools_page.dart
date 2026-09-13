library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:toastification/toastification.dart';

import '../../copy.dart';
import '../../state/agent_profiles.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';

class AgentToolsPage extends ConsumerStatefulWidget {
  const AgentToolsPage({super.key, required this.profileId});

  final String profileId;

  @override
  ConsumerState<AgentToolsPage> createState() => _AgentToolsPageState();
}

class _AgentToolsPageState extends ConsumerState<AgentToolsPage> {
  late final TextEditingController _mcp;
  Map<String, String> _permissions = {};
  var _loaded = false;
  AgentProfile? _profile;

  @override
  void initState() {
    super.initState();
    _mcp = TextEditingController();
    unawaited(_hydrate());
  }

  @override
  void dispose() {
    _mcp.dispose();
    super.dispose();
  }

  Future<void> _hydrate() async {
    await ref.read(agentProfilesProvider.notifier).ensureLoaded();
    if (!mounted) {
      return;
    }
    AgentProfile? profile;
    for (final p in ref.read(agentProfilesProvider)) {
      if (p.id == widget.profileId) {
        profile = p;
        break;
      }
    }
    if (profile == null) {
      if (mounted) {
        Navigator.of(context).maybePop();
      }
      return;
    }
    _mcp.text = [
      for (final e in profile.extensions)
        if (e.name.isNotEmpty && e.command.isNotEmpty)
          '${e.name} ${e.command.join(' ')}',
    ].join('\n');
    setState(() {
      _profile = profile;
      _permissions = Map<String, String>.from(profile!.permissionOverrides);
      _loaded = true;
    });
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
    final profile = _profile;
    if (profile == null) {
      return;
    }
    final next = profile.copyWith(
      permissionOverrides: _permissions,
      extensions: _parseMcp(_mcp.text),
    );
    await ref.read(agentProfilesProvider.notifier).saveEditor(next);
    if (!mounted) {
      return;
    }
    setState(() => _profile = next);
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: Text(Copy.agentSaved),
      autoCloseDuration: const Duration(seconds: 2),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    if (!_loaded) {
      return Scaffold(
        body: CustomScrollView(
          slivers: [
            KimSliverHeader(title: l10n.agentToolsTitle),
            const SliverFillRemaining(
              child: Center(child: CircularProgressIndicator()),
            ),
          ],
        ),
      );
    }

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(title: l10n.agentToolsTitle),
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            sliver: SliverList.list(
              children: [
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
                const Gap(20),
                FilledButton(
                  key: const Key('agent-tools-save'),
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
