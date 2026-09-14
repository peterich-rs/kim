library;

import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:toastification/toastification.dart';

import '../../agent/host_support.dart';
import '../../agent/skills_catalog.dart';
import '../../agent/workspace_access.dart';
import '../../agent_bridge.dart';
import '../../copy.dart';
import '../../state/agent_profiles.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';

class AgentPlazaPage extends ConsumerStatefulWidget {
  const AgentPlazaPage({super.key, this.assignTo});

  final String? assignTo;

  @override
  ConsumerState<AgentPlazaPage> createState() => _AgentPlazaPageState();
}

class _AgentPlazaPageState extends ConsumerState<AgentPlazaPage> {
  var _loaded = false;
  List<CatalogSkill> _app = const [];
  List<CatalogSkill> _eco = const [];
  AgentProfile? _assignProfile;

  @override
  void initState() {
    super.initState();
    unawaited(_hydrate());
  }

  Future<void> _hydrate() async {
    await ref.read(agentProfilesProvider.notifier).ensureLoaded();
    AgentProfile? assign;
    final assignTo = widget.assignTo?.trim() ?? '';
    if (assignTo.isNotEmpty) {
      for (final p in ref.read(agentProfilesProvider)) {
        if (p.id == assignTo) {
          assign = p;
          break;
        }
      }
    }
    var app = const <CatalogSkill>[];
    var eco = const <CatalogSkill>[];
    if (agentHostSupported) {
      try {
        final bridge = ref.read(agentBridgeProvider);
        app = await loadAppSkillCatalog(bridge: bridge);
        // Ecosystem shelf: always scan real ~/.agents (S-KD 9 / 23).
        final userRoot = await workspaceAccess.realUserAgentsSkills() ?? '';
        if (userRoot.isNotEmpty) {
          await bridge.ensure();
          final raw = await bridge.skillPortableListJson(
            userRoot: userRoot,
            projectRoot: '',
          );
          eco = parseSkillsJson(raw);
        }
        // Also merge project shelf when assigning to a coding profile.
        if (assign != null) {
          final project = await loadPortableSkillCatalog(
            bridge: bridge,
            profile: assign,
          );
          final byId = {for (final s in eco) s.id: s};
          for (final s in project) {
            byId[s.id] = s;
          }
          eco = byId.values.toList()..sort((a, b) => a.id.compareTo(b.id));
        }
      } catch (_) {}
    }
    if (!mounted) {
      return;
    }
    setState(() {
      _app = app;
      _eco = eco;
      _assignProfile = assign;
      _loaded = true;
    });
  }

  Future<void> _assignApp(CatalogSkill skill) async {
    final profile = _assignProfile;
    final l10n = AppLocalizations.of(context);
    if (profile == null) {
      toastification.show(
        context: context,
        type: ToastificationType.info,
        title: Text(l10n.agentPlazaPickAgentFirst),
        autoCloseDuration: const Duration(seconds: 3),
      );
      return;
    }
    final missing = missingToolsForAppSkill(profile, skill.id);
    var tools = profile.tools;
    var perms = Map<String, String>.from(profile.permissionOverrides);
    if (missing.isNotEmpty) {
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
          toastification.show(
            context: context,
            type: ToastificationType.warning,
            title: Text(l10n.agentSkillsNeedsToolsToast),
            autoCloseDuration: const Duration(seconds: 3),
          );
        }
        return;
      }
      tools = enableRequiredTools(tools, missing);
      if (missing.contains('bash')) {
        perms['bash'] = perms['bash'] == 'never_allow'
            ? 'never_allow'
            : 'ask_before';
      }
    }
    final skills = [
      for (final s in profile.skills)
        if (s.id != skill.id) s,
      appSkillRef(skill),
    ];
    final next = profile.copyWith(
      skills: skills,
      tools: tools,
      permissionOverrides: perms,
    );
    await ref.read(agentProfilesProvider.notifier).saveEditor(next);
    if (!mounted) {
      return;
    }
    setState(() => _assignProfile = next);
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: Text(l10n.agentPlazaAssigned(skill.id)),
      autoCloseDuration: const Duration(seconds: 2),
    );
    if (context.mounted && context.canPop()) {
      context.pop();
    }
  }

  Future<void> _importPortable() async {
    final l10n = AppLocalizations.of(context);
    if (kIsWeb) {
      return;
    }
    final userRoot = await workspaceAccess.realUserAgentsSkills();
    if (!mounted) {
      return;
    }
    if (userRoot == null || userRoot.isEmpty) {
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: Text(l10n.agentPlazaImportNoHome),
        autoCloseDuration: const Duration(seconds: 3),
      );
      return;
    }
    final picked = await FilePicker.getDirectoryPath();
    if (picked == null || !mounted) {
      return;
    }
    try {
      final id = await importPortableSkillDir(
        source: Directory(picked),
        userAgentsSkills: userRoot,
      );
      if (!mounted) {
        return;
      }
      toastification.show(
        context: context,
        type: ToastificationType.success,
        title: Text(l10n.agentPlazaImported(id)),
        autoCloseDuration: const Duration(seconds: 3),
      );
      await _hydrate();
    } catch (e) {
      if (!mounted) {
        return;
      }
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: Text(l10n.agentPlazaImportFailed),
        autoCloseDuration: const Duration(seconds: 3),
      );
    }
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
            KimSliverHeader(title: l10n.agentPlazaTitle),
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
          KimSliverHeader(title: l10n.agentPlazaTitle),
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            sliver: SliverList.list(
              children: [
                if (_assignProfile != null) ...[
                  Text(
                    l10n.agentPlazaAssigningTo(_assignProfile!.displayName),
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                  const Gap(12),
                ],
                Text(
                  l10n.agentPlazaKimSection,
                  style: theme.textTheme.titleSmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    if (_app.isEmpty)
                      ListTile(title: Text(l10n.agentSkillsAppEmpty))
                    else
                      for (var i = 0; i < _app.length; i++) ...[
                        if (i > 0) const Divider(height: 1),
                        ListTile(
                          key: Key('agent-plaza-kim-${_app[i].id}'),
                          title: Text(_app[i].name),
                          subtitle: Text(
                            [
                              _app[i].id,
                              if (_app[i].version.isNotEmpty)
                                'v${_app[i].version}',
                              if (_app[i].description.isNotEmpty)
                                _app[i].description,
                            ].join(' · '),
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                          trailing: _assignProfile == null
                              ? null
                              : TextButton(
                                  onPressed: () =>
                                      unawaited(_assignApp(_app[i])),
                                  child: Text(l10n.agentPlazaAssign),
                                ),
                        ),
                      ],
                  ],
                ),
                const Gap(18),
                Text(
                  l10n.agentPlazaEcoSection,
                  style: theme.textTheme.titleSmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    if (_eco.isEmpty)
                      ListTile(
                        title: Text(l10n.agentPlazaEcoEmpty),
                        subtitle: Text(l10n.agentPlazaEcoEmptyHint),
                      )
                    else
                      for (var i = 0; i < _eco.length; i++) ...[
                        if (i > 0) const Divider(height: 1),
                        ListTile(
                          key: Key('agent-plaza-eco-${_eco[i].id}'),
                          title: Text(_eco[i].name),
                          subtitle: Text(_eco[i].id),
                        ),
                      ],
                  ],
                ),
                const Gap(20),
                if (!kIsWeb)
                  OutlinedButton(
                    key: const Key('agent-plaza-import'),
                    onPressed: () => unawaited(_importPortable()),
                    child: Text(l10n.agentPlazaImport),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
