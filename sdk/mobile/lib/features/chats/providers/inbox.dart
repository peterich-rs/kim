library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/chats/data/threads_repository.dart';
import 'package:kim_mobile/features/session/kim_session.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/models/models.dart';

export 'package:kim_mobile/features/chats/data/threads_repository.dart'
    show withLocalThreads;

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

final threadsRepositoryProvider = Provider<ThreadsRepository>((ref) {
  return ThreadsRepository(ref.watch(clientPortProvider));
});

class ThreadsNotifier extends Notifier<ThreadsState> {
  var _query = '';

  @override
  ThreadsState build() {
    final dtos = ref.watch(kimSessionProvider.select((s) => s.threads));
    ref.watch(agentProfilesProvider);
    final agents = ref.read(agentProfilesProvider.notifier).visibleAgents;
    final threads = ref
        .read(threadsRepositoryProvider)
        .project(remote: dtos, localAgents: agents);
    return ThreadsState(threads: threads, query: _query);
  }

  void setQuery(String value) {
    _query = value;
    state = state.copyWith(query: value);
  }

  Future<void> markRead(String dest) async {
    final t = state.thread(dest);
    final kind = t?.kind ?? ThreadKind.user;
    await ref.read(threadsRepositoryProvider).markConversationRead(dest, kind);
  }

  Future<void> deleteThread(String id) async {
    await ref.read(threadsRepositoryProvider).deleteThread(id);
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
