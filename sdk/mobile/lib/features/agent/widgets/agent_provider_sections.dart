/// Provider account picker + model row + context window + reasoning blocks.
library;

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/empty_state.dart';
import 'package:kim_mobile/design/kim_group.dart';

class AgentProviderSection extends StatelessWidget {
  const AgentProviderSection({
    super.key,
    required this.accountsEmpty,
    required this.accountId,
    required this.providerItems,
    required this.selectedValue,
    required this.onSelectAccount,
    required this.onAddProvider,
  });

  final bool accountsEmpty;
  final String accountId;
  final List<DropdownMenuItem<String>> providerItems;
  final String? selectedValue;
  final void Function(String next) onSelectAccount;
  final VoidCallback onAddProvider;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    if (accountsEmpty) {
      return EmptyState(
        icon: LucideIcons.key,
        title: l10n.agentEmptyProviders,
        subtitle: l10n.agentEmptyProvidersHint,
        action: FilledButton(
          key: const Key('agent-add-provider'),
          onPressed: onAddProvider,
          child: Text(l10n.agentAddAccount),
        ),
      );
    }
    return KimGroupCard(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
          child: DropdownButton<String>(
            key: const Key('agent-provider'),
            value: () {
              if (selectedValue != null &&
                  providerItems.any((i) => i.value == selectedValue)) {
                return selectedValue;
              }
              return providerItems.first.value;
            }(),
            isExpanded: true,
            items: providerItems,
            onChanged: (next) {
              if (next != null) {
                onSelectAccount(next);
              }
            },
          ),
        ),
      ],
    );
  }
}

class AgentModelSection extends StatelessWidget {
  const AgentModelSection({
    super.key,
    required this.model,
    required this.onPickModel,
  });

  final TextEditingController model;
  final VoidCallback onPickModel;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return KimGroupCard(
      children: [
        ListTile(
          title: Text(Copy.agentModel),
          subtitle: TextField(
            key: const Key('agent-model'),
            controller: model,
            readOnly: true,
            onTap: onPickModel,
            decoration: InputDecoration(
              border: InputBorder.none,
              hintText: l10n.agentModelOther,
              suffixIcon: IconButton(
                tooltip: l10n.agentPickModel,
                onPressed: onPickModel,
                icon: const Icon(LucideIcons.chevronsUpDown, size: 18),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class AgentPromptSection extends StatelessWidget {
  const AgentPromptSection({
    super.key,
    required this.prompt,
    required this.maxLines,
    this.defaultHint = '',
  });

  final TextEditingController prompt;
  final int maxLines;
  final String defaultHint;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return KimGroupCard(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
          child: TextField(
            key: const Key('agent-prompt'),
            controller: prompt,
            maxLines: maxLines,
            decoration: InputDecoration(
              border: InputBorder.none,
              hintText: defaultHint,
              helperText: l10n.agentPromptHint,
            ),
          ),
        ),
      ],
    );
  }
}
