library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/catalog.dart';
import 'package:kim_mobile/features/agent/context_window.dart';

class AgentSettingsDraft {
  const AgentSettingsDraft({
    this.choice = const ReasoningChoice(kind: 'none'),
    this.surface = const ReasoningSurface(kind: 'none'),
    this.contextTokens = kDefaultContextTokens,
    this.loaded = false,
    this.accountId = '',
    this.vendors = const [],
    this.pendingModels = const [],
    this.profile,
    this.revision = 0,
  });

  final ReasoningChoice choice;
  final ReasoningSurface surface;
  final int contextTokens;
  final bool loaded;
  final String accountId;
  final List<VendorSummary> vendors;
  final List<String> pendingModels;
  final AgentProfile? profile;
  final int revision;

  AgentSettingsDraft copyWith({
    ReasoningChoice? choice,
    ReasoningSurface? surface,
    int? contextTokens,
    bool? loaded,
    String? accountId,
    List<VendorSummary>? vendors,
    List<String>? pendingModels,
    AgentProfile? profile,
    int? revision,
  }) {
    return AgentSettingsDraft(
      choice: choice ?? this.choice,
      surface: surface ?? this.surface,
      contextTokens: contextTokens ?? this.contextTokens,
      loaded: loaded ?? this.loaded,
      accountId: accountId ?? this.accountId,
      vendors: vendors ?? this.vendors,
      pendingModels: pendingModels ?? this.pendingModels,
      profile: profile ?? this.profile,
      revision: revision ?? this.revision,
    );
  }
}

class AgentSettingsForm extends Notifier<AgentSettingsDraft> {
  @override
  AgentSettingsDraft build() => const AgentSettingsDraft();

  void bump() => state = state.copyWith(revision: state.revision + 1);

  void setVendors(List<VendorSummary> vendors) =>
      state = state.copyWith(vendors: vendors);

  void markLoaded() => state = state.copyWith(loaded: true);

  void seedCreate({required String accountId, required int contextTokens}) {
    state = state.copyWith(
      loaded: true,
      accountId: accountId,
      contextTokens: contextTokens,
    );
  }

  void hydrateEditor({
    required String accountId,
    required int contextTokens,
    required ReasoningChoice choice,
  }) {
    state = state.copyWith(
      loaded: true,
      accountId: accountId,
      contextTokens: contextTokens,
      choice: choice,
    );
  }

  void setSurface({
    required ReasoningSurface surface,
    required ReasoningChoice choice,
  }) {
    state = state.copyWith(surface: surface, choice: choice);
  }

  void bindAccount({required String id, required int contextTokens}) {
    state = state.copyWith(
      accountId: id,
      pendingModels: const [],
      contextTokens: contextTokens,
    );
  }

  void setContextTokens(int value) =>
      state = state.copyWith(contextTokens: value);

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

  void setProfile(AgentProfile profile) =>
      state = state.copyWith(profile: profile);
}

final agentSettingsFormProvider =
    NotifierProvider.autoDispose<AgentSettingsForm, AgentSettingsDraft>(
      AgentSettingsForm.new,
    );
