library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/src/rust/api/types.dart';

/// Rust settings-table projection (ws_url / origin / env). Revision-bumped
/// after a preset switch by `DevPanelUi.bump` consumers.
final envSettingsProvider = FutureProvider.autoDispose<Settings>((ref) async {
  final client = ref.watch(clientPortProvider);
  return client.settingsGet();
});

class DevPanelView {
  const DevPanelView({this.metrics, this.wipeSeen = 0, this.revision = 0});

  final Metrics? metrics;
  final int wipeSeen;
  final int revision;

  DevPanelView copyWith({Metrics? metrics, int? wipeSeen, int? revision}) {
    return DevPanelView(
      metrics: metrics ?? this.metrics,
      wipeSeen: wipeSeen ?? this.wipeSeen,
      revision: revision ?? this.revision,
    );
  }
}

class DevPanelUi extends Notifier<DevPanelView> {
  @override
  DevPanelView build() => const DevPanelView();

  void showMetrics({required Metrics metrics, required int wipeSeen}) {
    state = state.copyWith(
      metrics: metrics,
      wipeSeen: wipeSeen,
      revision: state.revision + 1,
    );
  }

  void ackWipe(int total) => state = state.copyWith(wipeSeen: total);

  void bump() {
    state = state.copyWith(revision: state.revision + 1);
    ref.invalidate(envSettingsProvider);
  }
}

final devPanelProvider = NotifierProvider.autoDispose<DevPanelUi, DevPanelView>(
  DevPanelUi.new,
);
