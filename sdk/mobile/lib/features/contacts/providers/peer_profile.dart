library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/models/models.dart';

class PeerProfileView {
  const PeerProfileView({this.person, this.loading = true, this.error});

  final KimPerson? person;
  final bool loading;
  final String? error;
}

class PeerProfileUi extends Notifier<PeerProfileView> {
  PeerProfileUi(this.account);

  final String account;

  @override
  PeerProfileView build() => const PeerProfileView();

  void showCached(KimPerson person) {
    state = PeerProfileView(person: person, loading: false, error: state.error);
  }

  void showLoaded(KimPerson person) {
    state = PeerProfileView(person: person, loading: false);
  }

  void showFailed({required String error, required KimPerson fallback}) {
    state = PeerProfileView(
      person: state.person ?? fallback,
      loading: false,
      error: state.error ?? error,
    );
  }
}

final peerProfileProvider = NotifierProvider.autoDispose
    .family<PeerProfileUi, PeerProfileView, String>(PeerProfileUi.new);
