library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';
import 'contacts.dart';
import 'link.dart';
import 'providers.dart';
import 'session.dart';

class ProfileState {
  const ProfileState({
    required this.account,
    required this.nickname,
    required this.avatar,
  });

  factory ProfileState.empty() =>
      const ProfileState(account: '', nickname: '', avatar: '');

  final String account;
  final String nickname;
  final String avatar;

  String get title => nickname.isEmpty ? account : nickname;
}

class ProfileNotifier extends Notifier<ProfileState> {
  var _fetched = '';
  ProfileState? _hold;

  @override
  ProfileState build() {
    final account = ref.watch(sessionProvider.select((s) => s.account));
    final cached = ref.watch(runtimeProvider).settings.avatarOf(account);
    final online = ref.watch(
      linkProvider.select((g) => g.status == ConnStatus.online),
    );
    if (account.isEmpty) {
      _fetched = '';
      _hold = null;
      return ProfileState.empty();
    }
    if (online && _fetched != account) {
      unawaited(_refresh());
    }
    final hold = _hold;
    if (hold != null && hold.account == account) {
      return hold;
    }
    return ProfileState(account: account, nickname: account, avatar: cached);
  }

  Future<void> _refresh() async {
    final account = ref.read(sessionProvider).account;
    if (account.isEmpty) {
      return;
    }
    _fetched = account;
    try {
      final person = await ref.read(clientPortProvider).profile();
      if (!ref.mounted) {
        return;
      }
      await _apply(person);
    } catch (_) {
      // Keep the cached row.
    }
  }

  Future<void> applyAvatar(String url) async {
    final current = state;
    final nickname = current.nickname.isEmpty
        ? current.account
        : current.nickname;
    final person = await ref
        .read(clientPortProvider)
        .updateProfile(nickname: nickname, avatar: url);
    if (!ref.mounted) {
      return;
    }
    await _apply(person);
  }

  Future<void> _apply(KimPerson person) async {
    await ref.read(runtimeProvider).settings.saveAvatar(person.avatar);
    if (!ref.mounted) {
      return;
    }
    final account = person.account.isEmpty ? state.account : person.account;
    _fetched = account;
    final next = ProfileState(
      account: account,
      nickname: person.nickname.isEmpty ? account : person.nickname,
      avatar: person.avatar,
    );
    _hold = next;
    state = next;
  }
}

final profileProvider = NotifierProvider<ProfileNotifier, ProfileState>(
  ProfileNotifier.new,
);

String avatarFor(ProfileState me, ContactsState social, String account) {
  if (account.isEmpty) {
    return '';
  }
  if (account == me.account) {
    return me.avatar;
  }
  return social.person(account)?.avatar ?? '';
}
