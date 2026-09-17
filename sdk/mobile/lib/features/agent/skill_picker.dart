library;

import 'package:flutter/material.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';
import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/design/kim_theme.dart';

String skillShelfLabel(AppLocalizations l10n, SkillShelf shelf) {
  return switch (shelf) {
    SkillShelf.internal => l10n.agentSkillTagInternal,
    SkillShelf.download => l10n.agentSkillTagDownload,
    SkillShelf.global => l10n.agentSkillTagGlobal,
    SkillShelf.project => l10n.agentSkillTagProject,
  };
}

class SkillOriginChip extends StatelessWidget {
  const SkillOriginChip({super.key, required this.shelf});

  final SkillShelf shelf;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(KimTheme.radiusControl),
        border: Border.all(color: KimTheme.hairlineOf(context)),
      ),
      child: Text(
        skillShelfLabel(l10n, shelf),
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
          color: scheme.onSurfaceVariant,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}

/// Combined app + portable catalog. Checkboxes are multi-select.
class SkillPickerList extends StatelessWidget {
  const SkillPickerList({
    super.key,
    required this.skills,
    required this.selectedIds,
    required this.onChanged,
    this.empty,
  });

  final List<CatalogSkill> skills;
  final Set<String> selectedIds;
  final void Function(CatalogSkill skill, bool selected) onChanged;
  final Widget? empty;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    if (skills.isEmpty) {
      return empty ??
          KimGroupCard(
            children: [ListTile(title: Text(l10n.agentSkillsPickerEmpty))],
          );
    }
    return KimGroupCard(
      children: [
        for (var i = 0; i < skills.length; i++) ...[
          if (i > 0) const Divider(height: 1),
          _SkillTile(
            skill: skills[i],
            selected: selectedIds.contains(skills[i].id),
            onChanged: (next) => onChanged(skills[i], next),
          ),
        ],
      ],
    );
  }
}

class _SkillTile extends StatelessWidget {
  const _SkillTile({
    required this.skill,
    required this.selected,
    required this.onChanged,
  });

  final CatalogSkill skill;
  final bool selected;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    final desc = skill.listDescription;
    return CheckboxListTile(
      key: Key(
        skill.isApp
            ? 'agent-skills-app-${skill.id}'
            : 'agent-skills-portable-${skill.id}',
      ),
      value: selected,
      onChanged: (next) {
        if (next == null) {
          return;
        }
        onChanged(next);
      },
      controlAffinity: ListTileControlAffinity.leading,
      title: Row(
        children: [
          Expanded(child: Text(skill.name, overflow: TextOverflow.ellipsis)),
          const SizedBox(width: 8),
          SkillOriginChip(shelf: skill.shelf),
        ],
      ),
      subtitle: desc.isEmpty
          ? null
          : Text(desc, maxLines: 2, overflow: TextOverflow.ellipsis),
    );
  }
}
