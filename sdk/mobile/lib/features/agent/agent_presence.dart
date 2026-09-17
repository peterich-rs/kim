library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/pet_pack.dart';
import 'package:kim_mobile/features/session/typing.dart';

/// Conservative window so one-shot clips are not cut short of pack duration.
const kPetPresenceWindow = Duration(milliseconds: 1200);

abstract class AgentRunSink {
  void begin(String dest);
  void finish(String dest, {required bool failed});
}

/// Drops through after [detach] so a disposed notifier cannot throw.
class DetachableAgentRunSink implements AgentRunSink {
  DetachableAgentRunSink(this._inner);

  AgentRunSink? _inner;

  void detach() => _inner = null;

  @override
  void begin(String dest) => _inner?.begin(dest);

  @override
  void finish(String dest, {required bool failed}) =>
      _inner?.finish(dest, failed: failed);
}

class AgentPresence {
  const AgentPresence({required this.phase, this.packId = 'default'});

  final PetPhase phase;
  final String packId;

  @override
  bool operator ==(Object other) =>
      other is AgentPresence && other.phase == phase && other.packId == packId;

  @override
  int get hashCode => Object.hash(phase, packId);
}

class AgentRunStatus {
  const AgentRunStatus({
    this.running = const <String>{},
    this.finishedAt = const <String, DateTime>{},
    this.failedAt = const <String, DateTime>{},
    this.openedAt = const <String, DateTime>{},
  });

  final Set<String> running;
  final Map<String, DateTime> finishedAt;
  final Map<String, DateTime> failedAt;
  final Map<String, DateTime> openedAt;

  AgentRunStatus copyWith({
    Set<String>? running,
    Map<String, DateTime>? finishedAt,
    Map<String, DateTime>? failedAt,
    Map<String, DateTime>? openedAt,
  }) {
    return AgentRunStatus(
      running: running ?? this.running,
      finishedAt: finishedAt ?? this.finishedAt,
      failedAt: failedAt ?? this.failedAt,
      openedAt: openedAt ?? this.openedAt,
    );
  }
}

class AgentRunStatusNotifier extends Notifier<AgentRunStatus>
    implements AgentRunSink {
  final Set<String> _opened = <String>{};

  @override
  AgentRunStatus build() => const AgentRunStatus();

  @override
  void begin(String dest) {
    if (dest.isEmpty) {
      return;
    }
    final running = Set<String>.from(state.running)..add(dest);
    final finishedAt = Map<String, DateTime>.from(state.finishedAt)
      ..remove(dest);
    final failedAt = Map<String, DateTime>.from(state.failedAt)..remove(dest);
    state = state.copyWith(
      running: running,
      finishedAt: finishedAt,
      failedAt: failedAt,
    );
  }

  @override
  void finish(String dest, {required bool failed}) {
    if (dest.isEmpty) {
      return;
    }
    final now = DateTime.now();
    final running = Set<String>.from(state.running)..remove(dest);
    if (failed) {
      final failedAt = Map<String, DateTime>.from(state.failedAt)..[dest] = now;
      final finishedAt = Map<String, DateTime>.from(state.finishedAt)
        ..remove(dest);
      state = state.copyWith(
        running: running,
        failedAt: failedAt,
        finishedAt: finishedAt,
      );
    } else {
      final finishedAt = Map<String, DateTime>.from(state.finishedAt)
        ..[dest] = now;
      final failedAt = Map<String, DateTime>.from(state.failedAt)..remove(dest);
      state = state.copyWith(
        running: running,
        finishedAt: finishedAt,
        failedAt: failedAt,
      );
    }
  }

  void markOpened(String dest) {
    if (dest.isEmpty || _opened.contains(dest)) {
      return;
    }
    _opened.add(dest);
    final openedAt = Map<String, DateTime>.from(state.openedAt)
      ..[dest] = DateTime.now();
    state = state.copyWith(openedAt: openedAt);
  }

  void reviewPulse(String dest) {
    if (dest.isEmpty || state.running.contains(dest)) {
      return;
    }
    final finishedAt = Map<String, DateTime>.from(state.finishedAt)
      ..[dest] = DateTime.now();
    state = state.copyWith(finishedAt: finishedAt);
  }

  void forget(String dest) {
    _opened.remove(dest);
    final running = Set<String>.from(state.running)..remove(dest);
    final finishedAt = Map<String, DateTime>.from(state.finishedAt)
      ..remove(dest);
    final failedAt = Map<String, DateTime>.from(state.failedAt)..remove(dest);
    final openedAt = Map<String, DateTime>.from(state.openedAt)..remove(dest);
    state = state.copyWith(
      running: running,
      finishedAt: finishedAt,
      failedAt: failedAt,
      openedAt: openedAt,
    );
  }

  /// Drop one-shot timestamps so a remount cannot replay wave/review/failed.
  void consumeOneShot(String dest) {
    if (dest.isEmpty) {
      return;
    }
    if (!state.finishedAt.containsKey(dest) &&
        !state.failedAt.containsKey(dest) &&
        !state.openedAt.containsKey(dest)) {
      return;
    }
    final finishedAt = Map<String, DateTime>.from(state.finishedAt)
      ..remove(dest);
    final failedAt = Map<String, DateTime>.from(state.failedAt)..remove(dest);
    final openedAt = Map<String, DateTime>.from(state.openedAt)..remove(dest);
    state = state.copyWith(
      finishedAt: finishedAt,
      failedAt: failedAt,
      openedAt: openedAt,
    );
  }
}

final agentRunStatusProvider =
    NotifierProvider<AgentRunStatusNotifier, AgentRunStatus>(
      AgentRunStatusNotifier.new,
    );

AgentPresence presenceFor({
  required String dest,
  required AgentRunStatus status,
  required bool typing,
  DateTime? now,
}) {
  final at = now ?? DateTime.now();
  bool inWindow(DateTime? t) =>
      t != null && at.difference(t) < kPetPresenceWindow;
  if (inWindow(status.failedAt[dest])) {
    return const AgentPresence(phase: PetPhase.failed);
  }
  if (inWindow(status.finishedAt[dest])) {
    return const AgentPresence(phase: PetPhase.review);
  }
  if (inWindow(status.openedAt[dest])) {
    return const AgentPresence(phase: PetPhase.wave);
  }
  if (typing || status.running.contains(dest)) {
    return const AgentPresence(phase: PetPhase.running);
  }
  return const AgentPresence(phase: PetPhase.idle);
}

final agentPresenceProvider = Provider.autoDispose
    .family<AgentPresence, String>((ref, dest) {
      final status = ref.watch(agentRunStatusProvider);
      final typing = ref.watch(peerTypingProvider(dest));
      return presenceFor(dest: dest, status: status, typing: typing);
    });
