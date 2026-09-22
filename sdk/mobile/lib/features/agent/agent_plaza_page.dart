library;

import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/features/agent/agent_plaza_ui.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/skill_picker.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/design/kim_header.dart';

class AgentPlazaPage extends ConsumerStatefulWidget {
  const AgentPlazaPage({super.key, this.assignTo});

  final String? assignTo;

  @override
  ConsumerState<AgentPlazaPage> createState() => _AgentPlazaPageState();
}

class _AgentPlazaPageState extends ConsumerState<AgentPlazaPage> {
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
          eco = await loadPortableSkills(
            bridge: bridge,
            userRoot: userRoot,
          );
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
    ref
        .read(plazaUiProvider.notifier)
        .showLoaded(app: app, eco: eco, assign: assign);
  }

  Future<void> _assignApp(CatalogSkill skill) async {
    final profile = ref.read(plazaUiProvider).assign;
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
    var enableMissing = missing.isEmpty;
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
      enableMissing = true;
    }
    final next = assignAppSkillToProfile(
      profile: profile,
      skill: skill,
      enableMissingTools: enableMissing,
    );
    await ref.read(agentProfilesProvider.notifier).saveEditor(next);
    if (!mounted) {
      return;
    }
    ref.read(plazaUiProvider.notifier).setAssign(next);
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
    final view = ref.watch(plazaUiProvider);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final importAction = kIsWeb
        ? const <Widget>[]
        : [
            IconButton(
              key: const Key('agent-plaza-import'),
              tooltip: l10n.agentPlazaImport,
              onPressed: () => unawaited(_importPortable()),
              icon: Icon(
                LucideIcons.folderPlus,
                color: scheme.onSurfaceVariant,
              ),
            ),
          ];
    if (!view.loaded) {
      return Scaffold(
        body: CustomScrollView(
          slivers: [
            KimSliverHeader(title: l10n.agentPlazaTitle, actions: importAction),
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
          KimSliverHeader(title: l10n.agentPlazaTitle, actions: importAction),
          KimBodySliver(
            sliver: SliverList.list(
              children: [
                if (view.assign != null) ...[
                  Text(
                    l10n.agentPlazaAssigningTo(view.assign!.displayName),
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                  const Gap(12),
                ] else ...[
                  Text(
                    l10n.agentPlazaPickAgentFirst,
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
                    if (view.app.isEmpty)
                      ListTile(title: Text(l10n.agentSkillsAppEmpty))
                    else
                      for (var i = 0; i < view.app.length; i++) ...[
                        if (i > 0) const Divider(height: 1),
                        ListTile(
                          key: Key('agent-plaza-kim-${view.app[i].id}'),
                          title: Row(
                            children: [
                              Expanded(
                                child: Text(
                                  view.app[i].name,
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                              const SizedBox(width: 8),
                              SkillOriginChip(shelf: view.app[i].shelf),
                            ],
                          ),
                          subtitle: view.app[i].listDescription.isEmpty
                              ? null
                              : Text(
                                  view.app[i].listDescription,
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                ),
                          trailing: view.assign == null
                              ? null
                              : TextButton(
                                  onPressed: () =>
                                      unawaited(_assignApp(view.app[i])),
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
                    if (view.eco.isEmpty)
                      ListTile(
                        title: Text(l10n.agentPlazaEcoEmpty),
                        subtitle: Text(l10n.agentPlazaEcoEmptyHint),
                      )
                    else
                      for (var i = 0; i < view.eco.length; i++) ...[
                        if (i > 0) const Divider(height: 1),
                        ListTile(
                          key: Key('agent-plaza-eco-${view.eco[i].id}'),
                          title: Row(
                            children: [
                              Expanded(
                                child: Text(
                                  view.eco[i].name,
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                              const SizedBox(width: 8),
                              SkillOriginChip(shelf: view.eco[i].shelf),
                            ],
                          ),
                          subtitle: view.eco[i].listDescription.isEmpty
                              ? null
                              : Text(
                                  view.eco[i].listDescription,
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                ),
                        ),
                      ],
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
