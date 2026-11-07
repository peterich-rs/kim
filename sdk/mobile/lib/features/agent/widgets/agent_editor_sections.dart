/// Basics card (display name + aliases) of the agent editor.
library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/kim_group.dart';

class AgentBasicsSection extends StatelessWidget {
  const AgentBasicsSection({
    super.key,
    required this.isCreate,
    required this.displayName,
    required this.aliases,
    this.onNameChanged,
  });

  final bool isCreate;
  final TextEditingController displayName;
  final TextEditingController? aliases;
  final VoidCallback? onNameChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return KimGroupCard(
      children: [
        ListTile(
          title: Text(l10n.agentDisplayName),
          subtitle: TextField(
            key: const Key('agent-name'),
            controller: displayName,
            decoration: InputDecoration(
              border: InputBorder.none,
              hintText: l10n.agentNameHint,
            ),
            onChanged: (_) => onNameChanged?.call(),
          ),
        ),
        if (!isCreate && aliases != null) ...[
          const Divider(height: 1),
          ListTile(
            title: Text(l10n.agentAliases),
            subtitle: TextField(
              controller: aliases,
              decoration: InputDecoration(
                border: InputBorder.none,
                hintText: l10n.agentAliasesHint,
              ),
            ),
          ),
        ],
      ],
    );
  }
}

/// Section label shared by the editor's grouped blocks.
class AgentSectionLabel extends StatelessWidget {
  const AgentSectionLabel({super.key, required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Text(
      label,
      style: theme.textTheme.labelLarge?.copyWith(
        color: theme.colorScheme.onSurfaceVariant,
      ),
    );
  }
}
