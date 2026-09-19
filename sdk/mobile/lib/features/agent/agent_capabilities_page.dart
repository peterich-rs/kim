library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';
import 'package:kim_mobile/features/agent/workspace.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';
import 'package:kim_mobile/bridge/goose_bridge.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/core/paths.dart';
import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/ask_before_switch.dart';
import 'package:kim_mobile/features/agent/provider_accounts.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/features/agent/skill_picker.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/design/kim_header.dart';

/// Unified capability sheet (B-KD 9 / Phase 4).
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

class _AgentCapabilitiesPageState extends ConsumerState<AgentCapabilitiesPage> {
  final _imKey = GlobalKey();
  final _fsKey = GlobalKey();
  final _mcpKey = GlobalKey();
  final _skillsKey = GlobalKey();

  late final TextEditingController _mcp;
  var _loaded = false;
  AgentProfile? _profile;
  List<CapabilityRef> _caps = const [];
  List<SkillRef> _assigned = const [];
  List<String> _denylist = const [];
  List<CatalogSkill> _appCatalog = const [];
  List<CatalogSkill> _portable = const [];

  var _kindRepo = false;
  var _invalidRepo = false;
  String _cwd = '';
  String _repoPath = '';
  String _bookmark = '';

  String _previewSummary = '';
  var _previewFromHost = false;
  Timer? _previewDebounce;

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
    final paths = KimPaths.instance;
    await paths.ensureAgentDirs();
    var storedBookmark = profile.workspace.bookmarkRef;
    if (storedBookmark.isEmpty) {
      try {
        storedBookmark = await workspaceAccess.loadBookmark(profile.id) ?? '';
      } catch (_) {
        storedBookmark = '';
      }
    }
    final resolved = await resolveAgentProjectRoot(
      profile: profile,
      paths: paths,
    );

    var app = const <CatalogSkill>[];
    var portable = const <CatalogSkill>[];
    if (agentHostSupported) {
      try {
        final bridge = ref.read(agentBridgeProvider);
        app = await loadAppSkillCatalog(bridge: bridge);
        portable = await loadPortableSkillCatalog(
          bridge: bridge,
          profile: profile,
        );
      } catch (_) {
        // Host unavailable — still show assigned refs.
      }
    }

    if (!mounted) {
      return;
    }
    final caps = profile.resolveCapabilities();
    setState(() {
      _profile = profile;
      _caps = List<CapabilityRef>.from(caps);
      _assigned = List<SkillRef>.from(profile!.skills);
      _denylist = List<String>.from(profile.portableDenylist);
      _appCatalog = app;
      _portable = portable;
      _kindRepo = profile.workspace.isRepo;
      _repoPath = profile.workspace.path;
      _bookmark = storedBookmark;
      _cwd = resolved.path;
      _invalidRepo = resolved.invalidRepo;
      _mcp.text = mcpTextFromCaps(caps);
      _loaded = true;
    });
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

  void _schedulePreview() {
    _previewDebounce?.cancel();
    _previewDebounce = Timer(const Duration(milliseconds: 350), () {
      unawaited(_refreshPreview());
    });
  }

  Future<void> _refreshPreview() async {
    final profile = _profile;
    if (profile == null || !mounted) {
      return;
    }
    final draft = profile
        .withCapabilities(_capsWithMcpFromField())
        .copyWith(
          skills: _assigned,
          portableDenylist: _denylist,
          workspace: _workspaceSpec(),
        );
    final localNames = projectedToolNames(draft.resolveCapabilities());
    var summary = localNames.isEmpty ? '' : localNames.join(' · ');
    var fromHost = false;
    if (agentHostSupported) {
      try {
        final bridge = ref.read(agentBridgeProvider);
        final overlay = await ref
            .read(clientPortProvider)
            .getDeviceOverlay(profile.id);
        final paths = await skillHostPaths(
          access: ref.read(workspaceAccessProvider),
          overlay: overlay?.userAgentsSkills ?? '',
        );
        final account = ref
            .read(providerAccountsProvider.notifier)
            .byId(draft.accountId);
        // Prefs JSON only — never inject API keys into preview helpers.
        final Map<String, Object?> previewJson;
        if (account == null) {
          previewJson = Map<String, Object?>.from(draft.toJson());
          if (paths.userAgentsSkills.isNotEmpty) {
            previewJson['user_agents_skills'] = paths.userAgentsSkills;
          }
        } else {
          previewJson = draft.toHostJson(
            account,
            userAgentsSkills: paths.userAgentsSkills,
          );
        }
        final raw = await bridge.previewAssembled(
          profileJson: jsonEncode(previewJson),
          projectRoot: _cwd,
        );
        if (raw.isNotEmpty) {
          final decoded = jsonDecode(raw);
          if (decoded is Map) {
            final tools = decoded['tools'];
            final warnings = decoded['warnings'];
            final names = <String>[];
            if (tools is List) {
              for (final t in tools) {
                if (t is Map && t['name'] != null) {
                  names.add('${t['name']}');
                } else if (t is String) {
                  names.add(t);
                }
              }
            }
            final warnTexts = <String>[];
            if (warnings is List) {
              for (final w in warnings) {
                final text = '$w'.trim();
                if (text.isNotEmpty) {
                  warnTexts.add(text);
                }
              }
            }
            if (names.isNotEmpty || warnTexts.isNotEmpty) {
              summary = names.join(' · ');
              if (warnTexts.isNotEmpty) {
                final warn = warnTexts.join(' · ');
                summary = summary.isEmpty ? warn : '$summary · $warn';
              }
              fromHost = true;
            }
          }
        }
      } catch (_) {
        // Keep local fallback.
      }
    }
    if (!mounted) {
      return;
    }
    setState(() {
      _previewFromHost = fromHost;
      _previewSummary = summary;
    });
  }

  WorkspaceSpec _workspaceSpec() {
    return _kindRepo
        ? WorkspaceSpec(
            kind: WorkspaceSpec.kindRepo,
            path: _repoPath,
            bookmarkRef: _bookmark,
          )
        : WorkspaceSpec.sandbox;
  }

  List<CapabilityRef> _capsWithMcpFromField() {
    return mergeCapsWithMcpLines(_caps, _mcp.text);
  }

  Future<void> _persist({
    List<CapabilityRef>? caps,
    List<SkillRef>? skills,
    List<String>? denylist,
    WorkspaceSpec? workspace,
    Map<String, String>? permissionOverrides,
    bool toast = true,
  }) async {
    final profile = _profile;
    if (profile == null || !mounted) {
      return;
    }
    final l10n = AppLocalizations.of(context);
    final nextCaps = mergeCapsWithMcpLines(caps ?? _caps, _mcp.text);
    var next = profile
        .withCapabilities(nextCaps)
        .copyWith(
          skills: skills ?? _assigned,
          portableDenylist: denylist ?? _denylist,
          workspace: workspace ?? _workspaceSpec(),
          permissionOverrides: permissionOverrides,
        );
    setState(() {
      _profile = next;
      _caps = List<CapabilityRef>.from(next.resolveCapabilities());
      _assigned = List<SkillRef>.from(next.skills);
      _denylist = List<String>.from(next.portableDenylist);
    });
    _schedulePreview();
    try {
      await ref.read(agentProfilesProvider.notifier).saveEditor(next);
    } catch (_) {
      if (!mounted) {
        return;
      }
      AgentProfile? rolled;
      for (final p in ref.read(agentProfilesProvider)) {
        if (p.id == widget.profileId) {
          rolled = p;
          break;
        }
      }
      if (rolled != null) {
        setState(() {
          _profile = rolled;
          _caps = List<CapabilityRef>.from(rolled!.resolveCapabilities());
          _assigned = List<SkillRef>.from(rolled.skills);
          _denylist = List<String>.from(rolled.portableDenylist);
          _mcp.text = mcpTextFromCaps(_caps);
        });
      }
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: Text(l10n.agentSaveFailed),
        autoCloseDuration: const Duration(seconds: 3),
      );
      return;
    }
    if (!mounted || !toast) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: Text(Copy.agentSaved),
      autoCloseDuration: const Duration(seconds: 2),
    );
  }

  Future<void> _toggleIm(String kind, bool on) async {
    final next = upsertCapability(_caps, kind: kind, enabled: on);
    var perms = Map<String, String>.from(_profile?.permissionOverrides ?? {});
    if (!on) {
      final tool = switch (kind) {
        CapabilityKinds.imSendMessage => kAskBeforeSendMessage,
        CapabilityKinds.imReadClipboard => kAskBeforeReadClipboard,
        _ => '',
      };
      if (tool.isNotEmpty) {
        perms = setPermissionAskBefore(perms, tool, ask: false);
      }
    }
    await _persist(caps: next, permissionOverrides: perms);
  }

  Future<void> _setAsk(String tool, bool ask) async {
    final perms = setPermissionAskBefore(
      _profile?.permissionOverrides ?? const {},
      tool,
      ask: ask,
    );
    await _persist(permissionOverrides: perms);
  }

  Future<void> _setFs({required bool read, required bool write}) async {
    var next = List<CapabilityRef>.from(_caps);
    if (!read && !write) {
      next = upsertCapability(next, kind: CapabilityKinds.fs, enabled: false);
    } else {
      next = upsertCapability(
        next,
        kind: CapabilityKinds.fs,
        enabled: true,
        params: {'writable': write},
      );
    }
    var perms = Map<String, String>.from(_profile?.permissionOverrides ?? {});
    if (!write) {
      perms = setPermissionAskBefore(perms, kAskBeforeWriteFile, ask: false);
    }
    await _persist(caps: next, permissionOverrides: perms);
  }

  Future<void> _setBash(bool on) async {
    final next = upsertCapability(
      _caps,
      kind: CapabilityKinds.bash,
      enabled: on,
    );
    var perms = Map<String, String>.from(_profile?.permissionOverrides ?? {});
    if (!on) {
      perms = setPermissionAskBefore(perms, kAskBeforeBash, ask: false);
    }
    await _persist(caps: next, permissionOverrides: perms);
  }

  Future<void> _setSubagent(bool on) async {
    final next = upsertCapability(
      _caps,
      kind: CapabilityKinds.subagent,
      enabled: on,
    );
    var perms = Map<String, String>.from(_profile?.permissionOverrides ?? {});
    if (!on) {
      perms = setPermissionAskBefore(perms, kAskBeforeDelegate, ask: false);
    }
    await _persist(caps: next, permissionOverrides: perms);
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
      _cwd = picked.path;
      _invalidRepo = false;
    });
    final profile = _profile;
    if (profile != null) {
      await workspaceAccess.saveBookmark(profile.id, _bookmark);
    }
    var next = List<CapabilityRef>.from(_caps);
    next = upsertCapability(
      next,
      kind: CapabilityKinds.fs,
      enabled: true,
      params: const {'writable': true},
    );
    next = upsertCapability(next, kind: CapabilityKinds.bash, enabled: true);
    await _persist(caps: next, workspace: _workspaceSpec());
  }

  Future<void> _clearRepo() async {
    final profile = _profile;
    if (profile != null) {
      await workspaceAccess.clearBookmark(profile.id);
    }
    if (!mounted) {
      return;
    }
    setState(() {
      _kindRepo = false;
      _repoPath = '';
      _bookmark = '';
      _invalidRepo = false;
    });
    final paths = KimPaths.instance;
    final sandbox = await paths.ensureSandbox(widget.profileId);
    if (mounted) {
      setState(() => _cwd = sandbox.path);
    }
    await _persist(workspace: WorkspaceSpec.sandbox);
  }

  Future<void> _saveMcp() async {
    await _persist();
  }

  Future<void> _toggleApp(CatalogSkill skill, bool enable) async {
    final profile = _profile;
    if (profile == null) {
      return;
    }
    final l10n = AppLocalizations.of(context);
    if (enable) {
      final missing = missingToolsForAppSkill(
        profile.withCapabilities(_caps),
        skill.id,
      );
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
                key: const Key('agent-skills-enable-tools'),
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
        final caps = enableRequiredCapabilities(_caps, missing);
        var perms = Map<String, String>.from(profile.permissionOverrides);
        final skills = [
          for (final s in _assigned)
            if (s.id != skill.id) s,
          appSkillRef(skill),
        ];
        await _persist(caps: caps, skills: skills, permissionOverrides: perms);
        return;
      }
      final skills = [
        for (final s in _assigned)
          if (s.id != skill.id) s,
        appSkillRef(skill),
      ];
      await _persist(skills: skills);
      return;
    }
    final skills = [
      for (final s in _assigned)
        if (s.id != skill.id) s,
    ];
    await _persist(skills: skills);
  }

  Future<void> _toggleDenylist(String id, bool muted) async {
    final next = List<String>.from(_denylist);
    if (muted) {
      if (!next.contains(id)) {
        next.add(id);
      }
    } else {
      next.remove(id);
    }
    await _persist(denylist: next);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final plazaAction = IconButton(
      key: const Key('agent-skills-plaza'),
      tooltip: l10n.agentSkillsOpenPlaza,
      onPressed: () =>
          context.push('/agent/plaza?assignTo=${widget.profileId}'),
      icon: Icon(LucideIcons.store, color: scheme.onSurfaceVariant),
    );
    if (!_loaded) {
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
                if (_previewSummary.isNotEmpty || !_previewFromHost) ...[
                  KimGroupCard(
                    children: [
                      ListTile(
                        key: const Key('agent-capabilities-preview'),
                        title: Text(l10n.agentCapabilitiesPreview),
                        subtitle: Text(
                          _previewSummary.isEmpty
                              ? l10n.agentCapabilitiesPreviewEmpty
                              : (_previewFromHost
                                    ? _previewSummary
                                    : l10n.agentCapabilitiesPreviewLocal(
                                        _previewSummary,
                                      )),
                        ),
                      ),
                    ],
                  ),
                  const Gap(18),
                ],
                Text(
                  key: _imKey,
                  l10n.agentCapabilitiesImSection,
                  style: theme.textTheme.titleSmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    for (final entry in [
                      (
                        CapabilityKinds.imSendMessage,
                        l10n.agentToolSendMessage,
                      ),
                      (
                        CapabilityKinds.imReadClipboard,
                        l10n.agentToolClipboard,
                      ),
                      (
                        CapabilityKinds.imSearchContacts,
                        l10n.agentToolSearchContacts,
                      ),
                      (
                        CapabilityKinds.imSearchMessages,
                        l10n.agentToolSearchMessages,
                      ),
                      (
                        CapabilityKinds.imGetConversationContext,
                        l10n.agentToolGetConversationContext,
                      ),
                      (
                        CapabilityKinds.imListProfiles,
                        l10n.agentToolListProfiles,
                      ),
                    ]) ...[
                      if (entry.$1 != CapabilityKinds.imSendMessage)
                        const Divider(height: 1),
                      SwitchListTile(
                        key: Key('agent-cap-${entry.$1}'),
                        title: Text(entry.$2),
                        value: _capOn(entry.$1),
                        onChanged: (v) => unawaited(_toggleIm(entry.$1, v)),
                      ),
                      if (entry.$1 == CapabilityKinds.imSendMessage &&
                          _capOn(CapabilityKinds.imSendMessage)) ...[
                        const Divider(height: 1),
                        AskBeforeSwitch(
                          tool: kAskBeforeSendMessage,
                          value: permissionAsksBefore(
                            _profile?.permissionOverrides ?? const {},
                            kAskBeforeSendMessage,
                          ),
                          onChanged: (v) =>
                              unawaited(_setAsk(kAskBeforeSendMessage, v)),
                        ),
                      ],
                      if (entry.$1 == CapabilityKinds.imReadClipboard &&
                          _capOn(CapabilityKinds.imReadClipboard)) ...[
                        const Divider(height: 1),
                        AskBeforeSwitch(
                          tool: kAskBeforeReadClipboard,
                          value: permissionAsksBefore(
                            _profile?.permissionOverrides ?? const {},
                            kAskBeforeReadClipboard,
                          ),
                          onChanged: (v) =>
                              unawaited(_setAsk(kAskBeforeReadClipboard, v)),
                        ),
                      ],
                    ],
                  ],
                ),
                const Gap(18),
                Text(
                  key: _fsKey,
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
                      key: const Key('agent-workspace-preset-knowledge'),
                      onPressed: () async {
                        setState(() => _kindRepo = false);
                        await _setFs(read: true, write: true);
                        await _persist(workspace: WorkspaceSpec.sandbox);
                      },
                      child: Text(l10n.agentWorkspacePresetKnowledge),
                    ),
                    OutlinedButton(
                      key: const Key('agent-workspace-preset-coding'),
                      onPressed: () async {
                        setState(() => _kindRepo = true);
                        if (_repoPath.isEmpty) {
                          await _pickRepo();
                        } else {
                          await _setFs(read: true, write: true);
                          await _setBash(true);
                          await _persist(workspace: _workspaceSpec());
                        }
                      },
                      child: Text(l10n.agentWorkspacePresetCoding),
                    ),
                  ],
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    ListTile(
                      key: const Key('agent-workspace-cwd'),
                      title: Text(
                        _cwd,
                        style: theme.textTheme.bodyMedium?.copyWith(
                          fontFamily: 'monospace',
                        ),
                      ),
                      subtitle: Text(
                        _kindRepo
                            ? l10n.agentWorkspaceKindRepo
                            : l10n.agentWorkspaceKindSandbox,
                      ),
                    ),
                    const Divider(height: 1),
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
                          _repoPath.isEmpty
                              ? l10n.agentWorkspacePickRepo
                              : _repoPath,
                        ),
                        subtitle: _invalidRepo
                            ? Text(
                                l10n.agentWorkspaceRepoReselect,
                                style: TextStyle(color: scheme.error),
                              )
                            : null,
                        trailing: const Icon(Icons.folder_open),
                        onTap: () => unawaited(_pickRepo()),
                      ),
                    ],
                    const Divider(height: 1),
                    SwitchListTile(
                      key: const Key('agent-workspace-fs'),
                      title: Text(l10n.agentFsReadonly),
                      value: _fsOn,
                      onChanged: (next) => unawaited(
                        _setFs(read: next, write: next ? _fsWriteOn : false),
                      ),
                    ),
                    const Divider(height: 1),
                    SwitchListTile(
                      key: const Key('agent-workspace-fs-write'),
                      title: Text(l10n.agentFsWrite),
                      subtitle: Text(l10n.agentFsWriteHint),
                      value: _fsWriteOn,
                      onChanged: (next) =>
                          unawaited(_setFs(read: next || _fsOn, write: next)),
                    ),
                    if (_fsWriteOn) ...[
                      const Divider(height: 1),
                      AskBeforeSwitch(
                        tool: kAskBeforeWriteFile,
                        value: permissionAsksBefore(
                          _profile?.permissionOverrides ?? const {},
                          kAskBeforeWriteFile,
                        ),
                        onChanged: (v) =>
                            unawaited(_setAsk(kAskBeforeWriteFile, v)),
                      ),
                    ],
                    const Divider(height: 1),
                    SwitchListTile(
                      key: const Key('agent-workspace-bash'),
                      title: Text(l10n.agentBashDanger),
                      subtitle: Text(l10n.agentBashLater),
                      value: _bashOn,
                      onChanged: (next) => unawaited(_setBash(next)),
                    ),
                    if (_bashOn) ...[
                      const Divider(height: 1),
                      AskBeforeSwitch(
                        tool: kAskBeforeBash,
                        value: permissionAsksBefore(
                          _profile?.permissionOverrides ?? const {},
                          kAskBeforeBash,
                        ),
                        onChanged: (v) => unawaited(_setAsk(kAskBeforeBash, v)),
                      ),
                    ],
                    const Divider(height: 1),
                    SwitchListTile(
                      key: const Key('agent-cap-subagent'),
                      title: Text(l10n.agentToolDelegate),
                      value: _capOn(CapabilityKinds.subagent),
                      onChanged: (v) => unawaited(_setSubagent(v)),
                    ),
                    if (_capOn(CapabilityKinds.subagent)) ...[
                      const Divider(height: 1),
                      AskBeforeSwitch(
                        tool: kAskBeforeDelegate,
                        value: permissionAsksBefore(
                          _profile?.permissionOverrides ?? const {},
                          kAskBeforeDelegate,
                        ),
                        onChanged: (v) =>
                            unawaited(_setAsk(kAskBeforeDelegate, v)),
                      ),
                    ],
                  ],
                ),
                const Gap(18),
                Text(
                  key: _mcpKey,
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
                        onEditingComplete: () => unawaited(_saveMcp()),
                        decoration: InputDecoration(
                          border: InputBorder.none,
                          hintText: l10n.agentMcpHint,
                        ),
                      ),
                    ),
                    const Divider(height: 1),
                    ListTile(
                      title: Text(l10n.agentCapabilitiesMcpSave),
                      trailing: const Icon(Icons.save_outlined),
                      onTap: () => unawaited(_saveMcp()),
                    ),
                  ],
                ),
                const Gap(18),
                Text(
                  key: _skillsKey,
                  l10n.agentCapabilitiesSkillsSection,
                  style: theme.textTheme.titleSmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                SkillPickerList(
                  skills: mergeSkillCatalogs(
                    app: _appCatalog,
                    portable: _portable,
                  ),
                  selectedIds: {
                    for (final s in _assigned)
                      if (s.enabled) s.id,
                    for (final s in _portable)
                      if (!_denylist.contains(s.id)) s.id,
                  },
                  onChanged: (skill, selected) {
                    if (skill.isApp) {
                      unawaited(_toggleApp(skill, selected));
                      return;
                    }
                    unawaited(_toggleDenylist(skill.id, !selected));
                  },
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
