library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Peer read watermark for DM threads. Key = peer account.
class ReceiptsState {
  const ReceiptsState({this.readUpToByDest = const {}});

  final Map<String, int> readUpToByDest;

  int? of(String dest) => readUpToByDest[dest];

  ReceiptsState copyWith({Map<String, int>? readUpToByDest}) {
    return ReceiptsState(readUpToByDest: readUpToByDest ?? this.readUpToByDest);
  }
}

class ReceiptsNotifier extends Notifier<ReceiptsState> {
  @override
  ReceiptsState build() => const ReceiptsState();

  void applyPush({
    required String reader,
    required String dest,
    required int kind,
    required int messageId,
  }) {
    if (kind != 0 || messageId <= 0) {
      return; // DM only
    }
    // Viewer is the sender; thread id is the reader (peer who read).
    final thread = reader;
    if (thread.isEmpty) {
      return;
    }
    final prev = state.readUpToByDest[thread] ?? 0;
    if (messageId < prev) {
      return;
    }
    final next = Map<String, int>.from(state.readUpToByDest);
    next[thread] = messageId;
    state = state.copyWith(readUpToByDest: next);
  }

  void clear() {
    state = const ReceiptsState();
  }
}

final receiptsProvider = NotifierProvider<ReceiptsNotifier, ReceiptsState>(
  ReceiptsNotifier.new,
);

final peerReadUpToProvider = Provider.family<int?, String>((ref, dest) {
  return ref.watch(receiptsProvider.select((s) => s.of(dest)));
});
