library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Per-thread peer typing (ephemeral). Key = peer account / thread dest.
class TypingState {
  const TypingState({this.activeByDest = const {}});

  final Map<String, bool> activeByDest;

  bool isTyping(String dest) => activeByDest[dest] == true;

  TypingState copyWith({Map<String, bool>? activeByDest}) {
    return TypingState(activeByDest: activeByDest ?? this.activeByDest);
  }
}

class TypingNotifier extends Notifier<TypingState> {
  @override
  TypingState build() => const TypingState();

  void applyPush({
    required String typer,
    required String dest,
    required bool active,
  }) {
    // For viewer, the thread id is the typer (DM peer).
    final thread = typer;
    if (thread.isEmpty) {
      return;
    }
    final next = Map<String, bool>.from(state.activeByDest);
    if (active) {
      next[thread] = true;
    } else {
      next.remove(thread);
    }
    state = state.copyWith(activeByDest: next);
  }

  void clearDest(String dest) {
    if (!state.activeByDest.containsKey(dest)) {
      return;
    }
    final next = Map<String, bool>.from(state.activeByDest)..remove(dest);
    state = state.copyWith(activeByDest: next);
  }

  void clear() {
    state = const TypingState();
  }
}

final typingProvider = NotifierProvider<TypingNotifier, TypingState>(
  TypingNotifier.new,
);

final peerTypingProvider = Provider.family<bool, String>((ref, dest) {
  return ref.watch(typingProvider.select((s) => s.isTyping(dest)));
});
