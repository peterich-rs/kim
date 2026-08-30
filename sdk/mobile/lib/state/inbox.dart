library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../copy.dart';
import '../core/format.dart';
import '../core/haptics.dart';
import '../core/validation.dart';
import '../models/models.dart';
import 'contacts.dart';
import 'location.dart';
import 'providers.dart';
import 'session.dart';

class InboxState {
  const InboxState({
    required this.threads,
    required this.messages,
    this.query = '',
  });

  factory InboxState.empty() => const InboxState(threads: [], messages: {});

  final List<KimThread> threads;
  final Map<String, List<KimChatMsg>> messages;
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

  InboxState copyWith({
    List<KimThread>? threads,
    Map<String, List<KimChatMsg>>? messages,
    String? query,
  }) {
    return InboxState(
      threads: threads ?? this.threads,
      messages: messages ?? this.messages,
      query: query ?? this.query,
    );
  }
}

class InboxNotifier extends Notifier<InboxState> {
  static const _uuid = Uuid();

  @override
  InboxState build() {
    final account = ref.watch(sessionProvider.select((s) => s.account));
    if (account.isEmpty) {
      return InboxState.empty();
    }
    final store = ref.watch(conversationStoreProvider);
    return InboxState(threads: store.loadThreads(account), messages: {});
  }

  void setQuery(String value) {
    state = state.copyWith(query: value);
  }

  void receive(KimEvent event) {
    final dest = event.dest.isNotEmpty ? event.dest : event.sender;
    if (dest.isEmpty || event.body.isEmpty) {
      return;
    }
    final account = ref.read(sessionProvider).account;
    final existing = _thread(dest);
    if (existing == null) {
      _upsert(
        KimThread(
          id: dest,
          kind: ThreadKind.user,
          title: ref.read(contactsProvider).person(dest)?.title ?? dest,
        ),
      );
    }
    final msg = KimChatMsg(
      key: event.messageId == 0 ? _uuid.v4() : '${event.messageId}',
      dest: dest,
      sender: event.sender.isEmpty ? dest : event.sender,
      body: event.body,
      at: sendTimeMs(event.sendTime),
    );
    _append(msg, fromSelf: event.sender == account);
    unawaited(_persistMessages(dest));
    _persistThreads();
  }

  List<KimChatMsg> messagesFor(String dest) {
    final cached = state.messages[dest];
    if (cached != null) {
      return cached;
    }
    final account = ref.read(sessionProvider).account;
    final loaded = ref
        .read(conversationStoreProvider)
        .loadMessages(account, dest);
    state = state.copyWith(messages: {...state.messages, dest: loaded});
    return loaded;
  }

  KimThread ensureThread({
    required String id,
    ThreadKind kind = ThreadKind.user,
    String? title,
  }) {
    final existing = _thread(id);
    if (existing != null) {
      if (existing.unread == 0) {
        return existing;
      }
      final next = _upsert(existing.copyWith(unread: 0));
      _persistThreads();
      return next;
    }
    final created = KimThread(
      id: id,
      kind: kind,
      title: (title == null || title.isEmpty) ? id : title,
    );
    _upsert(created);
    _persistThreads();
    return created;
  }

  Future<void> deleteThread(String id) async {
    final account = ref.read(sessionProvider).account;
    await ref.read(conversationStoreProvider).deleteThread(account, id);
    final nextMsgs = Map<String, List<KimChatMsg>>.from(state.messages)
      ..remove(id);
    state = state.copyWith(
      threads: state.threads.where((t) => t.id != id).toList(),
      messages: nextMsgs,
    );
  }

  Future<KimChatMsg> send(String dest, String text) async {
    final body = text.trim();
    if (body.isEmpty) {
      throw StateError(Copy.required);
    }
    final accountErr = validateAccount(dest);
    if (accountErr != null) {
      throw StateError(accountErr);
    }
    final session = ref.read(sessionProvider);
    if (dest == session.account) {
      throw StateError(Copy.cannotChatSelf);
    }
    if (session.status != ConnStatus.online) {
      throw StateError(Copy.notConnected);
    }
    final existing = _thread(dest);
    final social = ref.read(contactsProvider);
    if (existing?.kind != ThreadKind.group &&
        social.ready &&
        !social.isFriend(dest)) {
      throw StateError(Copy.notFriends);
    }
    ensureThread(id: dest);
    final msg = KimChatMsg(
      key: _uuid.v4(),
      dest: dest,
      sender: session.account,
      body: body,
      at: DateTime.now().millisecondsSinceEpoch,
    );
    _append(msg, fromSelf: true);
    await _persistMessages(dest);
    _persistThreads();
    try {
      await ref.read(clientPortProvider).talk(dest, body);
      await KimHaptics.light();
      return msg;
    } catch (_) {
      _markFailed(dest, msg.key);
      await _persistMessages(dest);
      await KimHaptics.error();
      rethrow;
    }
  }

  KimThread _upsert(KimThread thread) {
    final rest = state.threads.where((t) => t.id != thread.id).toList();
    final next = [thread, ...rest]
      ..sort((a, b) => b.lastAt.compareTo(a.lastAt));
    state = state.copyWith(threads: next);
    return thread;
  }

  KimThread? _thread(String id) {
    for (final t in state.threads) {
      if (t.id == id) {
        return t;
      }
    }
    return null;
  }

  List<KimChatMsg> _loaded(String dest) {
    final cached = state.messages[dest];
    if (cached != null) {
      return cached;
    }
    final account = ref.read(sessionProvider).account;
    return ref.read(conversationStoreProvider).loadMessages(account, dest);
  }

  void _append(KimChatMsg msg, {required bool fromSelf}) {
    final prev = List<KimChatMsg>.from(_loaded(msg.dest));
    if (prev.any((m) => m.key == msg.key)) {
      return;
    }
    prev.add(msg);
    final clipped = prev.length > 400 ? prev.sublist(prev.length - 400) : prev;
    final existing = _thread(msg.dest);
    final viewing = chatIdFromPath(ref.read(locationProvider));
    final unread = fromSelf || msg.sys || viewing == msg.dest
        ? (existing?.unread ?? 0)
        : (existing?.unread ?? 0) + 1;
    _upsert(
      KimThread(
        id: msg.dest,
        kind: existing?.kind ?? ThreadKind.user,
        title: existing?.title ?? msg.dest,
        lastBody: msg.sys ? (existing?.lastBody ?? '') : truncate(msg.body),
        lastAt: msg.at,
        unread: unread,
      ),
    );
    state = state.copyWith(messages: {...state.messages, msg.dest: clipped});
  }

  void _markFailed(String dest, String key) {
    final prev = state.messages[dest];
    if (prev == null) {
      return;
    }
    state = state.copyWith(
      messages: {
        ...state.messages,
        dest: [
          for (final m in prev)
            if (m.key == key) m.copyWith(failed: true) else m,
        ],
      },
    );
  }

  void _persistThreads() {
    final account = ref.read(sessionProvider).account;
    unawaited(
      ref.read(conversationStoreProvider).saveThreads(account, state.threads),
    );
  }

  Future<void> _persistMessages(String dest) async {
    final account = ref.read(sessionProvider).account;
    await ref
        .read(conversationStoreProvider)
        .saveMessages(account, dest, state.messages[dest] ?? const []);
  }
}

final inboxProvider = NotifierProvider<InboxNotifier, InboxState>(
  InboxNotifier.new,
);
