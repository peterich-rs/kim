/// IM section of the capabilities sheet: IM tool switches + ask-before rows.
library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/features/agent/providers/agent_permission.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';
import 'package:kim_mobile/features/agent/widgets/ask_before_switch.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/copy.dart';

class ImCapabilitiesSection extends StatelessWidget {
  const ImCapabilitiesSection({
    super.key,
    required this.sectionKey,
    required this.capOn,
    required this.permissionAsksBefore,
    required this.onToggleIm,
    required this.onSetAsk,
  });

  final Key sectionKey;
  final bool Function(String kind) capOn;
  final bool Function(String tool) permissionAsksBefore;
  final void Function(String kind, bool on) onToggleIm;
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
          l10n.agentCapabilitiesImSection,
          style: theme.textTheme.titleSmall?.copyWith(
            color: scheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 8),
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
                onChanged: (v) => onToggleIm(entry.$1, v),
              ),
              if (entry.$1 == CapabilityKinds.imSendMessage &&
                  capOn(CapabilityKinds.imSendMessage)) ...[
                const Divider(height: 1),
                AskBeforeSwitch(
                  tool: kAskBeforeSendMessage,
                  value: permissionAsksBefore(kAskBeforeSendMessage),
                  onChanged: (v) => onSetAsk(kAskBeforeSendMessage, v),
                ),
              ],
              if (entry.$1 == CapabilityKinds.imReadClipboard &&
                  capOn(CapabilityKinds.imReadClipboard)) ...[
                const Divider(height: 1),
                AskBeforeSwitch(
                  tool: kAskBeforeReadClipboard,
                  value: permissionAsksBefore(kAskBeforeReadClipboard),
                  onChanged: (v) => onSetAsk(kAskBeforeReadClipboard, v),
                ),
              ],
            ],
          ],
        ),
      ],
    );
  }
}
