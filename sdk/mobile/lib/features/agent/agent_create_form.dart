library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/catalog.dart';
import 'package:kim_mobile/features/agent/context_window.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';

/// Draft for the create wizard. Text controllers stay on the step widgets.
class AgentCreateDraft {
  const AgentCreateDraft({
    this.step = 0,
    this.accountId = '',
    this.runtimeCodex = false,
    this.contextTokens = kDefaultContextTokens,
    this.kindRepo = false,
    this.repoPath = '',
    this.bookmark = '',
    this.saving = false,
    this.caps = kCreateDefaultCapabilities,
    this.appSelected = const {},
    this.vendors = const [],
    this.pendingModels = const [],
    this.choice = const ReasoningChoice(kind: 'none'),
    this.surface = const ReasoningSurface(kind: 'none'),
    this.perms = const {},
    this.app = const [],
    this.portable = const [],
    this.portableSelected = const {},
    this.portableHydrated = false,
    this.revision = 0,
  });

  final int step;
  final String accountId;
  final bool runtimeCodex;
  final int contextTokens;
  final bool kindRepo;
  final String repoPath;
  final String bookmark;
  final bool saving;
  final List<CapabilityRef> caps;
  final Set<String> appSelected;
  final List<VendorSummary> vendors;
  final List<String> pendingModels;
  final ReasoningChoice choice;
  final ReasoningSurface surface;
  final Map<String, String> perms;
  final List<CatalogSkill> app;
  final List<CatalogSkill> portable;
  final Set<String> portableSelected;
  final bool portableHydrated;
  final int revision;

  AgentCreateDraft copyWith({
    int? step,
    String? accountId,
    bool? runtimeCodex,
    int? contextTokens,
    bool? kindRepo,
    String? repoPath,
    String? bookmark,
    bool? saving,
    List<CapabilityRef>? caps,
    Set<String>? appSelected,
    List<VendorSummary>? vendors,
    List<String>? pendingModels,
    ReasoningChoice? choice,
    ReasoningSurface? surface,
    Map<String, String>? perms,
    List<CatalogSkill>? app,
    List<CatalogSkill>? portable,
    Set<String>? portableSelected,
    bool? portableHydrated,
    int? revision,
  }) {
    return AgentCreateDraft(
      step: step ?? this.step,
      accountId: accountId ?? this.accountId,
      runtimeCodex: runtimeCodex ?? this.runtimeCodex,
      contextTokens: contextTokens ?? this.contextTokens,
      kindRepo: kindRepo ?? this.kindRepo,
      repoPath: repoPath ?? this.repoPath,
      bookmark: bookmark ?? this.bookmark,
      saving: saving ?? this.saving,
      caps: caps ?? this.caps,
      appSelected: appSelected ?? this.appSelected,
      vendors: vendors ?? this.vendors,
      pendingModels: pendingModels ?? this.pendingModels,
      choice: choice ?? this.choice,
      surface: surface ?? this.surface,
      perms: perms ?? this.perms,
      app: app ?? this.app,
      portable: portable ?? this.portable,
      portableSelected: portableSelected ?? this.portableSelected,
      portableHydrated: portableHydrated ?? this.portableHydrated,
      revision: revision ?? this.revision,
    );
  }
}

class AgentCreateForm extends Notifier<AgentCreateDraft> {
  @override
  AgentCreateDraft build() => const AgentCreateDraft();

  void bump() => state = state.copyWith(revision: state.revision + 1);

  void setStep(int step) => state = state.copyWith(step: step);

  void next() => state = state.copyWith(step: state.step + 1);

  void back() {
    if (state.step == 0) {
      return;
    }
    state = state.copyWith(step: state.step - 1);
  }

  void setAccount(String id) => state = state.copyWith(accountId: id);

  void seedAccount({required String accountId, required int contextTokens}) {
    state = state.copyWith(accountId: accountId, contextTokens: contextTokens);
  }

  void bindAccount({required String id, required int contextTokens}) {
    state = state.copyWith(
      accountId: id,
      pendingModels: const [],
      contextTokens: contextTokens,
    );
  }

  void setRuntimeCodex(bool value) =>
      state = state.copyWith(runtimeCodex: value);

  void setContextTokens(int value) =>
      state = state.copyWith(contextTokens: value);

  void setKindRepo(bool value) => state = state.copyWith(kindRepo: value);

  void setRepo({required String path, required String bookmark}) {
    state = state.copyWith(repoPath: path, bookmark: bookmark, kindRepo: true);
  }

  void applyPickedRepo({
    required String path,
    required String bookmark,
    required List<CapabilityRef> caps,
  }) {
    state = state.copyWith(
      kindRepo: true,
      repoPath: path,
      bookmark: bookmark,
      caps: caps,
    );
  }

  void clearRepo() {
    state = state.copyWith(kindRepo: false, repoPath: '', bookmark: '');
  }

  void setSaving(bool value) => state = state.copyWith(saving: value);

  void setCaps(List<CapabilityRef> caps) => state = state.copyWith(caps: caps);

  void applyCaps({
    required List<CapabilityRef> caps,
    required Map<String, String> perms,
  }) {
    state = state.copyWith(caps: caps, perms: perms);
  }

  void setPerms(Map<String, String> perms) =>
      state = state.copyWith(perms: perms);

  void setVendors(List<VendorSummary> vendors) =>
      state = state.copyWith(vendors: vendors);

  void setPendingModels(List<String> models) =>
      state = state.copyWith(pendingModels: models);

  void rememberCustomModel({
    List<String>? pendingModels,
    required int contextTokens,
  }) {
    state = state.copyWith(
      pendingModels: pendingModels ?? state.pendingModels,
      contextTokens: contextTokens,
    );
  }

  void setChoice(ReasoningChoice choice) =>
      state = state.copyWith(choice: choice);

  void setSurface({
    required ReasoningSurface surface,
    required ReasoningChoice choice,
  }) {
    state = state.copyWith(surface: surface, choice: choice);
  }

  void replaceCatalog({
    required List<CatalogSkill> app,
    required List<CatalogSkill> portable,
  }) {
    if (!state.portableHydrated) {
      state = state.copyWith(
        app: app,
        portable: portable,
        portableSelected: {for (final skill in portable) skill.id},
        portableHydrated: true,
      );
      return;
    }
    final keep = state.portableSelected;
    state = state.copyWith(
      app: app,
      portable: portable,
      portableSelected: {
        for (final skill in portable)
          if (keep.contains(skill.id)) skill.id,
      },
    );
  }

  void toggleApp(String id, bool selected) {
    final next = {...state.appSelected};
    if (selected) {
      next.add(id);
    } else {
      next.remove(id);
    }
    state = state.copyWith(appSelected: next);
  }

  void togglePortable(String id, bool selected) {
    final next = {...state.portableSelected};
    if (selected) {
      next.add(id);
    } else {
      next.remove(id);
    }
    state = state.copyWith(portableSelected: next);
  }
}

final agentCreateFormProvider =
    NotifierProvider.autoDispose<AgentCreateForm, AgentCreateDraft>(
      AgentCreateForm.new,
    );
