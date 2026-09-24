library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/features/agent/agent_create_form.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/create/agent_create_helpers.dart';
import 'package:kim_mobile/features/agent/skill_picker.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';

/// Step 2: app + portable skill picker.
class AgentCreateStepSkills extends ConsumerWidget {
  const AgentCreateStepSkills({
    super.key,
    required this.displayName,
    required this.model,
    required this.prompt,
    required this.mcp,
  });

  final TextEditingController displayName;
  final TextEditingController model;
  final TextEditingController prompt;
  final TextEditingController mcp;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final draft = ref.watch(agentCreateFormProvider);
    final merged = mergeSkillCatalogs(app: draft.app, portable: draft.portable);
    final selected = <String>{...draft.appSelected, ...draft.portableSelected};
    return SkillPickerList(
      skills: merged,
      selectedIds: selected,
      onChanged: (skill, next) =>
          unawaited(_toggleSkill(context, ref, skill, next)),
    );
  }

  Future<void> _toggleSkill(
    BuildContext context,
    WidgetRef ref,
    CatalogSkill skill,
    bool selected,
  ) async {
    final form = ref.read(agentCreateFormProvider.notifier);
    if (skill.isApp) {
      if (selected) {
        if (!await _confirmMissingTools(context, ref, skill)) {
          return;
        }
        form.toggleApp(skill.id, true);
      } else {
        form.toggleApp(skill.id, false);
      }
      return;
    }
    form.togglePortable(skill.id, selected);
  }

  Future<bool> _confirmMissingTools(
    BuildContext context,
    WidgetRef ref,
    CatalogSkill skill,
  ) async {
    final l10n = AppLocalizations.of(context);
    final draft = ref.read(agentCreateFormProvider);
    final account = agentCreateSelectedAccount(ref, draft.accountId);
    final profile = AgentProfile(
      id: 'draft',
      displayName: displayName.text.trim(),
      providerKind: account?.vendorId ?? '',
      baseUrl: account?.baseUrl ?? '',
      model: model.text.trim(),
      keyRef: '',
      systemPrompt: prompt.text,
      capabilities: mergeCapsWithMcpLines(draft.caps, mcp.text),
    );
    final missing = missingToolsForAppSkill(profile, skill.id);
    if (missing.isEmpty) {
      return true;
    }
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
            onPressed: () => Navigator.pop(ctx, true),
            child: Text(l10n.agentSkillsNeedsToolsEnable),
          ),
        ],
      ),
    );
    if (open != true || !context.mounted) {
      if (context.mounted) {
        agentCreateToastInfo(context, l10n.agentSkillsNeedsToolsToast);
      }
      return false;
    }
    ref
        .read(agentCreateFormProvider.notifier)
        .setCaps(enableRequiredCapabilities(draft.caps, missing));
    return true;
  }
}
