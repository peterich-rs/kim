/// Unified capability sheet (B-KD 9 / Phase 4): IM / FS / MCP / skills.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:kim_mobile/features/agent/data/host_support.dart';
import 'package:kim_mobile/features/agent/data/skills_catalog.dart';
import 'package:kim_mobile/features/agent/data/workspace.dart';
import 'package:kim_mobile/features/agent/data/workspace_access.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/features/agent/providers/agent_capabilities_controller.dart';
import 'package:kim_mobile/features/agent/providers/agent_capabilities_form.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';
import 'package:kim_mobile/features/agent/widgets/agent_runtime_switch.dart';
import 'package:kim_mobile/features/agent/widgets/app_skill_enable_dialog.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/design/kim_header.dart';
import 'package:kim_mobile/router/app_routes.dart';
import 'package:kim_mobile/features/agent/views/sections/fs_section.dart';
import 'package:kim_mobile/features/agent/views/sections/im_section.dart';
import 'package:kim_mobile/features/agent/views/sections/mcp_section.dart';
import 'package:kim_mobile/features/agent/views/sections/preview_card.dart';
import 'package:kim_mobile/features/agent/views/sections/skills_section.dart';

part 'agent_capabilities_page_logic.part.dart';

class AgentCapabilitiesPage extends ConsumerStatefulWidget {
  const AgentCapabilitiesPage({
    super.key,
    required this.profileId,
    this.section,
  });

  final String profileId;

  /// Optional scroll target: `im` | `fs` | `mcp` | `skills`.
  final String? section;

  @override
  ConsumerState<AgentCapabilitiesPage> createState() =>
      _AgentCapabilitiesPageState();
}

/// App + portable skill catalogs; empty when the host is unavailable.
Future<(List<CatalogSkill>, List<CatalogSkill>)> loadSkillCatalogs({
  required AgentBridge bridge,
  required AgentProfile profile,
}) async {
  try {
    final app = await loadAppSkillCatalog(bridge: bridge);
    final portable = await loadPortableSkillCatalog(
      bridge: bridge,
      profile: profile,
    );
    return (app, portable);
  } catch (_) {
    return (const <CatalogSkill>[], const <CatalogSkill>[]);
  }
}

class _AgentCapabilitiesPageState extends ConsumerState<AgentCapabilitiesPage> {
  final _imKey = GlobalKey();
  final _fsKey = GlobalKey();
  final _mcpKey = GlobalKey();
  final _skillsKey = GlobalKey();

  late final TextEditingController _mcp;
  Timer? _previewDebounce;

  AgentCapabilitiesDraft get _draft => ref.read(agentCapabilitiesFormProvider);
  AgentCapabilitiesForm get _form =>
      ref.read(agentCapabilitiesFormProvider.notifier);

  @override
  void initState() {
    super.initState();
    _mcp = TextEditingController();
    unawaited(_hydrate());
  }

  @override
  void dispose() {
    _previewDebounce?.cancel();
    _mcp.dispose();
    super.dispose();
  }

  AgentCapabilitiesController _controller() => AgentCapabilitiesController(
    form: ref.read(agentCapabilitiesFormProvider.notifier),
    draft: ref.read(agentCapabilitiesFormProvider),
    saveEditor: (profile) =>
        ref.read(agentProfilesProvider.notifier).saveEditor(profile),
    savedProfiles: () => ref.read(agentProfilesProvider),
    mcp: _mcp,
    context: context,
  );

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
    final storedBookmark = profile.workspace.bookmarkRef;
    final resolved = await resolveAgentProjectRoot(
      profile: profile,
      client: ref.read(clientPortProvider),
    );

    var (app, portable) = agentHostSupported
        ? await loadSkillCatalogs(
            bridge: ref.read(agentBridgeProvider),
            profile: profile,
          )
        : (const <CatalogSkill>[], const <CatalogSkill>[]);

    if (!mounted) {
      return;
    }
    final caps = profile.resolveCapabilities();
    _mcp.text = mcpTextFromCaps(caps);
    _form.hydrate(
      profile: profile,
      caps: List<CapabilityRef>.from(caps),
      assigned: List<SkillRef>.from(profile.skills),
      denylist: List<String>.from(profile.portableDenylist),
      appCatalog: app,
      portable: portable,
      kindRepo: profile.workspace.isRepo,
      repoPath: profile.workspace.path,
      bookmark: storedBookmark,
      cwd: resolved.path,
      invalidRepo: resolved.invalidRepo,
    );
    _schedulePreview();
    WidgetsBinding.instance.addPostFrameCallback((_) => _scrollToSection());
  }

  void _scrollToSection() {
    final section = widget.section?.trim().toLowerCase() ?? '';
    final key = switch (section) {
      'im' => _imKey,
      'fs' || 'workspace' || 'bash' => _fsKey,
      'mcp' || 'tools' => _mcpKey,
      'skills' => _skillsKey,
      _ => null,
    };
    final ctx = key?.currentContext;
    if (ctx != null) {
      Scrollable.ensureVisible(
        ctx,
        duration: const Duration(milliseconds: 280),
        alignment: 0.1,
      );
    }
  }

  void _schedulePreview() {
    _previewDebounce?.cancel();
    _previewDebounce = Timer(const Duration(milliseconds: 350), () {
      unawaited(_refreshPreview());
    });
  }

  Future<void> _refreshPreview() async {
    final controller = _controller();
    final profile = _draft.profile;
    if (profile == null || !mounted) {
      return;
    }
    final draft = profile
        .withCapabilities(controller.capsWithMcpFromField())
        .copyWith(
          skills: _draft.assigned,
          portableDenylist: _draft.denylist,
          workspace: controller.workspaceSpec(),
        );
    final localNames = projectedToolNames(draft.resolveCapabilities());
    var summary = localNames.isEmpty ? '' : localNames.join(' · ');
    var fromHost = false;
    if (agentHostSupported) {
      try {
        final client = ref.read(clientPortProvider);
        // Preview assembles from store rows (saved profile + overlay +
        // account). Unsaved drafts fall back to the local projection above.
        final preview = await client.previewProfile(profile.id);
        if (preview.tools.isNotEmpty || preview.warnings.isNotEmpty) {
          final names = [for (final tool in preview.tools) tool.name];
          final warnTexts = [
            for (final warning in preview.warnings)
              if (warning.trim().isNotEmpty) warning.trim(),
          ];
          if (names.isNotEmpty || warnTexts.isNotEmpty) {
            summary = names.join(' · ');
            if (warnTexts.isNotEmpty) {
              final warn = warnTexts.join(' · ');
              summary = summary.isEmpty ? warn : '$summary · $warn';
            }
            fromHost = true;
          }
        }
      } catch (_) {
        // Keep local fallback.
      }
    }
    if (!mounted) {
      return;
    }
    _form.setPreview(fromHost: fromHost, summary: summary);
  }

  Future<void> _pickRepo() async {
    final picked = await workspaceAccess.pickDirectory();
    if (picked == null || !mounted) {
      return;
    }
    await _controller().applyPickedWorkspace(
      PickedWorkspaceValue(
        path: picked.path,
        bookmarkBase64: picked.bookmarkBase64,
      ),
    );
  }

  Future<void> _clearRepo() async {
    if (!mounted) {
      return;
    }
    final sandbox = await ref
        .read(clientPortProvider)
        .ensureAgentSandbox(widget.profileId);
    if (mounted) {
      _form.setCwd(sandbox);
    }
    await _controller().clearRepo(_draft.cwd);
  }

  @override
  Widget build(BuildContext context) {
    final draft = ref.watch(agentCapabilitiesFormProvider);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final c = _controller();
    final plazaAction = IconButton(
      key: const Key('agent-skills-plaza'),
      tooltip: l10n.agentSkillsOpenPlaza,
      onPressed: () =>
          context.push(AppRoutes.agentPlazaAssign(widget.profileId)),
      icon: Icon(LucideIcons.store, color: scheme.onSurfaceVariant),
    );
    if (!draft.loaded) {
      return Scaffold(
        body: CustomScrollView(
          slivers: [
            KimSliverHeader(
              title: l10n.agentCapabilitiesTitle,
              actions: [plazaAction],
            ),
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
          KimSliverHeader(
            title: l10n.agentCapabilitiesTitle,
            actions: [plazaAction],
          ),
          KimBodySliver(
            sliver: SliverList.list(
              children: [
                AgentProfileRuntimeSwitch(profileId: widget.profileId),
                if (draft.previewSummary.isNotEmpty ||
                    !draft.previewFromHost) ...[
                  CapabilitiesPreviewCard(
                    summary: draft.previewSummary,
                    fromHost: draft.previewFromHost,
                  ),
                  const Gap(18),
                ],
                ImCapabilitiesSection(
                  sectionKey: _imKey,
                  capOn: c.capOn,
                  permissionAsksBefore: c.asksBefore,
                  onToggleIm: (kind, on) => unawaited(c.toggleIm(kind, on)),
                  onSetAsk: (tool, ask) => unawaited(c.setAsk(tool, ask)),
                ),
                const Gap(18),
                FsCapabilitiesSection(
                  sectionKey: _fsKey,
                  fsOn: c.fsOn,
                  fsWriteOn: c.fsWriteOn,
                  bashOn: c.bashOn,
                  subagentOn: c.capOn(CapabilityKinds.subagent),
                  kindRepo: draft.kindRepo,
                  invalidRepo: draft.invalidRepo,
                  cwd: draft.cwd,
                  repoPath: draft.repoPath,
                  permissionAsksBefore: c.asksBefore,
                  onPresetKnowledge: () async {
                    _form.setKindRepo(false);
                    await c.setFs(read: true, write: true);
                    await c.persist(workspace: WorkspaceSpec.sandbox);
                  },
                  onPresetCoding: () async {
                    _form.setKindRepo(true);
                    if (draft.repoPath.isEmpty) {
                      await _pickRepo();
                    } else {
                      await c.setFs(read: true, write: true);
                      await c.setBash(true);
                      await c.persist(workspace: c.workspaceSpec());
                    }
                  },
                  onPickRepo: () => unawaited(_pickRepo()),
                  onClearRepo: () => unawaited(_clearRepo()),
                  onSetFs: ({required read, required write}) =>
                      unawaited(c.setFs(read: read, write: write)),
                  onSetBash: (on) => unawaited(c.setBash(on)),
                  onSetSubagent: (on) => unawaited(c.setSubagent(on)),
                  onSetAsk: (tool, ask) => unawaited(c.setAsk(tool, ask)),
                ),
                const Gap(18),
                McpCapabilitiesSection(
                  sectionKey: _mcpKey,
                  controller: _mcp,
                  onSave: () => unawaited(c.saveMcp()),
                ),
                const Gap(18),
                SkillsCapabilitiesSection(
                  sectionKey: _skillsKey,
                  appCatalog: draft.appCatalog,
                  portable: draft.portable,
                  assigned: draft.assigned,
                  denylist: draft.denylist,
                  onToggleApp: (skill, enable) =>
                      unawaited(_toggleApp(skill, enable)),
                  onToggleDenylist: (id, muted) =>
                      unawaited(_toggleDenylist(id, muted)),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
