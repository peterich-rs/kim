library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';

class PresenceState {
  const PresenceState({this.byAccount = const {}, this.lastSeenMs = const {}});

  final Map<String, PeerPresenceStatus> byAccount;
  final Map<String, int?> lastSeenMs;

  PeerPresenceStatus of(String account) =>
      byAccount[account] ?? PeerPresenceStatus.unknown;

  PresenceState copyWith({
    Map<String, PeerPresenceStatus>? byAccount,
    Map<String, int?>? lastSeenMs,
  }) {
    return PresenceState(
      byAccount: byAccount ?? this.byAccount,
      lastSeenMs: lastSeenMs ?? this.lastSeenMs,
    );
  }
}

class PresenceNotifier extends Notifier<PresenceState> {
  @override
  PresenceState build() => const PresenceState();

  void applySnapshot(List<Map<String, dynamic>> entries) {
    if (entries.isEmpty) {
      return;
    }
    final by = Map<String, PeerPresenceStatus>.from(state.byAccount);
    final seen = Map<String, int?>.from(state.lastSeenMs);
    for (final row in entries) {
      final account = '${row['account'] ?? ''}';
      if (account.isEmpty) {
        continue;
      }
      final status = peerPresenceFromWire(
        row['status'] is int
            ? row['status'] as int
            : int.tryParse('${row['status']}') ?? 0,
      );
      by[account] = status;
      final last = row['lastSeen'] ?? row['last_seen'];
      seen[account] = last is int ? last : int.tryParse('$last');
    }
    state = state.copyWith(byAccount: by, lastSeenMs: seen);
  }

  void applyPush({
    required String account,
    required int status,
    int lastSeen = 0,
  }) {
    if (account.isEmpty) {
      return;
    }
    final by = Map<String, PeerPresenceStatus>.from(state.byAccount);
    final seen = Map<String, int?>.from(state.lastSeenMs);
    by[account] = peerPresenceFromWire(status);
    seen[account] = lastSeen == 0 ? seen[account] : lastSeen;
    state = state.copyWith(byAccount: by, lastSeenMs: seen);
  }

  void clear() {
    state = const PresenceState();
  }
}

final presenceProvider = NotifierProvider<PresenceNotifier, PresenceState>(
  PresenceNotifier.new,
);

/// Convenience: presence for one account (unknown if unset).
final peerPresenceProvider = Provider.family<PeerPresenceStatus, String>((
  ref,
  account,
) {
  return ref.watch(presenceProvider.select((s) => s.of(account)));
});
