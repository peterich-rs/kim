library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/mention.dart';

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
    // Own composer typing on another device must not look like the peer
    // (or the agent) is typing. Agent busy uses typer = bot account.
    if (me.isNotEmpty && typer == me) {
      return;
    }
    if (typer.isEmpty) {
      return;
    }
    final next = Map<String, bool>.from(state.activeByDest);
    void write(String key, bool on) {
      if (key.isEmpty) {
        return;
      }
      if (on) {
        next[key] = true;
      } else {
        next.remove(key);
      }
    }

    write(typer, active);
    // Goose dest and registered `b_*` are the same 1:1; key both.
    if (dest != typer && (isAgentDest(dest) || isServerBotAccount(dest))) {
      write(dest, active);
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
