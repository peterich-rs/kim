/// Enable-flow dialog for an app skill that requires missing capabilities.
library;

import 'package:flutter/material.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/features/agent/data/skills_catalog.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';

/// Returns the caps / skills / permissionOverrides to persist, or null when
/// the user declined the enable-tools dialog.
Future<AppSkillEnableDecision?> confirmAppSkillEnable({
  required BuildContext context,
  required AgentProfile profile,
  required List<CapabilityRef> caps,
  required List<SkillRef> assigned,
  required CatalogSkill skill,
}) async {
  final l10n = AppLocalizations.of(context);
  final missing = missingToolsForAppSkill(
    profile.withCapabilities(caps),
    skill.id,
  );
  if (missing.isEmpty) {
    return AppSkillEnableDecision(
      caps: caps,
      skills: [
        for (final s in assigned)
          if (s.id != skill.id) s,
        appSkillRef(skill),
      ],
      permissionOverrides: profile.permissionOverrides,
    );
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
          key: const Key('agent-skills-enable-tools'),
          onPressed: () => Navigator.pop(ctx, true),
          child: Text(l10n.agentSkillsNeedsToolsEnable),
        ),
      ],
    ),
  );
  if (open != true) {
    if (context.mounted) {
      toastification.show(
        context: context,
        type: ToastificationType.warning,
        title: Text(l10n.agentSkillsNeedsToolsToast),
        autoCloseDuration: const Duration(seconds: 3),
      );
    }
    return null;
  }
  return AppSkillEnableDecision(
    caps: enableRequiredCapabilities(caps, missing),
    skills: [
      for (final s in assigned)
        if (s.id != skill.id) s,
      appSkillRef(skill),
    ],
    permissionOverrides: profile.permissionOverrides,
  );
}

class AppSkillEnableDecision {
  const AppSkillEnableDecision({
    required this.caps,
    required this.skills,
    required this.permissionOverrides,
  });

  final List<CapabilityRef> caps;
  final List<SkillRef> skills;
  final Map<String, String> permissionOverrides;
}
