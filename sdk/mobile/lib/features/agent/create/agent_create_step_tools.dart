library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/features/agent/agent_create_form.dart';
import 'package:kim_mobile/features/agent/agent_permission.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/ask_before_switch.dart';
import 'package:kim_mobile/features/agent/create/agent_create_helpers.dart';
import 'package:kim_mobile/features/agent/workspace_access.dart';

/// Step 1: IM / FS / bash capabilities, workspace, MCP.
class AgentCreateStepTools extends ConsumerWidget {
  const AgentCreateStepTools({super.key, required this.mcp});

  final TextEditingController mcp;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final draft = ref.watch(agentCreateFormProvider);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    final form = ref.read(agentCreateFormProvider.notifier);

    bool capOn(String kind) =>
        draft.caps.any((c) => c.kind == kind && c.enabled);
    final fsOn = capOn(CapabilityKinds.fs);
    var fsWriteOn = false;
    for (final c in draft.caps) {
      if (c.kind == CapabilityKinds.fs && c.enabled) {
        fsWriteOn = c.params['writable'] == true;
      }
    }
    final bashOn = capOn(CapabilityKinds.bash);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          l10n.agentCapabilitiesImSection,
          style: theme.textTheme.titleSmall?.copyWith(
            color: scheme.onSurfaceVariant,
          ),
        ),
        const Gap(8),
        KimGroupCard(
          children: [
            for (final entry in [
              (CapabilityKinds.imSendMessage, l10n.agentToolSendMessage),
              (CapabilityKinds.imReadClipboard, l10n.agentToolClipboard),
              (CapabilityKinds.imSearchContacts, l10n.agentToolSearchContacts),
              (CapabilityKinds.imSearchMessages, l10n.agentToolSearchMessages),
              (
                CapabilityKinds.imGetConversationContext,
                l10n.agentToolGetConversationContext,
              ),
              (CapabilityKinds.imListProfiles, l10n.agentToolListProfiles),
            ]) ...[
              if (entry.$1 != CapabilityKinds.imSendMessage)
                const Divider(height: 1),
              SwitchListTile(
                key: Key('agent-cap-${entry.$1}'),
                title: Text(entry.$2),
                value: capOn(entry.$1),
                onChanged: (v) => _setCap(ref, entry.$1, v),
              ),
              if (entry.$1 == CapabilityKinds.imSendMessage &&
                  capOn(CapabilityKinds.imSendMessage)) ...[
                const Divider(height: 1),
                AskBeforeSwitch(
                  tool: kAskBeforeSendMessage,
                  value: permissionAsksBefore(
                    draft.perms,
                    kAskBeforeSendMessage,
                  ),
                  onChanged: (v) => _setAsk(ref, kAskBeforeSendMessage, v),
                ),
              ],
              if (entry.$1 == CapabilityKinds.imReadClipboard &&
                  capOn(CapabilityKinds.imReadClipboard)) ...[
                const Divider(height: 1),
                AskBeforeSwitch(
                  tool: kAskBeforeReadClipboard,
                  value: permissionAsksBefore(
                    draft.perms,
                    kAskBeforeReadClipboard,
                  ),
                  onChanged: (v) => _setAsk(ref, kAskBeforeReadClipboard, v),
                ),
              ],
            ],
          ],
        ),
        const Gap(18),
        Text(
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
              onPressed: () {
                form.setKindRepo(false);
                _setFs(ref, read: true, write: true);
                unawaited(
                  reloadAgentCreateSkills(
                    ref: ref,
                    mounted: () => context.mounted,
                  ),
                );
              },
              child: Text(l10n.agentWorkspacePresetKnowledge),
            ),
            OutlinedButton(
              onPressed: () {
                if (draft.repoPath.isEmpty) {
                  unawaited(_pickRepo(context, ref));
                } else {
                  form.setKindRepo(true);
                  _setFs(ref, read: true, write: true);
                  _setCap(ref, CapabilityKinds.bash, true);
                  unawaited(
                    reloadAgentCreateSkills(
                      ref: ref,
                      mounted: () => context.mounted,
                    ),
                  );
                }
              },
              child: Text(l10n.agentWorkspacePresetCoding),
            ),
          ],
        ),
        const Gap(8),
        KimGroupCard(
          children: [
            SwitchListTile(
              key: const Key('agent-workspace-kind-repo'),
              title: Text(l10n.agentWorkspaceUseRepo),
              subtitle: Text(l10n.agentWorkspaceUseRepoHint),
              value: draft.kindRepo,
              onChanged: (next) {
                if (next) {
                  unawaited(_pickRepo(context, ref));
                } else {
                  unawaited(_clearRepo(context, ref));
                }
              },
            ),
            if (draft.kindRepo) ...[
              const Divider(height: 1),
              ListTile(
                key: const Key('agent-workspace-pick-repo'),
                title: Text(
                  draft.repoPath.isEmpty
                      ? l10n.agentWorkspacePickRepo
                      : draft.repoPath,
                ),
                trailing: const Icon(Icons.folder_open),
                onTap: () => unawaited(_pickRepo(context, ref)),
              ),
            ],
            const Divider(height: 1),
            SwitchListTile(
              key: const Key('agent-workspace-fs'),
              title: Text(l10n.agentFsReadonly),
              value: fsOn,
              onChanged: (next) =>
                  _setFs(ref, read: next, write: next ? fsWriteOn : false),
            ),
            const Divider(height: 1),
            SwitchListTile(
              key: const Key('agent-workspace-fs-write'),
              title: Text(l10n.agentFsWrite),
              subtitle: Text(l10n.agentFsWriteHint),
              value: fsWriteOn,
              onChanged: (next) => _setFs(ref, read: next || fsOn, write: next),
            ),
            if (fsWriteOn) ...[
              const Divider(height: 1),
              AskBeforeSwitch(
                tool: kAskBeforeWriteFile,
                value: permissionAsksBefore(draft.perms, kAskBeforeWriteFile),
                onChanged: (v) => _setAsk(ref, kAskBeforeWriteFile, v),
              ),
            ],
            const Divider(height: 1),
            SwitchListTile(
              key: const Key('agent-workspace-bash'),
              title: Text(l10n.agentBashDanger),
              subtitle: Text(l10n.agentBashLater),
              value: bashOn,
              onChanged: (next) => _setCap(ref, CapabilityKinds.bash, next),
            ),
            if (bashOn) ...[
              const Divider(height: 1),
              AskBeforeSwitch(
                tool: kAskBeforeBash,
                value: permissionAsksBefore(draft.perms, kAskBeforeBash),
                onChanged: (v) => _setAsk(ref, kAskBeforeBash, v),
              ),
            ],
            const Divider(height: 1),
            SwitchListTile(
              key: const Key('agent-cap-subagent'),
              title: Text(l10n.agentToolDelegate),
              value: capOn(CapabilityKinds.subagent),
              onChanged: (v) => _setCap(ref, CapabilityKinds.subagent, v),
            ),
            if (capOn(CapabilityKinds.subagent)) ...[
              const Divider(height: 1),
              AskBeforeSwitch(
                tool: kAskBeforeDelegate,
                value: permissionAsksBefore(draft.perms, kAskBeforeDelegate),
                onChanged: (v) => _setAsk(ref, kAskBeforeDelegate, v),
              ),
            ],
          ],
        ),
        const Gap(18),
        Text(
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
                controller: mcp,
                maxLines: 4,
                decoration: InputDecoration(
                  border: InputBorder.none,
                  hintText: l10n.agentMcpHint,
                ),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

void _setAsk(WidgetRef ref, String tool, bool ask) {
  final draft = ref.read(agentCreateFormProvider);
  ref
      .read(agentCreateFormProvider.notifier)
      .setPerms(setPermissionAskBefore(draft.perms, tool, ask: ask));
}

void _clearAsk(WidgetRef ref, String tool) {
  final draft = ref.read(agentCreateFormProvider);
  if (!permissionAsksBefore(draft.perms, tool)) {
    return;
  }
  _setAsk(ref, tool, false);
}

void _setCap(
  WidgetRef ref,
  String kind,
  bool on, {
  Map<String, Object?> params = const {},
}) {
  final draft = ref.read(agentCreateFormProvider);
  final caps = upsertCapability(
    draft.caps,
    kind: kind,
    enabled: on,
    params: params,
  );
  var perms = draft.perms;
  if (!on) {
    final tool = switch (kind) {
      CapabilityKinds.imSendMessage => kAskBeforeSendMessage,
      CapabilityKinds.imReadClipboard => kAskBeforeReadClipboard,
      CapabilityKinds.bash => kAskBeforeBash,
      CapabilityKinds.subagent => kAskBeforeDelegate,
      _ => '',
    };
    if (tool.isNotEmpty) {
      perms = setPermissionAskBefore(perms, tool, ask: false);
    }
  }
  if (kind == CapabilityKinds.fs && params['writable'] != true) {
    perms = setPermissionAskBefore(perms, kAskBeforeWriteFile, ask: false);
  }
  ref
      .read(agentCreateFormProvider.notifier)
      .applyCaps(caps: caps, perms: perms);
}

void _setFs(WidgetRef ref, {required bool read, required bool write}) {
  if (!read && !write) {
    _setCap(ref, CapabilityKinds.fs, false);
    _clearAsk(ref, kAskBeforeWriteFile);
    return;
  }
  _setCap(ref, CapabilityKinds.fs, true, params: {'writable': write});
  if (!write) {
    _clearAsk(ref, kAskBeforeWriteFile);
  }
}

Future<void> _pickRepo(BuildContext context, WidgetRef ref) async {
  final draft = ref.read(agentCreateFormProvider);
  final picked = await workspaceAccess.pickDirectory();
  if (picked == null || !context.mounted) {
    return;
  }
  final caps = upsertCapability(
    upsertCapability(
      draft.caps,
      kind: CapabilityKinds.fs,
      enabled: true,
      params: const {'writable': true},
    ),
    kind: CapabilityKinds.bash,
    enabled: true,
  );
  ref
      .read(agentCreateFormProvider.notifier)
      .applyPickedRepo(
        path: picked.path,
        bookmark: picked.bookmarkBase64,
        caps: caps,
      );
  await reloadAgentCreateSkills(ref: ref, mounted: () => context.mounted);
}

Future<void> _clearRepo(BuildContext context, WidgetRef ref) async {
  ref.read(agentCreateFormProvider.notifier).clearRepo();
  await reloadAgentCreateSkills(ref: ref, mounted: () => context.mounted);
}
