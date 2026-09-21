library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/agent/skills_catalog.dart';

class PlazaView {
  const PlazaView({
    this.loaded = false,
    this.app = const [],
    this.eco = const [],
    this.assign,
  });

  final bool loaded;
  final List<CatalogSkill> app;
  final List<CatalogSkill> eco;
  final AgentProfile? assign;

  PlazaView copyWith({
    bool? loaded,
    List<CatalogSkill>? app,
    List<CatalogSkill>? eco,
    AgentProfile? assign,
    bool clearAssign = false,
  }) {
    return PlazaView(
      loaded: loaded ?? this.loaded,
      app: app ?? this.app,
      eco: eco ?? this.eco,
      assign: clearAssign ? assign : (assign ?? this.assign),
    );
  }
}

class PlazaUi extends Notifier<PlazaView> {
  @override
  PlazaView build() => const PlazaView();

  void showLoaded({
    required List<CatalogSkill> app,
    required List<CatalogSkill> eco,
    AgentProfile? assign,
  }) {
    state = PlazaView(loaded: true, app: app, eco: eco, assign: assign);
  }

  void setAssign(AgentProfile profile) =>
      state = state.copyWith(assign: profile);
}

final plazaUiProvider = NotifierProvider.autoDispose<PlazaUi, PlazaView>(
  PlazaUi.new,
);
