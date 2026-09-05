library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/format.dart';
import '../core/image_extra.dart';
import '../models/models.dart';
import 'contacts.dart';
import 'location.dart';
import 'providers.dart';
import 'auth.dart';

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

class ThreadsNotifier extends Notifier<ThreadsState> {
  @override
  ThreadsState build() {
    final account = ref.watch(authProvider.select((s) => s.account));
    if (account.isEmpty) {
      return ThreadsState.empty();
    }
    final store = ref.watch(conversationStoreProvider);
    return ThreadsState(threads: store.loadThreads(account));
  }

  void setQuery(String value) {
    state = state.copyWith(query: value);
  }

  void mergeInbox(List<KimThread> incoming) {
    if (incoming.isEmpty) {
      return;
    }
    final viewing = chatIdFromPath(ref.read(locationProvider));
    final byId = {for (final t in state.threads) t.id: t};
    for (final t in incoming) {
      final prev = byId[t.id];
      byId[t.id] = KimThread(
        id: t.id,
        kind: t.kind,
        title: t.title.isEmpty ? (prev?.title ?? t.id) : t.title,
        lastBody: t.lastBody.isEmpty ? (prev?.lastBody ?? '') : t.lastBody,
        lastAt: t.lastAt == 0 ? (prev?.lastAt ?? 0) : t.lastAt,
        unread: _mergedUnread(prev, t, viewing),
        avatar: t.avatar.isEmpty ? (prev?.avatar ?? '') : t.avatar,
      );
    }
    final next = byId.values.toList()
      ..sort((a, b) => b.lastAt.compareTo(a.lastAt));
    state = state.copyWith(threads: next);
    _persist();
  }

  void applyTalk(KimChatMsg msg, {required bool fromSelf}) {
    final existing = state.thread(msg.dest);
    final viewing = chatIdFromPath(ref.read(locationProvider));
    final unread = viewing == msg.dest
        ? 0
        : fromSelf || msg.sys
        ? (existing?.unread ?? 0)
        : (existing?.unread ?? 0) + 1;
    _upsert(
      KimThread(
        id: msg.dest,
        kind: existing?.kind ?? ThreadKind.user,
        title:
            existing?.title ??
            (ref.read(contactsProvider).person(msg.dest)?.title ?? msg.dest),
        lastBody: msg.sys ? (existing?.lastBody ?? '') : previewBody(msg),
        lastAt: msg.at,
        unread: unread,
        avatar: existing?.avatar ?? '',
      ),
    );
  }

  void markRead(String id) {
    final existing = state.thread(id);
    if (existing == null || existing.unread == 0) {
      return;
    }
    _upsert(existing.copyWith(unread: 0));
    _persist();
    unawaited(
      ref
          .read(conversationStoreProvider)
          .markThreadRead(ref.read(authProvider).account, id),
    );
  }

  KimThread ensureThread({
    required String id,
    ThreadKind kind = ThreadKind.user,
    String? title,
  }) {
    final existing = state.thread(id);
    if (existing != null) {
      if (existing.unread == 0) {
        return existing;
      }
      final next = _upsert(existing.copyWith(unread: 0));
      _persist();
      unawaited(
        ref
            .read(conversationStoreProvider)
            .markThreadRead(ref.read(authProvider).account, id),
      );
      return next;
    }
    final created = KimThread(
      id: id,
      kind: kind,
      title: (title == null || title.isEmpty) ? id : title,
    );
    _upsert(created);
    _persist();
    return created;
  }

  Future<void> deleteThread(String id) async {
    final account = ref.read(authProvider).account;
    await ref.read(conversationStoreProvider).deleteThread(account, id);
    state = state.copyWith(
      threads: state.threads.where((t) => t.id != id).toList(),
    );
  }

  int _mergedUnread(KimThread? prev, KimThread incoming, String? viewing) {
    if (viewing == incoming.id) {
      return 0;
    }
    if (prev != null &&
        prev.unread == 0 &&
        sendTimeMs(prev.lastAt) >= sendTimeMs(incoming.lastAt)) {
      return 0;
    }
    return incoming.unread;
  }

  KimThread _upsert(KimThread thread) {
    final rest = state.threads.where((t) => t.id != thread.id).toList();
    final next = [thread, ...rest]
      ..sort((a, b) => b.lastAt.compareTo(a.lastAt));
    state = state.copyWith(threads: next);
    return thread;
  }

  void _persist() {
    final account = ref.read(authProvider).account;
    unawaited(() async {
      final store = ref.read(conversationStoreProvider);
      for (final t in state.threads) {
        await store.upsertThread(account, t);
      }
    }());
  }
}

final threadsProvider = NotifierProvider<ThreadsNotifier, ThreadsState>(
  ThreadsNotifier.new,
);

/// Alias kept so call sites / tests can migrate off [inboxProvider].
final inboxProvider = threadsProvider;
