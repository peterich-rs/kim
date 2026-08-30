library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../copy.dart';
import '../core/haptics.dart';
import '../models/models.dart';
import 'providers.dart';
import 'session.dart';

class ContactsState {
  const ContactsState({
    required this.friends,
    required this.incoming,
    required this.outgoing,
    required this.hits,
    this.ready = false,
    this.loading = false,
    this.query = '',
  });

  factory ContactsState.empty() =>
      const ContactsState(friends: [], incoming: [], outgoing: {}, hits: []);

  final List<KimPerson> friends;
  final List<KimPerson> incoming;
  final Set<String> outgoing;
  final List<KimPerson> hits;
  final bool ready;
  final bool loading;
  final String query;

  int get incomingCount => incoming.length;

  bool isFriend(String account) => friends.any((p) => p.account == account);

  bool isOutgoing(String account) => outgoing.contains(account);

  bool isIncoming(String account) => incoming.any((p) => p.account == account);

  KimPerson? person(String account) {
    for (final p in friends) {
      if (p.account == account) {
        return p;
      }
    }
    for (final p in incoming) {
      if (p.account == account) {
        return p;
      }
    }
    return null;
  }

  ContactsState copyWith({
    List<KimPerson>? friends,
    List<KimPerson>? incoming,
    Set<String>? outgoing,
    List<KimPerson>? hits,
    bool? ready,
    bool? loading,
    String? query,
  }) {
    return ContactsState(
      friends: friends ?? this.friends,
      incoming: incoming ?? this.incoming,
      outgoing: outgoing ?? this.outgoing,
      hits: hits ?? this.hits,
      ready: ready ?? this.ready,
      loading: loading ?? this.loading,
      query: query ?? this.query,
    );
  }
}

class ContactsNotifier extends Notifier<ContactsState> {
  @override
  ContactsState build() {
    ref.listen<bool>(sessionProvider.select((s) => s.signedIn), (prev, next) {
      if (next == false) {
        state = ContactsState.empty();
      }
    });
    ref.listen<ConnStatus>(sessionProvider.select((s) => s.status), (
      prev,
      next,
    ) {
      if (next == ConnStatus.online) {
        unawaited(refresh());
      }
    });
    Future.microtask(() {
      if (ref.read(sessionProvider).status == ConnStatus.online) {
        unawaited(refresh());
      }
    });
    return ContactsState.empty();
  }

  Future<void> refresh() async {
    final session = ref.read(sessionProvider);
    if (session.status != ConnStatus.online) {
      return;
    }
    final client = ref.read(clientPortProvider);
    state = state.copyWith(loading: true);
    try {
      final friends = await client.friendList();
      final incoming = await client.friendIncoming();
      final friendIds = {for (final p in friends) p.account};
      state = state.copyWith(
        friends: friends,
        incoming: incoming,
        outgoing: {...state.outgoing}..removeWhere(friendIds.contains),
        ready: true,
        loading: false,
      );
    } catch (_) {
      state = state.copyWith(ready: true, loading: false);
    }
  }

  Future<void> search(String query) async {
    final q = query.trim();
    state = state.copyWith(query: q);
    if (q.isEmpty) {
      state = state.copyWith(hits: const []);
      return;
    }
    final rows = await ref.read(clientPortProvider).searchUsers(q);
    state = state.copyWith(hits: rows, query: q);
  }

  Future<void> request(String dest) async {
    await ref.read(clientPortProvider).friendRequest(dest);
    await refresh();
    if (state.isFriend(dest)) {
      await KimHaptics.success();
      return;
    }
    state = state.copyWith(outgoing: {...state.outgoing, dest});
    await KimHaptics.light();
  }

  Future<void> accept(String dest) async {
    await ref.read(clientPortProvider).friendAccept(dest);
    await refresh();
    await KimHaptics.success();
  }

  Future<void> reject(String dest) async {
    await ref.read(clientPortProvider).friendReject(dest);
    await refresh();
  }
}

final contactsProvider = NotifierProvider<ContactsNotifier, ContactsState>(
  ContactsNotifier.new,
);

String socialError(Object err) {
  final msg = err.toString();
  if (msg.contains('status 108')) {
    return Copy.userNotFound;
  }
  if (msg.contains('status 110')) {
    return Copy.blocked;
  }
  if (msg.contains('status 101')) {
    return Copy.cannotAddSelf;
  }
  if (msg.contains('status 109')) {
    return Copy.notFriends;
  }
  return Copy.sendFailed;
}
