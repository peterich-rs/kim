/// Persist + capability mutation logic for the capabilities sheet. The page
/// builds sections; this file owns the save path and rollback.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/features/agent/providers/agent_capabilities_form.dart';
import 'package:kim_mobile/features/agent/providers/agent_permission.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';

class AgentCapabilitiesController {
  AgentCapabilitiesController({
    required this.form,
    required this.draft,
    required this.saveEditor,
    required this.savedProfiles,
    required this.mcp,
    required this.context,
  });

  final AgentCapabilitiesForm form;
  final AgentCapabilitiesDraft draft;
  final Future<void> Function(AgentProfile profile) saveEditor;
  final List<AgentProfile> Function() savedProfiles;
  final TextEditingController mcp;
  final BuildContext context;

  AgentCapabilitiesForm get _form => form;
  AgentCapabilitiesDraft get _draft => draft;
  AgentProfile? get _profile => _draft.profile;
  List<CapabilityRef> get _caps => _draft.caps;
  List<SkillRef> get _assigned => _draft.assigned;
  List<String> get _denylist => _draft.denylist;
  bool get _kindRepo => _draft.kindRepo;
  String get _repoPath => _draft.repoPath;
  String get _bookmark => _draft.bookmark;

  bool capOn(String kind) => _caps.any((c) => c.kind == kind && c.enabled);

  bool get fsOn => capOn(CapabilityKinds.fs);
  bool get bashOn => capOn(CapabilityKinds.bash);

  bool get fsWriteOn {
    for (final c in _caps) {
      if (c.kind == CapabilityKinds.fs && c.enabled) {
        return c.params['writable'] == true;
      }
    }
    return false;
  }

  bool asksBefore(String tool) =>
      permissionAsksBefore(_profile?.permissionOverrides ?? const {}, tool);

  WorkspaceSpec workspaceSpec() {
    return _kindRepo
        ? WorkspaceSpec(
            kind: WorkspaceSpec.kindRepo,
            path: _repoPath,
            bookmarkRef: _bookmark,
          )
        : WorkspaceSpec.sandbox;
  }

  List<CapabilityRef> capsWithMcpFromField() {
    return mergeCapsWithMcpLines(_caps, mcp.text);
  }

  Future<void> persist({
    List<CapabilityRef>? caps,
    List<SkillRef>? skills,
    List<String>? denylist,
    WorkspaceSpec? workspace,
    Map<String, String>? permissionOverrides,
    bool toast = true,
    String? profileId,
  }) async {
    final profile = _profile;
    if (profile == null || !context.mounted) {
      return;
    }
    final l10n = AppLocalizations.of(context);
    final nextCaps = mergeCapsWithMcpLines(caps ?? _caps, mcp.text);
    final next = profile
        .withCapabilities(nextCaps)
        .copyWith(
          skills: skills ?? _assigned,
          portableDenylist: denylist ?? _denylist,
          workspace: workspace ?? workspaceSpec(),
          permissionOverrides: permissionOverrides,
        );
    _form.applyEditor(
      profile: next,
      caps: List<CapabilityRef>.from(next.resolveCapabilities()),
      assigned: List<SkillRef>.from(next.skills),
      denylist: List<String>.from(next.portableDenylist),
    );
    try {
      await saveEditor(next);
    } catch (_) {
      if (!context.mounted) {
        return;
      }
      AgentProfile? rolled;
      for (final p in savedProfiles()) {
        if (p.id == (profileId ?? profile.id)) {
          rolled = p;
          break;
        }
      }
      if (rolled != null) {
        final rolledCaps = List<CapabilityRef>.from(
          rolled.resolveCapabilities(),
        );
        mcp.text = mcpTextFromCaps(rolledCaps);
        _form.applyEditor(
          profile: rolled,
          caps: rolledCaps,
          assigned: List<SkillRef>.from(rolled.skills),
          denylist: List<String>.from(rolled.portableDenylist),
        );
      }
      toastification.show(
        context: context,
        type: ToastificationType.error,
        title: Text(l10n.agentSaveFailed),
        autoCloseDuration: const Duration(seconds: 3),
      );
      return;
    }
    if (!context.mounted || !toast) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.success,
      title: Text(Copy.agentSaved),
      autoCloseDuration: const Duration(seconds: 2),
    );
  }

  Future<void> toggleIm(String kind, bool on) async {
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
    await persist(caps: next, permissionOverrides: perms);
  }

  Future<void> setAsk(String tool, bool ask) async {
    final perms = setPermissionAskBefore(
      _profile?.permissionOverrides ?? const {},
      tool,
      ask: ask,
    );
    await persist(permissionOverrides: perms);
  }

  Future<void> setFs({required bool read, required bool write}) async {
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
    await persist(caps: next, permissionOverrides: perms);
  }

  Future<void> setBash(bool on) async {
    final next = upsertCapability(
      _caps,
      kind: CapabilityKinds.bash,
      enabled: on,
    );
    var perms = Map<String, String>.from(_profile?.permissionOverrides ?? {});
    if (!on) {
      perms = setPermissionAskBefore(perms, kAskBeforeBash, ask: false);
    }
    await persist(caps: next, permissionOverrides: perms);
  }

  Future<void> setSubagent(bool on) async {
    final next = upsertCapability(
      _caps,
      kind: CapabilityKinds.subagent,
      enabled: on,
    );
    var perms = Map<String, String>.from(_profile?.permissionOverrides ?? {});
    if (!on) {
      perms = setPermissionAskBefore(perms, kAskBeforeDelegate, ask: false);
    }
    await persist(caps: next, permissionOverrides: perms);
  }

  Future<void> applyPickedWorkspace(PickedWorkspaceValue picked) async {
    _form.applyWorkspace(
      kindRepo: true,
      repoPath: picked.path,
      bookmark: picked.bookmarkBase64,
      cwd: picked.path,
      invalidRepo: false,
    );
    var next = List<CapabilityRef>.from(_caps);
    next = upsertCapability(
      next,
      kind: CapabilityKinds.fs,
      enabled: true,
      params: const {'writable': true},
    );
    next = upsertCapability(next, kind: CapabilityKinds.bash, enabled: true);
    await persist(caps: next, workspace: workspaceSpec());
  }

  Future<void> clearRepo(String cwd) async {
    _form.applyWorkspace(
      kindRepo: false,
      repoPath: '',
      bookmark: '',
      cwd: cwd,
      invalidRepo: false,
    );
    await persist(workspace: WorkspaceSpec.sandbox);
  }

  Future<void> saveMcp() async {
    await persist();
  }
}

/// View-value mirror of the picker result so this file does not import the
/// data layer.
class PickedWorkspaceValue {
  const PickedWorkspaceValue({required this.path, this.bookmarkBase64 = ''});

  final String path;
  final String bookmarkBase64;
}
