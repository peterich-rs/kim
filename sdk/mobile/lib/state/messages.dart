library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/conversation_store.dart';
import '../models/models.dart';
import 'auth.dart';
import 'inbox.dart';
import 'providers.dart';

class ThreadMessagesState {
  const ThreadMessagesState({
    required this.items,
    this.loadingOlder = false,
    this.hasMore = true,
    this.unreadAnchorId,
  });

  final List<KimChatMsg> items;
  final bool loadingOlder;
  final bool hasMore;
  final String? unreadAnchorId;

  ThreadMessagesState copyWith({
    List<KimChatMsg>? items,
    bool? loadingOlder,
    bool? hasMore,
    String? unreadAnchorId,
  }) {
    return ThreadMessagesState(
      items: items ?? this.items,
      loadingOlder: loadingOlder ?? this.loadingOlder,
      hasMore: hasMore ?? this.hasMore,
      unreadAnchorId: unreadAnchorId ?? this.unreadAnchorId,
    );
  }
}

class ThreadMessagesNotifier extends Notifier<ThreadMessagesState> {
  ThreadMessagesNotifier(this.dest);

  final String dest;
  var _reconciled = false;

  @override
  ThreadMessagesState build() {
    final account = ref.watch(authProvider.select((s) => s.account));
    if (account.isEmpty) {
      return const ThreadMessagesState(items: [], hasMore: false);
    }
    final store = ref.read(conversationStoreProvider);
    final page = store.loadMessagesPage(account, dest, limit: 50);
    if (store.isolateBacked) {
      unawaited(_hydrate(account, store));
    }
    return ThreadMessagesState(items: page.reversed.toList(), hasMore: true);
  }

  Future<void> _hydrate(String account, ConversationStore store) async {
    await store.ensureMessages(account, dest);
    if (!ref.mounted) {
      return;
    }
    final page = store.loadMessagesPage(account, dest, limit: 50);
    if (page.isEmpty) {
      return;
    }
    receiveAll(page.reversed);
  }

  void receive(KimChatMsg msg) {
    receiveAll([msg]);
  }

  void receiveAll(Iterable<KimChatMsg> msgs) {
    final incoming = [
      for (final m in msgs)
        if (m.dest == dest) m,
    ];
    if (incoming.isEmpty) {
      return;
    }
    var prev = List<KimChatMsg>.from(state.items);
    for (final msg in incoming) {
      final idx = _indexOf(prev, msg);
      if (idx >= 0) {
        prev[idx] = _merge(prev[idx], msg);
      } else {
        prev.add(msg);
      }
    }
    prev.sort((a, b) {
      final byAt = a.at.compareTo(b.at);
      if (byAt != 0) {
        return byAt;
      }
      return a.key.compareTo(b.key);
    });
    if (prev.length > ConversationStore.maxMessages) {
      prev = prev.sublist(prev.length - ConversationStore.maxMessages);
    }
    state = state.copyWith(items: prev);
  }

  void patch(String key, KimChatMsg Function(KimChatMsg) update) {
    final prev = [
      for (final m in state.items)
        if (m.key == key) update(m) else m,
    ];
    state = state.copyWith(items: prev);
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
    final account = ref.read(authProvider).account;
    if (account.isEmpty || state.items.isEmpty) {
      return;
    }
    state = state.copyWith(loadingOlder: true);
    final oldest = state.items.first;
    try {
      final store = ref.read(conversationStoreProvider);
      final local = store.loadMessagesPage(
        account,
        dest,
        beforeAt: oldest.at,
        beforeKey: oldest.key,
        limit: 50,
      );
      var incoming = local.reversed.toList();
      var remoteLen = 0;
      final beforeId = _historyBeforeId(state.items);
      final localHit = local.length >= 50;
      if (!localHit && beforeId != 0) {
        try {
          final remote = await ref
              .read(clientPortProvider)
              .history(dest, ThreadKind.user, beforeId: beforeId, limit: 50);
          remoteLen = remote.length;
          final repo = ref.read(messageRepositoryProvider);
          incoming = [
            ...incoming,
            for (final row in remote)
              repo.fromHistory(row, dest: dest, account: account),
          ];
        } catch (_) {}
      }
      if (!ref.mounted) {
        return;
      }
      final results = await ref
          .read(messageRepositoryProvider)
          .applySync(account, incoming);
      if (!ref.mounted) {
        return;
      }
      receiveAll([for (final r in results) r.message]);
      state = state.copyWith(
        loadingOlder: false,
        hasMore: local.length >= 50 || remoteLen >= 50,
      );
    } catch (_) {
      if (ref.mounted) {
        state = state.copyWith(loadingOlder: false);
      }
    }
  }

  Future<void> reconcile() async {
    final account = ref.read(authProvider).account;
    if (account.isEmpty) {
      return;
    }
    try {
      final remote = await ref
          .read(clientPortProvider)
          .history(dest, ThreadKind.user, beforeId: 0, limit: 50);
      if (!ref.mounted) {
        return;
      }
      final repo = ref.read(messageRepositoryProvider);
      final msgs = [
        for (final row in remote)
          repo.fromHistory(row, dest: dest, account: account),
      ];
      final results = await repo.applySync(account, msgs);
      if (!ref.mounted) {
        return;
      }
      receiveAll([for (final r in results) r.message]);
      _reconciled = true;
      state = state.copyWith(hasMore: remote.length >= 50);
    } catch (_) {
      if (ref.mounted && !_reconciled) {
        state = state.copyWith(hasMore: true);
      }
    }
  }

  Future<void> markRead() async {
    final account = ref.read(authProvider).account;
    if (account.isEmpty) {
      return;
    }
    ref.read(threadsProvider.notifier).markRead(dest);
    await ref.read(conversationStoreProvider).markThreadRead(account, dest);
    var messageId = 0;
    for (final m in state.items.reversed) {
      if (m.messageId != 0) {
        messageId = m.messageId;
        break;
      }
    }
    final kind =
        ref.read(threadsProvider).thread(dest)?.kind ?? ThreadKind.user;
    try {
      await ref.read(clientPortProvider).markRead(dest, kind, messageId);
    } catch (_) {}
  }

  int _historyBeforeId(List<KimChatMsg> items) {
    for (final m in items) {
      if (m.messageId != 0) {
        return m.messageId;
      }
    }
    return 0;
  }

  int _indexOf(List<KimChatMsg> rows, KimChatMsg msg) {
    for (var i = 0; i < rows.length; i++) {
      final row = rows[i];
      if (row.key == msg.key) {
        return i;
      }
      if (msg.messageId != 0 && row.messageId == msg.messageId) {
        return i;
      }
    }
    return -1;
  }

  KimChatMsg _merge(KimChatMsg prev, KimChatMsg next) {
    return prev.copyWith(
      body: next.body,
      failed: next.failed,
      kind: next.kind,
      width: next.width,
      height: next.height,
      messageId: next.messageId == 0 ? prev.messageId : next.messageId,
      status: next.status,
      at: next.at == 0 ? prev.at : next.at,
      localPath: next.localPath,
    );
  }
}

final threadMessagesProvider =
    NotifierProvider.family<
      ThreadMessagesNotifier,
      ThreadMessagesState,
      String
    >(ThreadMessagesNotifier.new);
