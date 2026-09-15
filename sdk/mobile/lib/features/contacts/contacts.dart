library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/features/agent/host_support.dart';
import 'package:kim_mobile/features/agent/mention.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/failures.dart';
import 'package:kim_mobile/core/haptics.dart';
import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/models/models.dart';
import 'package:kim_mobile/src/rust/api/types.dart';
import 'package:kim_mobile/features/agent/agent_profiles.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/session/kim_session.dart';
import 'package:kim_mobile/features/session/providers.dart';

class ContactsState {
  const ContactsState({
    required this.friends,
    required this.incoming,
    required this.outgoing,
    required this.hits,
    this.ready = false,
    this.loading = false,
    this.query = '',
    this.syncError,
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
  final String? syncError;

  int get incomingCount => incoming.length;

  bool isFriend(String account) =>
      isAgentDest(account) ||
      isServerBotAccount(account) ||
      friends.any((p) => p.account == account);

  bool isOutgoing(String account) => outgoing.contains(account);

  bool isIncoming(String account) => incoming.any((p) => p.account == account);

  KimPerson? person(String account) {
    final canon = canonicalAgentDest(account);
    for (final p in friends) {
      if (p.account == account || p.account == canon) {
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
    String? syncError,
    bool syncErrorSet = false,
  }) {
    return ContactsState(
      friends: friends ?? this.friends,
      incoming: incoming ?? this.incoming,
      outgoing: outgoing ?? this.outgoing,
      hits: hits ?? this.hits,
      ready: ready ?? this.ready,
      loading: loading ?? this.loading,
      query: query ?? this.query,
      syncError: syncErrorSet ? syncError : this.syncError,
    );
  }
}

class ContactsNotifier extends Notifier<ContactsState> {
  StreamSubscription<ContactsSnapshotDto>? _contacts;

  @override
  ContactsState build() {
    ref.onDispose(() {
      unawaited(_contacts?.cancel());
      _contacts = null;
    });
    ref.listen(authProvider.select((s) => s.signedIn), (prev, next) {
      if (next == false) {
        state = ContactsState.empty();
      }
    });
    ref.listen(kimSessionProvider.select((s) => s.link), (prev, next) {
      if (next is LinkStateDto_Online) {
        unawaited(refresh());
      }
    });
    _contacts = ref
        .read(clientPortProvider)
        .watchContacts()
        .listen(
          _applySnapshot,
          onError: (Object error, StackTrace stackTrace) {
            KimLogger.warn('contacts watch', error, stackTrace);
          },
        );
    Future.microtask(() {
      if (!ref.mounted) {
        return;
      }
      if (ref.read(kimSessionProvider).link is LinkStateDto_Online) {
        unawaited(refresh());
      }
    });
    ref.listen(agentProfilesProvider, (prev, next) {
      _syncLocalAgents();
    });
    final agents = ref.read(agentProfilesProvider.notifier).visibleAgents;
    return ContactsState(
      friends: !agentHostSupported
          ? const []
          : withLocalAgents(const [], agents),
      incoming: const [],
      outgoing: const {},
      hits: const [],
    );
  }

  void _syncLocalAgents() {
    if (!ref.mounted) {
      return;
    }
    final humans = [
      for (final p in state.friends)
        if (!isAgentDest(p.account)) p,
    ];
    final agents = ref.read(agentProfilesProvider.notifier).visibleAgents;
    state = state.copyWith(
      friends: !agentHostSupported ? humans : withLocalAgents(humans, agents),
    );
  }

  Future<void> refresh() async {
    final client = ref.read(clientPortProvider);
    state = state.copyWith(loading: true);
    try {
      await client.refreshContacts();
    } catch (e, st) {
      KimLogger.warn('contacts refresh', e, st);
    } finally {
      if (ref.mounted) {
        state = state.copyWith(loading: false);
      }
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
    if (!ref.mounted) {
      return;
    }
    final needle = q.toLowerCase();
    final agents = ref.read(agentProfilesProvider.notifier).visibleAgents;
    final localHit = agents.any(
      (p) =>
          p.id.toLowerCase().contains(needle) ||
          p.displayName.contains(q) ||
          p.aliases.any((a) => a.contains(q)),
    );
    state = state.copyWith(
      hits: localHit ? withLocalAgents(rows, agents) : rows,
      query: q,
    );
  }

  Future<void> request(String dest) async {
    await ref.read(clientPortProvider).friendRequest(dest);
    if (!ref.mounted) {
      return;
    }
    await refresh();
    if (!ref.mounted) {
      return;
    }
    if (state.isFriend(dest)) {
      await KimHaptics.success();
      return;
    }
    await KimHaptics.light();
  }

  Future<void> accept(String dest) async {
    await ref.read(clientPortProvider).friendAccept(dest);
    if (!ref.mounted) {
      return;
    }
    await refresh();
    if (ref.mounted) {
      await KimHaptics.success();
    }
  }

  Future<void> reject(String dest) async {
    await ref.read(clientPortProvider).friendReject(dest);
    if (ref.mounted) {
      await refresh();
    }
  }

  /// Remove a human friend (`chat.friend.remove`) or own bot (`chat.bot.delete`).
  Future<void> removePeer(String dest, {required bool isBot}) async {
    final client = ref.read(clientPortProvider);
    if (isBot) {
      await client.botDelete(dest);
    } else {
      await client.friendRemove(dest);
    }
    if (!ref.mounted) {
      return;
    }
    await KimHaptics.success();
  }

  void _applySnapshot(ContactsSnapshotDto snapshot) {
    if (!ref.mounted) {
      return;
    }
    final friends = <KimPerson>[];
    final incoming = <KimPerson>[];
    final outgoing = <String>{};
    for (final p in snapshot.contacts) {
      final person = KimPerson(
        account: p.account,
        nickname: p.nickname.isEmpty ? p.account : p.nickname,
        avatar: p.avatar,
        bio: p.bio,
        kind: p.kind == ProfileKind.bot ? ProfileKind.bot : ProfileKind.user,
      );
      switch (p.relation) {
        case 'incoming':
          incoming.add(person);
        case 'outgoing':
          outgoing.add(p.account);
        default:
          friends.add(person);
      }
    }
    final friendIds = {for (final p in friends) p.account};
    state = state.copyWith(
      friends: !agentHostSupported
          ? friends
          : withLocalAgents(
              friends,
              ref.read(agentProfilesProvider.notifier).visibleAgents,
            ),
      incoming: incoming,
      outgoing: outgoing..removeWhere(friendIds.contains),
      ready: true,
      syncError: snapshot.syncError,
      syncErrorSet: true,
    );
  }
}

final contactsProvider = NotifierProvider<ContactsNotifier, ContactsState>(
  ContactsNotifier.new,
);

String socialError(Object err) {
  final kim = KimException.tryFrom(err);
  if (kim != null) {
    return switch (kim.kind) {
      KimErrorKind.userNotFound => Copy.userNotFound,
      KimErrorKind.blocked => Copy.blocked,
      KimErrorKind.cannotChatSelf => Copy.cannotAddSelf,
      KimErrorKind.notFriends => Copy.notFriends,
      KimErrorKind.protocol when kim.message.contains('113') =>
        Copy.botSocialDenied,
      _ => Copy.sendFailed,
    };
  }
  return Copy.sendFailed;
}
