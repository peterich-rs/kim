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

  /// Destinations whose busy bit is driven by local [applyAgentTurn].
  /// Heartbeat `chat.bot.typing` is for other devices; it must not fight
  /// this machine on the owner desktop.
  final _localAgentTurn = <String, bool>{};

  @override
  TypingState build() {
    ref.onDispose(() => _ttl?.cancel());
    return const TypingState();
  }

  /// Owner-desktop Goose turn. Local source of truth: only `running` is
  /// busy. queued / waitingPermission / done / error are not "typing".
  /// The next turn lights again by calling this with [busy] true.
  void applyAgentTurn({
    required String dest,
    required bool busy,
    String me = '',
  }) {
    if (dest.isEmpty) {
      return;
    }
    _localAgentTurn[dest] = busy;
    _write(typer: dest, dest: dest, active: busy, me: me);
  }

  void applyPush({
    required String typer,
    required String dest,
    required bool active,
    String me = '',
  }) {
    // Local AgentTurn already owns this dest. Ignore heartbeat / server echo.
    if (_localAgentTurn.containsKey(typer) ||
        _localAgentTurn.containsKey(dest)) {
      return;
    }
    _write(typer: typer, dest: dest, active: active, me: me);
  }

  void _write({
    required String typer,
    required String dest,
    required bool active,
    required String me,
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
    _localAgentTurn.clear();
    state = const TypingState();
  }

  /// Drop stale human / remote-bot typing if the peer crashes mid-indicator.
  /// Local AgentTurn busy is not TTL'd — [applyAgentTurn] off is the stop.
  void _armTtl() {
    _ttl?.cancel();
    final hasEphemeral = state.activeByDest.keys.any(
      (key) => _localAgentTurn[key] != true,
    );
    if (!hasEphemeral) {
      _ttl = null;
      return;
    }
    _ttl = Timer(const Duration(seconds: 20), () {
      final next = Map<String, bool>.from(state.activeByDest)
        ..removeWhere((key, _) => _localAgentTurn[key] != true);
      state = TypingState(activeByDest: next);
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
