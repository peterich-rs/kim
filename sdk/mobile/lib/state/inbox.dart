library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../agent/host_support.dart';
import '../agent/mention.dart';
import '../models/models.dart';
import 'agent_profiles.dart';
import 'kim_session.dart';
import 'providers.dart';

class ThreadsState {
  const ThreadsState({required this.threads, this.query = ''});

  factory ThreadsState.empty() => const ThreadsState(threads: []);

  final List<KimThread> threads;
  final String query;

  List<KimThread> get visible {
    final q = query.trim().toLowerCase();
    if (q.isEmpty) {
      return threads;
    }
    return threads
        .where(
          (t) =>
              t.title.toLowerCase().contains(q) ||
              t.id.toLowerCase().contains(q),
        )
        .toList();
  }

  KimThread? thread(String id) {
    for (final t in threads) {
      if (t.id == id) {
        return t;
      }
    }
    return null;
  }

  ThreadsState copyWith({List<KimThread>? threads, String? query}) {
    return ThreadsState(
      threads: threads ?? this.threads,
      query: query ?? this.query,
    );
  }
}

List<KimThread> withLocalThreads(
  List<KimThread> threads,
  List<AgentProfile> enabled,
) {
  final existing = {for (final t in threads) t.id};
  final extras = <KimThread>[];
  for (final profile in enabled) {
    final id = personForProfile(profile).account;
    if (existing.contains(id)) {
      continue;
    }
    extras.add(
      KimThread(id: id, kind: ThreadKind.user, title: profile.displayName),
    );
    existing.add(id);
  }
  if (extras.isEmpty) {
    return threads;
  }
  return [...extras, ...threads];
}

class ThreadsNotifier extends Notifier<ThreadsState> {
  var _query = '';

  @override
  ThreadsState build() {
    final dtos = ref.watch(kimSessionProvider.select((s) => s.threads));
    ref.watch(agentProfilesProvider);
    var threads = [for (final t in dtos) kimThreadFromDto(t)];
    if (agentHostSupported) {
      final agents = ref.read(agentProfilesProvider.notifier).visibleAgents;
      threads = withLocalThreads(threads, agents);
    }
    return ThreadsState(threads: threads, query: _query);
  }

  void setQuery(String value) {
    _query = value;
    state = state.copyWith(query: value);
  }

  Future<void> markRead(String dest) async {
    final t = state.thread(dest);
    final kind = t?.kind ?? ThreadKind.user;
    await ref.read(clientPortProvider).markRead(dest, kind, 0);
  }

  Future<void> deleteThread(String id) async {
    await ref.read(clientPortProvider).deleteThread(id);
  }

  KimThread ensureThread({
    required String id,
    ThreadKind kind = ThreadKind.user,
    String? title,
  }) {
    return state.thread(id) ??
        KimThread(id: id, kind: kind, title: title ?? id);
  }
}

final threadsProvider = NotifierProvider<ThreadsNotifier, ThreadsState>(
  ThreadsNotifier.new,
);
