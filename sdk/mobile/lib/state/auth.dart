library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../copy.dart';
import '../core/haptics.dart';
import '../core/jwt.dart';
import '../core/user_agent.dart';
import 'providers.dart';

class AuthState {
  const AuthState({required this.signedIn, required this.account, this.notice});

  factory AuthState.signedOut({String? notice}) =>
      AuthState(signedIn: false, account: '', notice: notice);

  final bool signedIn;
  final String account;
  final String? notice;
}

class AuthNotifier extends Notifier<AuthState> {
  @override
  AuthState build() {
    final settings = ref.watch(runtimeProvider).settings;
    final expired =
        settings.token.isNotEmpty && JwtPeek.isExpired(settings.token);
    if (expired) {
      settings.discardedExpiredToken = true;
      unawaited(settings.saveToken(''));
    }
    if (settings.token.isEmpty || expired) {
      return AuthState.signedOut(
        notice: (expired || settings.discardedExpiredToken)
            ? Copy.sessionExpired
            : null,
      );
    }
    return AuthState(signedIn: true, account: settings.account);
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
    if (!ref.mounted) {
      return;
    }
    await runtime.settings.saveSession(
      token: session.token,
      account: session.account,
    );
    if (!ref.mounted) {
      return;
    }
    await KimHaptics.success();
    state = AuthState(signedIn: true, account: session.account);
  }

  Future<void> signOut({bool expired = false}) async {
    final runtime = ref.read(runtimeProvider);
    final auth = ref.read(authPortProvider);
    final client = ref.read(clientPortProvider);
    try {
      await client.stopSession();
    } catch (_) {}
    if (!ref.mounted) {
      return;
    }
    try {
      await auth.logout(
        origin: runtime.settings.httpOrigin,
        userAgent: kimUserAgent(runtime),
        token: runtime.settings.token,
      );
    } catch (_) {}
    if (!ref.mounted) {
      return;
    }
    await runtime.settings.clearSession();
    if (!ref.mounted) {
      return;
    }
    if (expired) {
      await KimHaptics.error();
      state = AuthState.signedOut(notice: Copy.sessionExpired);
      return;
    }
    await KimHaptics.success();
    state = AuthState.signedOut();
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
    if (!ref.mounted) {
      return;
    }
    await KimHaptics.success();
  }

  Future<void> savePushedToken(String token) async {
    if (token.isEmpty) {
      return;
    }
    await ref.read(runtimeProvider).settings.saveToken(token);
  }
}

final authProvider = NotifierProvider<AuthNotifier, AuthState>(
  AuthNotifier.new,
);
