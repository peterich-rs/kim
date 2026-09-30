/// FS / workspace section: presets, repo picker, fs + bash switches.
library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/features/agent/providers/agent_permission.dart';
import 'package:kim_mobile/features/agent/widgets/ask_before_switch.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/copy.dart';

class FsCapabilitiesSection extends StatelessWidget {
  const FsCapabilitiesSection({
    super.key,
    required this.sectionKey,
    required this.fsOn,
    required this.fsWriteOn,
    required this.bashOn,
    required this.kindRepo,
    required this.invalidRepo,
    required this.cwd,
    required this.repoPath,
    required this.permissionAsksBefore,
    required this.onPresetKnowledge,
    required this.onPresetCoding,
    required this.onPickRepo,
    required this.onClearRepo,
    required this.onSetFs,
    required this.onSetBash,
    required this.onSetAsk,
    required this.subagentOn,
    required this.onSetSubagent,
  });

  final Key sectionKey;
  final bool fsOn;
  final bool fsWriteOn;
  final bool bashOn;
  final bool subagentOn;
  final bool kindRepo;
  final bool invalidRepo;
  final String cwd;
  final String repoPath;
  final bool Function(String tool) permissionAsksBefore;
  final VoidCallback onPresetKnowledge;
  final VoidCallback onPresetCoding;
  final VoidCallback onPickRepo;
  final VoidCallback onClearRepo;
  final void Function({required bool read, required bool write}) onSetFs;
  final void Function(bool on) onSetBash;
  final void Function(bool on) onSetSubagent;
  final void Function(String tool, bool ask) onSetAsk;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          key: sectionKey,
          l10n.agentCapabilitiesFsSection,
          style: theme.textTheme.titleSmall?.copyWith(
            color: scheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 8),
        Text(
          l10n.agentWorkspacePresetHint,
          style: theme.textTheme.bodySmall?.copyWith(
            color: scheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          children: [
            OutlinedButton(
              key: const Key('agent-workspace-preset-knowledge'),
              onPressed: onPresetKnowledge,
              child: Text(l10n.agentWorkspacePresetKnowledge),
            ),
            OutlinedButton(
              key: const Key('agent-workspace-preset-coding'),
              onPressed: onPresetCoding,
              child: Text(l10n.agentWorkspacePresetCoding),
            ),
          ],
        ),
        const SizedBox(height: 8),
        KimGroupCard(
          children: [
            ListTile(
              key: const Key('agent-workspace-cwd'),
              title: Text(
                cwd,
                style: theme.textTheme.bodyMedium?.copyWith(
                  fontFamily: 'monospace',
                ),
              ),
              subtitle: Text(
                kindRepo
                    ? l10n.agentWorkspaceKindRepo
                    : l10n.agentWorkspaceKindSandbox,
              ),
            ),
            const Divider(height: 1),
            SwitchListTile(
              key: const Key('agent-workspace-kind-repo'),
              title: Text(l10n.agentWorkspaceUseRepo),
              subtitle: Text(l10n.agentWorkspaceUseRepoHint),
              value: kindRepo,
              onChanged: (next) => next ? onPickRepo() : onClearRepo(),
            ),
            if (kindRepo) ...[
              const Divider(height: 1),
              ListTile(
                key: const Key('agent-workspace-pick-repo'),
                title: Text(
                  repoPath.isEmpty ? l10n.agentWorkspacePickRepo : repoPath,
                ),
                subtitle: invalidRepo
                    ? Text(
                        l10n.agentWorkspaceRepoReselect,
                        style: TextStyle(color: scheme.error),
                      )
                    : null,
                trailing: const Icon(Icons.folder_open),
                onTap: onPickRepo,
              ),
            ],
            const Divider(height: 1),
            SwitchListTile(
              key: const Key('agent-workspace-fs'),
              title: Text(l10n.agentFsReadonly),
              value: fsOn,
              onChanged: (next) =>
                  onSetFs(read: next, write: next ? fsWriteOn : false),
            ),
            const Divider(height: 1),
            SwitchListTile(
              key: const Key('agent-workspace-fs-write'),
              title: Text(l10n.agentFsWrite),
              subtitle: Text(l10n.agentFsWriteHint),
              value: fsWriteOn,
              onChanged: (next) => onSetFs(read: next || fsOn, write: next),
            ),
            if (fsWriteOn) ...[
              const Divider(height: 1),
              AskBeforeSwitch(
                tool: kAskBeforeWriteFile,
                value: permissionAsksBefore(kAskBeforeWriteFile),
                onChanged: (v) => onSetAsk(kAskBeforeWriteFile, v),
              ),
            ],
            const Divider(height: 1),
            SwitchListTile(
              key: const Key('agent-workspace-bash'),
              title: Text(l10n.agentBashDanger),
              subtitle: Text(l10n.agentBashLater),
              value: bashOn,
              onChanged: onSetBash,
            ),
            if (bashOn) ...[
              const Divider(height: 1),
              AskBeforeSwitch(
                tool: kAskBeforeBash,
                value: permissionAsksBefore(kAskBeforeBash),
                onChanged: (v) => onSetAsk(kAskBeforeBash, v),
              ),
            ],
            const Divider(height: 1),
            SwitchListTile(
              key: const Key('agent-cap-subagent'),
              title: Text(l10n.agentToolDelegate),
              value: subagentOn,
              onChanged: onSetSubagent,
            ),
            if (subagentOn) ...[
              const Divider(height: 1),
              AskBeforeSwitch(
                tool: kAskBeforeDelegate,
                value: permissionAsksBefore(kAskBeforeDelegate),
                onChanged: (v) => onSetAsk(kAskBeforeDelegate, v),
              ),
            ],
          ],
        ),
      ],
    );
  }
}
