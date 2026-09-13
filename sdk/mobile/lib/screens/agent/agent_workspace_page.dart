library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:toastification/toastification.dart';

import '../../agent/workspace.dart';
import '../../agent/workspace_access.dart';
import '../../copy.dart';
import '../../core/paths.dart';
import '../../state/agent_profiles.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';

class AgentWorkspacePage extends ConsumerStatefulWidget {
  const AgentWorkspacePage({super.key, required this.profileId});

  final String profileId;

  @override
  ConsumerState<AgentWorkspacePage> createState() => _AgentWorkspacePageState();
}

class _AgentWorkspacePageState extends ConsumerState<AgentWorkspacePage> {
  var _fs = false;
  var _fsWrite = false;
  var _bash = false;
  var _loaded = false;
  var _kindRepo = false;
  var _invalidRepo = false;
  String _cwd = '';
  String _repoPath = '';
  String _bookmark = '';
  AgentProfile? _profile;

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
    final paths = KimPaths.instance;
    await paths.ensureAgentDirs();
    final storedBookmark =
        profile.workspace.bookmarkRef.isNotEmpty
            ? profile.workspace.bookmarkRef
            : (await workspaceAccess.loadBookmark(profile.id) ?? '');
    final resolved = await resolveAgentProjectRoot(
      profile: profile,
      paths: paths,
    );
    if (!mounted) {
      return;
    }
    setState(() {
      _profile = profile;
      _fs = profile!.tools.fs || profile.tools.fsWrite;
      _fsWrite = profile.tools.fsWrite;
      _bash = profile.tools.bash;
      _kindRepo = profile.workspace.isRepo;
      _repoPath = profile.workspace.path;
      _bookmark = storedBookmark;
      _cwd = resolved.path;
      _invalidRepo = resolved.invalidRepo;
      _loaded = true;
    });
  }

  void _applyKnowledgePreset() {
    setState(() {
      _kindRepo = false;
      _fs = true;
      _fsWrite = true;
    });
  }

  void _applyCodingPreset() {
    setState(() {
      _kindRepo = true;
      _fs = true;
      _fsWrite = true;
      _bash = true;
    });
    if (_repoPath.isEmpty) {
      unawaited(_pickRepo());
    }
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
      _fs = true;
      _fsWrite = true;
    });
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
  }

  Future<void> _save() async {
    final profile = _profile;
    if (profile == null) {
      return;
    }
    final l10n = AppLocalizations.of(context);
    if (_kindRepo && _repoPath.isEmpty) {
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: Text(l10n.agentWorkspaceRepoRequired),
        autoCloseDuration: const Duration(seconds: 3),
      );
      return;
    }
    if (_kindRepo &&
        !kIsWeb &&
        defaultTargetPlatform == TargetPlatform.macOS &&
        _bookmark.isEmpty &&
        !kDebugMode) {
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: Text(l10n.agentWorkspaceRepoReselect),
        autoCloseDuration: const Duration(seconds: 4),
      );
      return;
    }
    final fs = _fs || _fsWrite;
    final workspace = _kindRepo
        ? WorkspaceSpec(
            kind: WorkspaceSpec.kindRepo,
            path: _repoPath,
            bookmarkRef: _bookmark,
          )
        : WorkspaceSpec.sandbox;
    if (_kindRepo) {
      await workspaceAccess.saveBookmark(profile.id, _bookmark);
    } else {
      await workspaceAccess.clearBookmark(profile.id);
    }
    final store = ref.read(agentProfilesProvider.notifier);
    final next = profile.copyWith(
      workspace: workspace,
      tools: profile.tools.copyWith(
        fs: fs,
        fsWrite: _fsWrite,
        bash: _bash,
      ),
      permissionOverrides: () {
        final map = Map<String, String>.from(profile.permissionOverrides);
        if (_bash) {
          map['bash'] =
              map['bash'] == 'never_allow' ? 'never_allow' : 'ask_before';
        }
        return map;
      }(),
    );
    await store.saveEditor(next);
    if (!mounted) {
      return;
    }
    final resolved = await resolveAgentProjectRoot(
      profile: next,
      paths: KimPaths.instance,
    );
    if (!mounted) {
      return;
    }
    setState(() {
      _profile = next;
      _cwd = resolved.path;
      _fs = fs;
      _invalidRepo = resolved.invalidRepo;
    });
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
            KimSliverHeader(title: l10n.agentWorkspaceTitle),
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
          KimSliverHeader(title: l10n.agentWorkspaceTitle),
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            sliver: SliverList.list(
              children: [
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
                      onPressed: _applyKnowledgePreset,
                      child: Text(l10n.agentWorkspacePresetKnowledge),
                    ),
                    OutlinedButton(
                      key: const Key('agent-workspace-preset-coding'),
                      onPressed: _applyCodingPreset,
                      child: Text(l10n.agentWorkspacePresetCoding),
                    ),
                  ],
                ),
                const Gap(18),
                Text(
                  l10n.agentWorkspacePath,
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
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
                  ],
                ),
                const Gap(18),
                KimGroupCard(
                  children: [
                    SwitchListTile(
                      key: const Key('agent-workspace-fs'),
                      title: Text(l10n.agentFsReadonly),
                      value: _fs,
                      onChanged: (next) => setState(() {
                        _fs = next;
                        if (!next) {
                          _fsWrite = false;
                        }
                      }),
                    ),
                    const Divider(height: 1),
                    SwitchListTile(
                      key: const Key('agent-workspace-fs-write'),
                      title: Text(l10n.agentFsWrite),
                      subtitle: Text(l10n.agentFsWriteHint),
                      value: _fsWrite,
                      onChanged: (next) => setState(() {
                        _fsWrite = next;
                        if (next) {
                          _fs = true;
                        }
                      }),
                    ),
                    const Divider(height: 1),
                    SwitchListTile(
                      key: const Key('agent-workspace-bash'),
                      title: Text(l10n.agentBashDanger),
                      subtitle: Text(l10n.agentBashLater),
                      value: _bash,
                      onChanged: (next) => setState(() => _bash = next),
                    ),
                  ],
                ),
                const Gap(20),
                FilledButton(
                  key: const Key('agent-workspace-save'),
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
