library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:toastification/toastification.dart';

import '../../agent/host_support.dart';
import '../../agent/skills_catalog.dart';
import '../../agent_bridge.dart';
import '../../copy.dart';
import '../../state/agent_profiles.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';

class AgentSkillsPage extends ConsumerStatefulWidget {
  const AgentSkillsPage({super.key, required this.profileId});

  final String profileId;

  @override
  ConsumerState<AgentSkillsPage> createState() => _AgentSkillsPageState();
}

class _AgentSkillsPageState extends ConsumerState<AgentSkillsPage> {
  var _loaded = false;
  AgentProfile? _profile;
  List<CatalogSkill> _appCatalog = const [];
  List<CatalogSkill> _portable = const [];
  List<SkillRef> _assigned = const [];
  List<String> _denylist = const [];

  @override
  void initState() {
    super.initState();
    unawaited(_hydrate());
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
        // Host unavailable — still show assigned refs from prefs.
      }
    }
    if (!mounted) {
      return;
    }
    setState(() {
      _profile = profile;
      _assigned = List<SkillRef>.from(profile!.skills);
      _denylist = List<String>.from(profile.portableDenylist);
      _appCatalog = app;
      _portable = portable;
      _loaded = true;
    });
  }

  bool _isAssigned(String id) => _assigned.any((s) => s.id == id && s.enabled);

  Future<void> _persist({
    List<SkillRef>? skills,
    List<String>? denylist,
    AgentToolSet? tools,
    Map<String, String>? permissionOverrides,
  }) async {
    final profile = _profile;
    if (profile == null) {
      return;
    }
    var next = profile.copyWith(
      skills: skills ?? _assigned,
      portableDenylist: denylist ?? _denylist,
      tools: tools,
      permissionOverrides: permissionOverrides,
    );
    if (tools != null) {
      next = next.withCapabilities(
        capabilitiesFromLegacy(tools, next.extensions),
      );
    }
    // Optimistic: flip switches immediately; disk/network save follows.
    setState(() {
      _profile = next;
      _assigned = List<SkillRef>.from(next.skills);
      _denylist = List<String>.from(next.portableDenylist);
    });
    try {
      await ref.read(agentProfilesProvider.notifier).saveEditor(next);
    } catch (_) {
      if (!mounted) {
        return;
      }
      // Provider state unchanged on failure — roll the switch back.
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
          _assigned = List<SkillRef>.from(rolled!.skills);
          _denylist = List<String>.from(rolled.portableDenylist);
        });
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
  }

  Future<void> _toggleApp(CatalogSkill skill, bool enable) async {
    final profile = _profile;
    if (profile == null) {
      return;
    }
    final l10n = AppLocalizations.of(context);
    if (enable) {
      final missing = missingToolsForAppSkill(profile, skill.id);
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
        final tools = enableRequiredTools(profile.tools, missing);
        var perms = Map<String, String>.from(profile.permissionOverrides);
        if (missing.contains('bash')) {
          perms['bash'] = perms['bash'] == 'never_allow'
              ? 'never_allow'
              : 'ask_before';
        }
        final skills = [
          for (final s in _assigned)
            if (s.id != skill.id) s,
          appSkillRef(skill),
        ];
        await _persist(
          skills: skills,
          tools: tools,
          permissionOverrides: perms,
        );
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
    if (!_loaded) {
      return Scaffold(
        body: CustomScrollView(
          slivers: [
            KimSliverHeader(title: l10n.agentSkillsTitle),
            const SliverFillRemaining(
              child: Center(child: CircularProgressIndicator()),
            ),
          ],
        ),
      );
    }

    final scanOff =
        _profile != null &&
        !_profile!.workspace.isRepo &&
        !_profile!.tools.fs &&
        !_profile!.tools.fsWrite;

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          KimSliverHeader(title: l10n.agentSkillsTitle),
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            sliver: SliverList.list(
              children: [
                Text(
                  l10n.agentSkillsAppSection,
                  style: theme.textTheme.titleSmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    if (_appCatalog.isEmpty)
                      ListTile(
                        title: Text(l10n.agentSkillsAppEmpty),
                        subtitle: Text(l10n.agentSkillsAppEmptyHint),
                      )
                    else
                      for (var i = 0; i < _appCatalog.length; i++) ...[
                        if (i > 0) const Divider(height: 1),
                        SwitchListTile(
                          key: Key('agent-skills-app-${_appCatalog[i].id}'),
                          title: Text(_appCatalog[i].name),
                          subtitle: Text(
                            [
                              _appCatalog[i].id,
                              if (_appCatalog[i].description.isNotEmpty)
                                _appCatalog[i].description,
                            ].join(' · '),
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                          value: _isAssigned(_appCatalog[i].id),
                          onChanged: (v) =>
                              unawaited(_toggleApp(_appCatalog[i], v)),
                        ),
                      ],
                  ],
                ),
                const Gap(18),
                Text(
                  l10n.agentSkillsPortableSection,
                  style: theme.textTheme.titleSmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                const Gap(8),
                KimGroupCard(
                  children: [
                    if (scanOff)
                      ListTile(
                        title: Text(l10n.agentSkillsPortableScanOff),
                        subtitle: Text(l10n.agentSkillsPortableScanOffHint),
                      )
                    else if (_portable.isEmpty)
                      ListTile(
                        title: Text(l10n.agentSkillsPortableEmpty),
                        subtitle: Text(l10n.agentSkillsPortableEmptyHint),
                      )
                    else
                      for (var i = 0; i < _portable.length; i++) ...[
                        if (i > 0) const Divider(height: 1),
                        SwitchListTile(
                          key: Key('agent-skills-mute-${_portable[i].id}'),
                          title: Text(_portable[i].name),
                          subtitle: Text(
                            [
                              _portable[i].id,
                              l10n.agentSkillsPortableMuteHint,
                            ].join(' · '),
                          ),
                          // Switch ON = muted (in denylist).
                          value: _denylist.contains(_portable[i].id),
                          onChanged: (muted) => unawaited(
                            _toggleDenylist(_portable[i].id, muted),
                          ),
                        ),
                      ],
                  ],
                ),
                const Gap(20),
                OutlinedButton(
                  key: const Key('agent-skills-plaza'),
                  onPressed: () =>
                      context.push('/agent/plaza?assignTo=${widget.profileId}'),
                  child: Text(l10n.agentSkillsOpenPlaza),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
