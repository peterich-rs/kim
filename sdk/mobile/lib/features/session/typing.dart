library;

import 'dart:async';

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
  Timer? _ttl;

  @override
  TypingState build() {
    ref.onDispose(() => _ttl?.cancel());
    return const TypingState();
  }

  void applyPush({
    required String typer,
    required String dest,
    required bool active,
    String me = '',
  }) {
    // Peer typing: thread is the typer. Own typing on another device of a
    // bot 1:1: thread is dest (the bot).
    final thread = (me.isNotEmpty && typer == me) ? dest : typer;
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
    _armTtl();
  }

  void clearDest(String dest) {
    if (!state.activeByDest.containsKey(dest)) {
      return;
    }
    final next = Map<String, bool>.from(state.activeByDest)..remove(dest);
    state = state.copyWith(activeByDest: next);
    _armTtl();
  }

  void clear() {
    _ttl?.cancel();
    _ttl = null;
    state = const TypingState();
  }

  /// Drop stale typing if a peer crashes mid-indicator (humans + bots).
  void _armTtl() {
    _ttl?.cancel();
    if (state.activeByDest.isEmpty) {
      _ttl = null;
      return;
    }
    _ttl = Timer(const Duration(seconds: 20), () {
      if (state.activeByDest.isEmpty) {
        return;
      }
      state = const TypingState();
      _ttl = null;
    });
  }
}

final typingProvider = NotifierProvider<TypingNotifier, TypingState>(
  TypingNotifier.new,
);

final peerTypingProvider = Provider.family<bool, String>((ref, dest) {
  return ref.watch(typingProvider.select((s) => s.isTyping(dest)));
});
