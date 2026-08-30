library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../copy.dart';
import '../core/errors.dart';
import '../core/haptics.dart';
import '../core/user_agent.dart';
import '../models/models.dart';
import 'providers.dart';

class SessionState {
  const SessionState({
    required this.signedIn,
    required this.account,
    required this.status,
    this.connectError,
    this.busy = false,
  });

  factory SessionState.signedOut() => const SessionState(
    signedIn: false,
    account: '',
    status: ConnStatus.offline,
  );

  final bool signedIn;
  final String account;
  final ConnStatus status;
  final String? connectError;
  final bool busy;

  String get statusLabel => switch (status) {
    ConnStatus.online => Copy.online,
    ConnStatus.connecting => Copy.connecting,
    ConnStatus.reconnecting => Copy.reconnecting,
    ConnStatus.offline => Copy.offline,
  };

  SessionState copyWith({
    bool? signedIn,
    String? account,
    ConnStatus? status,
    String? connectError,
    bool clearError = false,
    bool? busy,
  }) {
    return SessionState(
      signedIn: signedIn ?? this.signedIn,
      account: account ?? this.account,
      status: status ?? this.status,
      connectError: clearError ? null : (connectError ?? this.connectError),
      busy: busy ?? this.busy,
    );
  }
}

class SessionNotifier extends Notifier<SessionState> {
  var _connectGen = 0;

  @override
  SessionState build() {
    final runtime = ref.watch(runtimeProvider);
    final signedIn = runtime.settings.token.isNotEmpty;
    if (signedIn) {
      Future.microtask(connect);
    }
    return SessionState(
      signedIn: signedIn,
      account: runtime.settings.account,
      status: signedIn ? ConnStatus.connecting : ConnStatus.offline,
    );
  }

  Future<void> signIn({
    required bool register,
    required String account,
    required String password,
  }) async {
    final runtime = ref.read(runtimeProvider);
    final auth = ref.read(authPortProvider);
    final ua = kimUserAgent(runtime);
    final origin = runtime.settings.httpOrigin;
    final session = register
        ? await auth.register(
            origin: origin,
            userAgent: ua,
            account: account,
            password: password,
          )
        : await auth.login(
            origin: origin,
            userAgent: ua,
            account: account,
            password: password,
          );
    await runtime.settings.saveSession(
      token: session.token,
      account: session.account,
    );
    await KimHaptics.success();
    state = SessionState(
      signedIn: true,
      account: session.account,
      status: ConnStatus.connecting,
    );
    unawaited(connect());
  }

  Future<void> connect() async {
    final runtime = ref.read(runtimeProvider);
    final token = runtime.settings.token;
    if (token.isEmpty) {
      return;
    }
    final gen = ++_connectGen;
    state = state.copyWith(status: ConnStatus.connecting, clearError: true);
    try {
      final client = ref.read(clientPortProvider);
      await client.connect(
        runtime.settings.url,
        token,
        userAgent: kimUserAgent(runtime),
      );
      await client.loginWs();
      if (gen != _connectGen) {
        return;
      }
      state = state.copyWith(status: ConnStatus.online, clearError: true);
    } catch (err) {
      if (gen != _connectGen) {
        return;
      }
      state = state.copyWith(
        status: ConnStatus.offline,
        connectError: mapUserError(err),
      );
    }
  }

  /// Radio or socket dropped. Keep the JWT; tear down the WS off the UI path.
  void dropLink({String? error}) {
    if (!state.signedIn) {
      return;
    }
    _connectGen++;
    if (state.status == ConnStatus.offline &&
        (error == null || error == state.connectError)) {
      unawaited(_dropSocket());
      return;
    }
    state = state.copyWith(
      status: ConnStatus.offline,
      connectError: error ?? Copy.offline,
    );
    unawaited(_dropSocket());
  }

  Future<void> _dropSocket() async {
    try {
      await ref.read(clientPortProvider).disconnect();
    } catch (_) {}
  }

  Future<void> signOut() async {
    if (state.busy) {
      return;
    }
    state = state.copyWith(busy: true);
    final runtime = ref.read(runtimeProvider);
    final auth = ref.read(authPortProvider);
    final client = ref.read(clientPortProvider);
    try {
      try {
        await client.disconnect();
      } catch (_) {}
      try {
        await auth.logout(
          origin: runtime.settings.httpOrigin,
          userAgent: kimUserAgent(runtime),
          token: runtime.settings.token,
        );
      } catch (_) {}
      await runtime.settings.clearSession();
      await KimHaptics.success();
      _connectGen++;
      state = SessionState.signedOut();
    } finally {
      if (state.busy) {
        state = state.copyWith(busy: false);
      }
    }
  }

  Future<void> changePassword({
    required String oldPassword,
    required String newPassword,
  }) async {
    final runtime = ref.read(runtimeProvider);
    await ref
        .read(authPortProvider)
        .changePassword(
          origin: runtime.settings.httpOrigin,
          userAgent: kimUserAgent(runtime),
          token: runtime.settings.token,
          oldPassword: oldPassword,
          newPassword: newPassword,
        );
    await KimHaptics.success();
  }

  void markOffline({String? error}) => dropLink(error: error);
}

final sessionProvider = NotifierProvider<SessionNotifier, SessionState>(
  SessionNotifier.new,
);
