library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';

/// Step 3: system prompt / review.
class AgentCreateStepReview extends StatelessWidget {
  const AgentCreateStepReview({super.key, required this.prompt});

  final TextEditingController prompt;

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
            maxLines: 8,
            decoration: InputDecoration(
              border: InputBorder.none,
              hintText: kDefaultSystemPrompt,
              helperText: l10n.agentPromptHint,
            ),
          ),
        ),
      ],
    );
  }
}
