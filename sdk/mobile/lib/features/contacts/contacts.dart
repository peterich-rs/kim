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
  StreamSubscription<SessionUpdateDto>? _events;

  @override
  ContactsState build() {
    ref.onDispose(() {
      unawaited(_events?.cancel());
      _events = null;
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
    _listenEvents();
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

  void _listenEvents() {
    if (_events != null) {
      return;
    }
    _events = ref.read(clientPortProvider).watchSessionEvents().listen((event) {
      if (!ref.mounted) {
        return;
      }
      switch (event) {
        case SessionUpdateDto_FriendRequest(:final from, :final nickname):
          onRequest(from, nickname);
        case SessionUpdateDto_FriendAccepted(:final from, :final nickname):
          onAccepted(from, nickname);
        case SessionUpdateDto_ProfileUpdated(
          :final account,
          :final nickname,
          :final avatar,
        ):
          onProfileUpdated(account, nickname, avatar);
        case SessionUpdateDto_ContactsChanged(:final contacts):
          applyChanged(contacts);
        default:
          break;
      }
    });
  }

  Future<void> refresh() async {
    if (ref.read(kimSessionProvider).link is! LinkStateDto_Online) {
      return;
    }
    final client = ref.read(clientPortProvider);
    state = state.copyWith(loading: true);
    try {
      final rows = await client.refreshContacts();
      if (!ref.mounted) {
        return;
      }
      applyChanged(rows);
    } catch (e, st) {
      KimLogger.warn('contacts refresh', e, st);
      if (ref.mounted) {
        state = state.copyWith(ready: true, loading: false);
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
    state = state.copyWith(outgoing: {...state.outgoing, dest});
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
    state = state.copyWith(
      friends: state.friends.where((p) => p.account != dest).toList(),
      hits: state.hits.where((p) => p.account != dest).toList(),
      outgoing: {...state.outgoing}..remove(dest),
    );
    await KimHaptics.success();
  }

  void applyChanged(List<PersonDto> contacts) {
    final friends = <KimPerson>[];
    final incoming = <KimPerson>[];
    final outgoing = <String>{};
    for (final p in contacts) {
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
      outgoing: {...state.outgoing, ...outgoing}
        ..removeWhere(friendIds.contains),
      ready: true,
      loading: false,
    );
  }

  void onRequest(String from, String nickname) {
    if (from.isEmpty || state.isFriend(from)) {
      return;
    }
    if (state.isOutgoing(from)) {
      onAccepted(from, nickname);
      return;
    }
    if (state.isIncoming(from)) {
      return;
    }
    final person = KimPerson(
      account: from,
      nickname: nickname.isEmpty ? from : nickname,
    );
    state = state.copyWith(incoming: [person, ...state.incoming]);
  }

  void onAccepted(String from, String nickname) {
    if (from.isEmpty) {
      return;
    }
    final person = KimPerson(
      account: from,
      nickname: nickname.isEmpty ? from : nickname,
    );
    final friends = [
      if (!state.isFriend(from)) person,
      ...state.friends.where((p) => p.account != from),
    ];
    state = state.copyWith(
      friends: friends,
      incoming: state.incoming.where((p) => p.account != from).toList(),
      outgoing: {...state.outgoing}..remove(from),
    );
    unawaited(refresh());
  }

  /// Patch a known contact's nickname/avatar from `chat.user.updated`.
  void onProfileUpdated(String account, String nickname, String avatar) {
    if (account.isEmpty) {
      return;
    }
    final title = nickname.isEmpty ? account : nickname;
    List<KimPerson> patch(List<KimPerson> rows) {
      return [
        for (final p in rows)
          if (p.account == account)
            KimPerson(
              account: account,
              nickname: title,
              avatar: avatar,
              bio: p.bio,
              kind: p.kind,
            )
          else
            p,
      ];
    }

    final friends = patch(state.friends);
    final incoming = patch(state.incoming);
    final hits = patch(state.hits);
    if (friends == state.friends &&
        incoming == state.incoming &&
        hits == state.hits) {
      return;
    }
    state = state.copyWith(friends: friends, incoming: incoming, hits: hits);
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
