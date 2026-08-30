library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../copy.dart';
import '../core/errors.dart';
import '../models/models.dart';
import 'auth.dart';
import 'gateway.dart';

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

  factory SessionState.from(AuthState auth, AsyncValue<ConnStatus> gateway) {
    return SessionState(
      signedIn: auth.signedIn,
      account: auth.account,
      status: statusOf(gateway),
      connectError: errorOf(gateway),
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

  static ConnStatus statusOf(AsyncValue<ConnStatus> gateway) {
    if (gateway.retrying) {
      return ConnStatus.reconnecting;
    }
    if (gateway.isLoading && !gateway.hasValue) {
      return ConnStatus.connecting;
    }
    if (gateway.hasError) {
      return ConnStatus.offline;
    }
    return gateway.value ?? ConnStatus.offline;
  }

  static String? errorOf(AsyncValue<ConnStatus> gateway) {
    if (!gateway.hasError || gateway.retrying) {
      return null;
    }
    return mapUserError(gateway.error!);
  }
}

/// Derived view of [authProvider] + [gatewayProvider] for chrome (banner, me).
final sessionProvider = Provider<SessionState>((ref) {
  return SessionState.from(ref.watch(authProvider), ref.watch(gatewayProvider));
});
