/// MCP section: server list editor + save row.
library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/copy.dart';

class McpCapabilitiesSection extends StatelessWidget {
  const McpCapabilitiesSection({
    super.key,
    required this.sectionKey,
    required this.controller,
    required this.onSave,
  });

  final Key sectionKey;
  final TextEditingController controller;
  final VoidCallback onSave;

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
          l10n.agentCapabilitiesMcpSection,
          style: theme.textTheme.titleSmall?.copyWith(
            color: scheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 8),
        KimGroupCard(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
              child: TextField(
                key: const Key('agent-tools-mcp'),
                controller: controller,
                maxLines: 4,
                onEditingComplete: onSave,
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
              onTap: onSave,
            ),
          ],
        ),
      ],
    );
  }
}
