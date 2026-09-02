library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/format.dart';
import '../core/image_extra.dart';
import '../models/models.dart';
import 'providers.dart';
import 'auth.dart';

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

  @override
  ThreadMessagesState build() {
    final account = ref.watch(authProvider.select((s) => s.account));
    if (account.isEmpty) {
      return const ThreadMessagesState(items: [], hasMore: false);
    }
    final page = ref
        .read(conversationStoreProvider)
        .loadMessagesPage(account, dest, limit: 50);
    final items = page.reversed.toList();
    return ThreadMessagesState(items: items, hasMore: page.length >= 50);
  }

  void receive(KimChatMsg msg) {
    if (msg.dest != dest) {
      return;
    }
    final prev = List<KimChatMsg>.from(state.items);
    final idx = _indexOf(prev, msg);
    if (idx >= 0) {
      prev[idx] = _merge(prev[idx], msg);
      state = state.copyWith(items: prev);
      return;
    }
    prev.add(msg);
    prev.sort((a, b) {
      final byAt = a.at.compareTo(b.at);
      if (byAt != 0) {
        return byAt;
      }
      return a.key.compareTo(b.key);
    });
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
      final local = ref
          .read(conversationStoreProvider)
          .loadMessagesPage(account, dest, beforeAt: oldest.at, limit: 50);
      var incoming = local.reversed.toList();
      try {
        final remote = await ref
            .read(clientPortProvider)
            .history(
              dest,
              ThreadKind.user,
              beforeId: oldest.messageId,
              limit: 50,
            );
        incoming = [
          ...incoming,
          for (final row in remote) _fromHistory(row, account),
        ];
      } catch (_) {}
      if (!ref.mounted) {
        return;
      }
      await ref
          .read(conversationStoreProvider)
          .upsertMessages(account, dest, incoming);
      if (!ref.mounted) {
        return;
      }
      for (final msg in incoming) {
        receive(msg);
      }
      state = state.copyWith(loadingOlder: false, hasMore: local.length >= 50);
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
      final msgs = [for (final row in remote) _fromHistory(row, account)];
      await ref
          .read(conversationStoreProvider)
          .upsertMessages(account, dest, msgs);
      if (!ref.mounted) {
        return;
      }
      for (final msg in msgs) {
        receive(msg);
      }
    } catch (_) {}
  }

  Future<void> markRead() async {
    final account = ref.read(authProvider).account;
    await ref.read(conversationStoreProvider).markThreadRead(account, dest);
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
    );
  }

  KimChatMsg _fromHistory(KimHistoryMsg row, String account) {
    final extra = parseImageExtra(row.extra);
    return KimChatMsg(
      key: row.messageId == 0
          ? 'hist-${row.sendTime}-${row.sender}'
          : '${row.messageId}',
      dest: dest,
      sender: row.sender.isEmpty
          ? (row.direction == 1 ? account : dest)
          : row.sender,
      body: row.body,
      at: sendTimeMs(row.sendTime),
      kind: kindFromWire(body: row.body, extra: row.extra, type: row.msgType),
      width: extra?.width ?? 0,
      height: extra?.height ?? 0,
      messageId: row.messageId,
    );
  }
}

final threadMessagesProvider =
    NotifierProvider.family<
      ThreadMessagesNotifier,
      ThreadMessagesState,
      String
    >(ThreadMessagesNotifier.new);
