library;

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';

String agentWorkspaceSubtitle(AppLocalizations l10n, AgentProfile profile) {
  final tools = projectToolSet(profile.resolveCapabilities());
  final fs = tools.fs || tools.fsWrite;
  final write = tools.fsWrite;
  final bash = tools.bash;
  final kind = profile.workspace.isRepo
      ? l10n.agentWorkspaceKindRepo
      : l10n.agentWorkspaceKindSandbox;
  final caps = () {
    if (write && bash) {
      return l10n.agentWorkspaceCapsWriteBash;
    }
    if (write) {
      return l10n.agentWorkspaceCapsWrite;
    }
    if (fs && bash) {
      return l10n.agentWorkspaceCapsFsBash;
    }
    if (fs) {
      return l10n.agentWorkspaceCapsFs;
    }
    if (bash) {
      return l10n.agentWorkspaceCapsBash;
    }
    return l10n.agentWorkspaceCapsOff;
  }();
  return '$kind · $caps';
}

String agentSkillsSubtitle(AppLocalizations l10n, AgentProfile profile) {
  final assigned = [
    for (final s in profile.skills)
      if (s.enabled && s.id.isNotEmpty) s.id,
  ];
  if (assigned.isEmpty) {
    return l10n.agentSkillsNone;
  }
  return assigned.join(' · ');
}

String agentToolsSubtitle(AppLocalizations l10n, AgentProfile profile) {
  final mcp = profile.resolveCapabilities().where(
    (c) => c.enabled && c.kind == CapabilityKinds.mcp,
  );
  final count = mcp.length;
  if (count == 0) {
    return l10n.agentToolsSubtitleDefault;
  }
  return l10n.agentToolsSubtitleMcp(count);
}

String agentCapabilitiesSubtitle(AppLocalizations l10n, AgentProfile profile) {
  final names = projectedToolNames(profile.resolveCapabilities());
  final skills = [
    for (final s in profile.skills)
      if (s.enabled && s.id.isNotEmpty) s.id,
  ];
  if (names.isEmpty && skills.isEmpty) {
    return l10n.agentCapabilitiesSubtitleEmpty;
  }
  final parts = <String>[
    if (names.isNotEmpty) l10n.agentCapabilitiesSubtitleTools(names.length),
    if (skills.isNotEmpty) l10n.agentCapabilitiesSubtitleSkills(skills.length),
  ];
  return parts.join(' · ');
}
