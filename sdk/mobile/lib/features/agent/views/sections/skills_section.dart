/// Skills section: merged catalog picker with app/portable toggles.
library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/features/agent/data/skills_catalog.dart';
import 'package:kim_mobile/features/agent/providers/agent_profiles.dart';
import 'package:kim_mobile/features/agent/widgets/skill_picker.dart';

class SkillsCapabilitiesSection extends StatelessWidget {
  const SkillsCapabilitiesSection({
    super.key,
    required this.sectionKey,
    required this.appCatalog,
    required this.portable,
    required this.assigned,
    required this.denylist,
    required this.onToggleApp,
    required this.onToggleDenylist,
  });

  final Key sectionKey;
  final List<CatalogSkill> appCatalog;
  final List<CatalogSkill> portable;
  final List<SkillRef> assigned;
  final List<String> denylist;
  final void Function(CatalogSkill skill, bool enable) onToggleApp;
  final void Function(String id, bool muted) onToggleDenylist;

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
          l10n.agentCapabilitiesSkillsSection,
          style: theme.textTheme.titleSmall?.copyWith(
            color: scheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 8),
        SkillPickerList(
          skills: mergeSkillCatalogs(app: appCatalog, portable: portable),
          selectedIds: {
            for (final s in assigned)
              if (s.enabled) s.id,
            for (final s in portable)
              if (!denylist.contains(s.id)) s.id,
          },
          onChanged: (skill, selected) {
            if (skill.isApp) {
              onToggleApp(skill, selected);
              return;
            }
            onToggleDenylist(skill.id, !selected);
          },
        ),
      ],
    );
  }
}
