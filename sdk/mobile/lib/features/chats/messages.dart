library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/chats/inbox.dart';
import 'package:kim_mobile/features/session/kim_session.dart';
import 'package:kim_mobile/features/session/providers.dart';

const _unset = Object();

class ThreadMessagesState {
  const ThreadMessagesState({
    required this.items,
    this.loadingOlder = false,
    this.hasMore = true,
    this.unreadAnchorId,
    this.historyError,
  });

  final List<KimChatMsg> items;
  final bool loadingOlder;
  final bool hasMore;
  final String? unreadAnchorId;
  final String? historyError;

  ThreadMessagesState copyWith({
    List<KimChatMsg>? items,
    bool? loadingOlder,
    bool? hasMore,
    String? unreadAnchorId,
    Object? historyError = _unset,
  }) {
    return ThreadMessagesState(
      items: items ?? this.items,
      loadingOlder: loadingOlder ?? this.loadingOlder,
      hasMore: hasMore ?? this.hasMore,
      unreadAnchorId: unreadAnchorId ?? this.unreadAnchorId,
      historyError: identical(historyError, _unset)
          ? this.historyError
          : historyError as String?,
    );
  }
}

class ThreadMessagesNotifier extends Notifier<ThreadMessagesState> {
  ThreadMessagesNotifier(this.dest);

  final String dest;
  StreamSubscription<TimelineUpdate>? _sub;
  var _awaitingSnapshot = false;

  @override
  ThreadMessagesState build() {
    final account = ref.watch(authProvider.select((s) => s.account));
    if (account.isEmpty) {
      unawaited(_sub?.cancel());
      _sub = null;
      return const ThreadMessagesState(items: [], hasMore: false);
    }
    _listen();
    ref.onDispose(() {
      unawaited(_sub?.cancel());
      _sub = null;
    });
    return const ThreadMessagesState(items: []);
  }

  void _listen() {
    KimLogger.info('watchThread dest=$dest');
    unawaited(_sub?.cancel());
    _awaitingSnapshot = false;
    try {
      _sub = ref
          .read(clientPortProvider)
          .watchThread(dest)
          .listen(
            (update) {
              if (!ref.mounted) {
                return;
              }
              switch (update) {
                case TimelineUpdate_Snapshot(:final snapshot):
                  _onSnapshot(snapshot);
                case TimelineUpdate_Delta(:final delta):
                  if (_awaitingSnapshot) {
                    return;
                  }
                  _onDelta(delta);
                case TimelineUpdate_Resync():
                  KimLogger.info('timeline resync dest=$dest');
                  _awaitingSnapshot = true;
                  state = state.copyWith(items: const []);
              }
            },
            onError: (Object e, StackTrace st) {
              KimLogger.warn('watchThread', e, st);
            },
          );
    } catch (e, st) {
      KimLogger.warn('watchThread subscribe', e, st);
    }
  }

  void _onSnapshot(TimelineSnapshot snapshot) {
    _awaitingSnapshot = false;
    final hot = [
      for (final m in snapshot.messages) kimChatFrom(m),
      for (final m in snapshot.pending) kimChatFrom(m),
    ];
    state = state.copyWith(
      items: _sorted(hot),
      loadingOlder: snapshot.loadingOlder,
      hasMore: snapshot.hasMore,
      historyError: snapshot.historyError,
    );
  }

  void _onDelta(TimelineDelta delta) {
    final byKey = {for (final m in state.items) m.key: m};
    for (final key in delta.deletedKeys) {
      byKey.remove(key);
    }
    for (final u in delta.upserts) {
      byKey[u.key] = kimChatFrom(u);
    }
    state = state.copyWith(items: _sorted(byKey.values.toList()));
  }

  List<KimChatMsg> _sorted(List<KimChatMsg> items) {
    items.sort((a, b) {
      final byAt = a.at.compareTo(b.at);
      if (byAt != 0) {
        return byAt;
      }
      return a.key.compareTo(b.key);
    });
    return items;
  }

  void captureUnreadAnchor({required int unread, required String self}) {
    if (unread <= 0 || state.items.isEmpty) {
      return;
    }
    var left = unread;
    String? anchor;
    for (var i = state.items.length - 1; i >= 0; i--) {
      final m = state.items[i];
      if (m.sys || m.sender == self) {
        continue;
      }
      anchor = m.key;
      left -= 1;
      if (left <= 0) {
        break;
      }
    }
    if (anchor != null) {
      state = state.copyWith(unreadAnchorId: anchor);
    }
  }

  Future<void> loadOlder() async {
    if (state.loadingOlder || !state.hasMore) {
      return;
    }
    state = state.copyWith(loadingOlder: true);
    try {
      await ref.read(clientPortProvider).loadOlder(dest: dest);
    } catch (e, st) {
      KimLogger.warn('loadOlder', e, st);
      if (ref.mounted) {
        state = state.copyWith(loadingOlder: false);
      }
    }
  }

  Future<void> markRead() async {
    await markConversationRead();
  }

  Future<void> markConversationRead() async {
    final kind =
        ref.read(threadsProvider).thread(dest)?.kind ?? ThreadKind.user;
    try {
      await ref.read(clientPortProvider).markConversationRead(dest, kind);
    } catch (e, st) {
      KimLogger.warn('markConversationRead', e, st);
    }
  }
}

final threadMessagesProvider = NotifierProvider.autoDispose
    .family<ThreadMessagesNotifier, ThreadMessagesState, String>(
      ThreadMessagesNotifier.new,
    );
