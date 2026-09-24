library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';

class AgentCapabilitiesDraft {
  const AgentCapabilitiesDraft({
    this.loaded = false,
    this.profile,
    this.caps = const [],
    this.assigned = const [],
    this.denylist = const [],
    this.appCatalog = const [],
    this.portable = const [],
    this.kindRepo = false,
    this.invalidRepo = false,
    this.cwd = '',
    this.repoPath = '',
    this.bookmark = '',
    this.previewSummary = '',
    this.previewFromHost = false,
  });

  final bool loaded;
  final AgentProfile? profile;
  final List<CapabilityRef> caps;
  final List<SkillRef> assigned;
  final List<String> denylist;
  final List<CatalogSkill> appCatalog;
  final List<CatalogSkill> portable;
  final bool kindRepo;
  final bool invalidRepo;
  final String cwd;
  final String repoPath;
  final String bookmark;
  final String previewSummary;
  final bool previewFromHost;

  AgentCapabilitiesDraft copyWith({
    bool? loaded,
    AgentProfile? profile,
    List<CapabilityRef>? caps,
    List<SkillRef>? assigned,
    List<String>? denylist,
    List<CatalogSkill>? appCatalog,
    List<CatalogSkill>? portable,
    bool? kindRepo,
    bool? invalidRepo,
    String? cwd,
    String? repoPath,
    String? bookmark,
    String? previewSummary,
    bool? previewFromHost,
  }) {
    return AgentCapabilitiesDraft(
      loaded: loaded ?? this.loaded,
      profile: profile ?? this.profile,
      caps: caps ?? this.caps,
      assigned: assigned ?? this.assigned,
      denylist: denylist ?? this.denylist,
      appCatalog: appCatalog ?? this.appCatalog,
      portable: portable ?? this.portable,
      kindRepo: kindRepo ?? this.kindRepo,
      invalidRepo: invalidRepo ?? this.invalidRepo,
      cwd: cwd ?? this.cwd,
      repoPath: repoPath ?? this.repoPath,
      bookmark: bookmark ?? this.bookmark,
      previewSummary: previewSummary ?? this.previewSummary,
      previewFromHost: previewFromHost ?? this.previewFromHost,
    );
  }
}

class AgentCapabilitiesForm extends Notifier<AgentCapabilitiesDraft> {
  @override
  AgentCapabilitiesDraft build() => const AgentCapabilitiesDraft();

  void hydrate({
    required AgentProfile profile,
    required List<CapabilityRef> caps,
    required List<SkillRef> assigned,
    required List<String> denylist,
    required List<CatalogSkill> appCatalog,
    required List<CatalogSkill> portable,
    required bool kindRepo,
    required String repoPath,
    required String bookmark,
    required String cwd,
    required bool invalidRepo,
  }) {
    state = state.copyWith(
      loaded: true,
      profile: profile,
      caps: caps,
      assigned: assigned,
      denylist: denylist,
      appCatalog: appCatalog,
      portable: portable,
      kindRepo: kindRepo,
      repoPath: repoPath,
      bookmark: bookmark,
      cwd: cwd,
      invalidRepo: invalidRepo,
    );
  }

  void setPreview({required bool fromHost, required String summary}) {
    state = state.copyWith(previewFromHost: fromHost, previewSummary: summary);
  }

  void applyEditor({
    required AgentProfile profile,
    required List<CapabilityRef> caps,
    required List<SkillRef> assigned,
    required List<String> denylist,
  }) {
    state = state.copyWith(
      profile: profile,
      caps: caps,
      assigned: assigned,
      denylist: denylist,
    );
  }

  void setKindRepo(bool value) => state = state.copyWith(kindRepo: value);

  void applyWorkspace({
    required bool kindRepo,
    required String repoPath,
    required String bookmark,
    required String cwd,
    required bool invalidRepo,
  }) {
    state = state.copyWith(
      kindRepo: kindRepo,
      repoPath: repoPath,
      bookmark: bookmark,
      cwd: cwd,
      invalidRepo: invalidRepo,
    );
  }

  void setCwd(String cwd) => state = state.copyWith(cwd: cwd);
}

final agentCapabilitiesFormProvider =
    NotifierProvider.autoDispose<AgentCapabilitiesForm, AgentCapabilitiesDraft>(
      AgentCapabilitiesForm.new,
    );
