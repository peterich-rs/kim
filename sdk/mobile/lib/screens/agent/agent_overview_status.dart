library;

import '../../copy.dart';
import '../../state/agent_profiles.dart';

String agentWorkspaceSubtitle(AppLocalizations l10n, AgentProfile profile) {
  final fs = profile.tools.fs || profile.tools.fsWrite;
  final write = profile.tools.fsWrite;
  final bash = profile.tools.bash;
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
  final mcp = profile.extensions
      .where((e) => e.name.isNotEmpty && e.command.isNotEmpty)
      .length;
  if (mcp == 0) {
    return l10n.agentToolsSubtitleDefault;
  }
  return l10n.agentToolsSubtitleMcp(mcp);
}
