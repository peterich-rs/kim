library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../copy.dart';
import '../models/models.dart';
import 'auth.dart';
import 'link.dart';

class SessionState {
  const SessionState({
    required this.signedIn,
    required this.account,
    required this.status,
    this.connectError,
  });

  factory SessionState.signedOut() => const SessionState(
    signedIn: false,
    account: '',
    status: ConnStatus.offline,
  );

  factory SessionState.from(AuthState auth, KimLinkState link) {
    return SessionState(
      signedIn: auth.signedIn,
      account: auth.account,
      status: link.status,
      connectError: link.error,
    );
  }

  final bool signedIn;
  final String account;
  final ConnStatus status;
  final String? connectError;

  String get statusLabel => switch (status) {
    ConnStatus.online => Copy.online,
    ConnStatus.connecting => Copy.connecting,
    ConnStatus.reconnecting => Copy.reconnecting,
    ConnStatus.offline => Copy.offline,
  };
}

/// Derived view of [authProvider] + [linkProvider] for chrome (banner, me).
final sessionProvider = Provider<SessionState>((ref) {
  return SessionState.from(ref.watch(authProvider), ref.watch(linkProvider));
});
